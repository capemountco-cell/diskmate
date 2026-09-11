//! Shared fixtures for the engine's integration tests.
//!
//! Every test builds a real directory tree on disk and scans it. Mocking the
//! filesystem would defeat the purpose: the behaviour worth testing here is
//! precisely what happens when `read_dir` and `lstat` meet a symlink, a
//! vanished entry, a name that is not valid UTF-8, or a directory the process
//! may not open.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use diskmate_core::db::ChildQuery;
use diskmate_core::model::{EntryRow, ScanOptions, ScanProgress, ScanSummary};
use diskmate_core::paths::AppPaths;
use diskmate_core::services::StorageService;

/// A temporary DiskMate: its own index and its own tree to scan.
pub struct Harness {
    pub service: Arc<StorageService>,
    pub tree: PathBuf,
    _data: tempfile::TempDir,
    _tree: tempfile::TempDir,
}

impl Harness {
    pub fn new() -> Self {
        let data = tempfile::tempdir().expect("temp data dir");
        let tree = tempfile::tempdir().expect("temp tree dir");
        let paths =
            AppPaths::resolve(None, Some(data.path().join("state"))).expect("resolve app paths");
        let service = StorageService::new(paths).expect("open index");
        // Canonicalised, because the scanner canonicalises its root and macOS
        // hands out temp directories under `/var`, which is a symlink to
        // `/private/var`. Without this the tests compare two spellings of one
        // path, and only notice on a Mac.
        let tree_path = tree.path().canonicalize().expect("canonicalise tree");
        Self {
            service,
            tree: tree_path,
            _data: data,
            _tree: tree,
        }
    }

    pub fn options(&self) -> ScanOptions {
        let mut options = ScanOptions::new(&self.tree);
        // Small batches and a shallow queue so the tests exercise the batching
        // and backpressure paths rather than fitting in one buffer.
        options.batch_size = 16;
        options.queue_capacity = 4;
        options.threads = Some(4);
        options.progress_interval_ms = 16;
        options
    }

    pub fn scan(&self) -> ScanSummary {
        self.scan_with(self.options())
    }

    pub fn scan_with(&self, options: ScanOptions) -> ScanSummary {
        self.service
            .scan(options, |_| {})
            .expect("scan should succeed")
    }

    pub fn scan_collecting_progress(
        &self,
        options: ScanOptions,
    ) -> (ScanSummary, Vec<ScanProgress>) {
        let collected = std::sync::Mutex::new(Vec::new());
        let summary = self
            .service
            .scan(options, |progress| collected.lock().unwrap().push(progress))
            .expect("scan should succeed");
        (summary, collected.into_inner().unwrap())
    }

    /// Children of the scan root, largest first.
    pub fn root_children(&self, scan_id: i64) -> Vec<EntryRow> {
        let root = self.service.snapshot(scan_id).expect("snapshot").root;
        self.service
            .children(&ChildQuery::new(scan_id, root.id))
            .expect("children")
    }

    pub fn child_named(&self, scan_id: i64, parent_id: i64, name: &str) -> EntryRow {
        self.service
            .children(&ChildQuery {
                limit: 1_000,
                ..ChildQuery::new(scan_id, parent_id)
            })
            .expect("children")
            .into_iter()
            .find(|row| row.name == name)
            .unwrap_or_else(|| panic!("no child named {name:?}"))
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.tree.join(relative)
    }
}

/// Create a directory, including parents.
pub fn dir(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    std::fs::create_dir_all(&path).expect("create dir");
    path
}

/// Create a file of exactly `size` bytes.
pub fn file(root: &Path, relative: &str, size: usize) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(&path, vec![b'x'; size]).expect("write file");
    path
}

#[cfg(unix)]
pub fn symlink(target: &Path, link: &Path) {
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::os::unix::fs::symlink(target, link).expect("create symlink");
}

/// Whether this process can still read a directory after its mode is cleared.
/// Root can, which changes what a permission test is allowed to assert.
pub fn running_as_root() -> bool {
    // SAFETY: `geteuid` takes no arguments and cannot fail.
    unsafe { libc::geteuid() == 0 }
}
