//! The Storage service: volumes, scans, and navigation over the index.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::db::{self, ChildQuery, StorageQueries};
use crate::error::{CoreError, Result};
use crate::model::{
    Category, EntryRow, ExtensionBucket, ScanError, ScanErrorKind, ScanOptions, ScanProgress,
    ScanSummary, VolumeInfo,
};
use crate::paths::AppPaths;
use crate::platform::{self, SystemProbe};
use crate::scanner::{run_scan, CancellationToken, ScanRequest};

/// Everything the Storage screen needs after a scan, in one round trip.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSnapshot {
    pub scan: ScanSummary,
    pub root: EntryRow,
    pub largest_folders: Vec<EntryRow>,
    pub largest_files: Vec<EntryRow>,
    pub categories: Vec<CategoryBucket>,
    pub extensions: Vec<ExtensionBucket>,
    pub errors: Vec<ErrorBucket>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBucket {
    pub category: Category,
    pub label: String,
    pub file_count: i64,
    pub total_size: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBucket {
    pub kind: ScanErrorKind,
    pub count: i64,
    /// A short, non-alarming explanation of what this class of error means.
    pub explanation: String,
}

/// A running scan, so the UI can cancel it.
#[derive(Clone)]
pub struct ScanHandle {
    pub scan_root: PathBuf,
    token: CancellationToken,
}

impl ScanHandle {
    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

/// Owns the index connection and mediates every storage operation.
///
/// The connection lives behind a mutex and is *moved* into a scan for the
/// duration of that scan, so a scan and a navigation query can never race on
/// the same handle. Navigation during a scan reads through a separate
/// short-lived connection, which WAL makes safe.
pub struct StorageService {
    paths: AppPaths,
    conn: Mutex<Option<Connection>>,
    probe: &'static dyn SystemProbe,
    active_scan: Mutex<Option<ScanHandle>>,
}

impl StorageService {
    pub fn new(paths: AppPaths) -> Result<Arc<Self>> {
        let conn = db::open(&paths.database)?;
        // A scan left in `running` means the app died mid-scan. Recording that
        // at startup keeps the index's own account of itself truthful.
        StorageQueries::reap_interrupted_scans(&conn)?;
        Ok(Arc::new(Self {
            paths,
            conn: Mutex::new(Some(conn)),
            probe: platform::probe(),
            active_scan: Mutex::new(None),
        }))
    }

    /// Build a service over an in-memory index. Used by tests and benchmarks.
    pub fn in_memory(paths: AppPaths) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            paths,
            conn: Mutex::new(Some(db::open_in_memory()?)),
            probe: platform::probe(),
            active_scan: Mutex::new(None),
        }))
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn platform_name(&self) -> &'static str {
        self.probe.name()
    }

    pub fn volumes(&self) -> Result<Vec<VolumeInfo>> {
        self.probe.volumes()
    }

    /// Borrow the index for a read. Returns an error while a scan owns it,
    /// which the caller turns into "busy" rather than blocking the UI thread.
    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.conn.lock().expect("index mutex poisoned");
        match guard.as_ref() {
            Some(conn) => f(conn),
            // A scan holds the primary connection. Open a second read-only view
            // instead of making the UI wait; WAL means it sees a consistent
            // snapshot of everything committed so far.
            None => {
                drop(guard);
                let conn = db::open(&self.paths.database)?;
                f(&conn)
            }
        }
    }

    /// Default options for scanning a volume, with the platform's mount table
    /// folded in so the walk stays on the chosen volume.
    pub fn default_options(&self, root: &Path) -> ScanOptions {
        let mut options = ScanOptions::new(root);
        options.skip_paths = self.probe.skip_mount_points(root);
        // A macOS boot volume is two APFS volumes joined by firmlinks, so a
        // scan of `/` has to be allowed to change device. `skip_paths` is what
        // keeps it from wandering onto genuinely different disks.
        options.cross_device = root == Path::new("/");
        options
    }

    /// Run a scan to completion. Blocking; callers run it on their own thread.
    pub fn scan(
        &self,
        options: ScanOptions,
        mut on_progress: impl FnMut(ScanProgress),
    ) -> Result<ScanSummary> {
        let token = CancellationToken::new();
        let handle = ScanHandle { scan_root: options.root.clone(), token: token.clone() };
        {
            let mut active = self.active_scan.lock().expect("scan mutex poisoned");
            if active.is_some() {
                return Err(CoreError::invalid("a scan is already running"));
            }
            *active = Some(handle);
        }

        let conn = self
            .conn
            .lock()
            .expect("index mutex poisoned")
            .take()
            .ok_or_else(|| CoreError::invalid("the index is busy"))?;

        let request = ScanRequest { options, volume_id: None, cancel: token };
        let outcome = run_scan(conn, &request, Some(&self.paths.database), &mut on_progress);

        // The connection goes back whether the scan succeeded or not; losing it
        // would leave the app unable to read its own index.
        let result = match outcome {
            Ok((conn, summary)) => {
                *self.conn.lock().expect("index mutex poisoned") = Some(conn);
                Ok(summary)
            }
            Err(err) => {
                if let Ok(conn) = db::open(&self.paths.database) {
                    *self.conn.lock().expect("index mutex poisoned") = Some(conn);
                }
                Err(err)
            }
        };
        *self.active_scan.lock().expect("scan mutex poisoned") = None;
        result
    }

    pub fn cancel_active_scan(&self) -> bool {
        match self.active_scan.lock().expect("scan mutex poisoned").as_ref() {
            Some(handle) => {
                handle.cancel();
                true
            }
            None => false,
        }
    }

    pub fn active_scan(&self) -> Option<PathBuf> {
        self.active_scan
            .lock()
            .expect("scan mutex poisoned")
            .as_ref()
            .map(|handle| handle.scan_root.clone())
    }

    /// The most recent usable scan for a root, if one exists.
    pub fn latest_scan(&self, root: &Path) -> Result<Option<ScanSummary>> {
        let root = root.to_string_lossy().into_owned();
        self.with_conn(|conn| StorageQueries::latest_scan_for_root(conn, &root))
    }

    pub fn recent_scans(&self, limit: u32) -> Result<Vec<ScanSummary>> {
        self.with_conn(|conn| StorageQueries::recent_scans(conn, limit))
    }

    /// Assemble the Storage screen's overview for a finished scan.
    pub fn snapshot(&self, scan_id: i64) -> Result<StorageSnapshot> {
        self.with_conn(|conn| {
            let mut scan = StorageQueries::scan_summary(conn, scan_id)?;
            scan.index_bytes = db::index_size_bytes(&self.paths.database);
            let root = StorageQueries::root_entry(conn, scan_id)?;
            Ok(StorageSnapshot {
                largest_folders: StorageQueries::largest_directories(conn, scan_id, 12, 1)?,
                largest_files: StorageQueries::largest_files(conn, scan_id, 12)?,
                categories: StorageQueries::category_breakdown(conn, scan_id)?
                    .into_iter()
                    .map(|(category, file_count, total_size)| CategoryBucket {
                        category,
                        label: category.label().to_string(),
                        file_count,
                        total_size,
                    })
                    .collect(),
                extensions: StorageQueries::extension_breakdown(conn, scan_id, 12)?,
                errors: StorageQueries::error_summary(conn, scan_id)?
                    .into_iter()
                    .map(|(kind, count)| ErrorBucket { kind, count, explanation: explain(kind) })
                    .collect(),
                scan,
                root,
            })
        })
    }

    pub fn children(&self, query: &ChildQuery) -> Result<Vec<EntryRow>> {
        self.with_conn(|conn| StorageQueries::children(conn, query))
    }

    pub fn child_count(&self, scan_id: i64, parent_id: i64, hide_hidden: bool) -> Result<i64> {
        self.with_conn(|conn| StorageQueries::child_count(conn, scan_id, parent_id, hide_hidden))
    }

    pub fn entry(&self, scan_id: i64, entry_id: i64) -> Result<EntryRow> {
        self.with_conn(|conn| StorageQueries::entry_by_id(conn, scan_id, entry_id))
    }

    pub fn directory_by_path(&self, scan_id: i64, path: &str) -> Result<Option<EntryRow>> {
        self.with_conn(|conn| StorageQueries::directory_by_path(conn, scan_id, path))
    }

    pub fn ancestors(&self, scan_id: i64, entry_id: i64) -> Result<Vec<EntryRow>> {
        self.with_conn(|conn| StorageQueries::ancestors(conn, scan_id, entry_id))
    }

    pub fn scan_errors(&self, scan_id: i64, limit: u32) -> Result<Vec<ScanError>> {
        self.with_conn(|conn| StorageQueries::scan_errors(conn, scan_id, limit))
    }

    /// Drop old scans so the index does not grow without bound across rescans.
    pub fn prune(&self, keep_per_root: u32) -> Result<usize> {
        self.with_conn(|conn| StorageQueries::prune_scans(conn, keep_per_root))
    }
}

/// Plain-language explanations. Deliberately factual: a folder DiskMate could
/// not read is a fact about permissions, not a problem with the user's Mac.
fn explain(kind: ScanErrorKind) -> String {
    match kind {
        ScanErrorKind::PermissionDenied => {
            "macOS did not allow DiskMate to read these. Granting Full Disk Access in \
             System Settings > Privacy & Security lets them be included."
        }
        ScanErrorKind::NotFound | ScanErrorKind::Vanished => {
            "These were removed while the scan was running, so they are not included."
        }
        ScanErrorKind::TraversalLoop => {
            "These are second paths to a folder already counted, so they were not counted twice."
        }
        ScanErrorKind::CrossDevice => {
            "Another volume is mounted here. Scan it separately to see what it contains."
        }
        ScanErrorKind::Io => "The disk reported an error reading these.",
    }
    .to_string()
}
