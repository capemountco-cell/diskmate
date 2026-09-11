use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Lifecycle of a scan row. An interrupted scan is never silently discarded:
/// its partial index stays queryable and is labelled as incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
    /// The process died mid-scan; detected on next startup.
    Interrupted,
}

impl ScanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "failed" => Self::Failed,
            _ => Self::Interrupted,
        }
    }

    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

/// Knobs for a single scan. Defaults are tuned for a laptop: enough threads to
/// saturate an SSD without starving the UI, and a bounded queue so memory does
/// not grow with the size of the filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    pub root: PathBuf,
    /// Descend into directories that live on a different device than the root.
    /// Off by default so that scanning `/` does not wander into every mounted
    /// disk image and network share.
    pub cross_device: bool,
    /// Hard cap on traversal depth. `None` means unlimited.
    pub max_depth: Option<i32>,
    /// Directories the scanner must not descend into. The storage service fills
    /// this from the platform's mount table so that a scan of one volume does
    /// not wander into every other mounted disk, network share or synthetic
    /// filesystem. Matching is by exact path, which is what the mount table
    /// gives us.
    pub skip_paths: Vec<PathBuf>,
    /// Worker threads. `None` picks `min(available_parallelism, 8)`.
    pub threads: Option<usize>,
    /// Entries per batch handed to the database writer.
    pub batch_size: usize,
    /// Capacity of the traversal -> writer channel, in batches.
    pub queue_capacity: usize,
    /// Minimum interval between progress events, in milliseconds.
    pub progress_interval_ms: u64,
}

impl ScanOptions {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            cross_device: false,
            max_depth: None,
            skip_paths: Vec::new(),
            threads: None,
            batch_size: 2_048,
            queue_capacity: 64,
            progress_interval_ms: 150,
        }
    }

    pub fn resolved_threads(&self) -> usize {
        self.threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().clamp(1, 8))
                .unwrap_or(4)
        })
    }
}

/// Why DiskMate could not read something. Recorded per occurrence so the UI can
/// say "18 folders needed Full Disk Access" instead of silently under-counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanErrorKind {
    PermissionDenied,
    NotFound,
    /// Entry vanished between `read_dir` and `stat` — normal on a live system.
    Vanished,
    /// Refused to descend because the directory was already visited via another
    /// path (hard-linked directory, firmlink, bind mount).
    TraversalLoop,
    /// Skipped because it lives on a different device and `cross_device` is off.
    CrossDevice,
    Io,
}

impl ScanErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PermissionDenied => "permissionDenied",
            Self::NotFound => "notFound",
            Self::Vanished => "vanished",
            Self::TraversalLoop => "traversalLoop",
            Self::CrossDevice => "crossDevice",
            Self::Io => "io",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "permissionDenied" => Self::PermissionDenied,
            "notFound" => Self::NotFound,
            "vanished" => Self::Vanished,
            "traversalLoop" => Self::TraversalLoop,
            "crossDevice" => Self::CrossDevice,
            _ => Self::Io,
        }
    }

    pub fn from_io(error: &std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::NotFound => Self::NotFound,
            _ => Self::Io,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanError {
    pub path: String,
    pub kind: ScanErrorKind,
    /// Raw OS error number where one exists, for support diagnostics.
    pub code: Option<i32>,
    pub message: String,
}

/// A coalesced progress sample. Emitted on a timer, not per entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scan_id: i64,
    pub status: ScanStatus,
    pub entries_scanned: u64,
    pub files_scanned: u64,
    pub directories_scanned: u64,
    pub symlinks_scanned: u64,
    pub bytes_indexed: u64,
    pub errors: u64,
    pub elapsed_ms: u64,
    pub entries_per_second: f64,
    /// A directory currently being read. Sampled, so it is indicative only.
    pub current_path: Option<String>,
}

/// The durable result of a scan, also used as the benchmark record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub scan_id: i64,
    pub volume_id: Option<i64>,
    pub root_path: String,
    pub status: ScanStatus,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub duration_ms: i64,
    pub file_count: i64,
    pub dir_count: i64,
    pub symlink_count: i64,
    pub other_count: i64,
    pub error_count: i64,
    pub logical_bytes: i64,
    pub allocated_bytes: i64,
    pub entries_per_second: f64,
    /// Size of the SQLite index after this scan, in bytes.
    pub index_bytes: i64,
    pub engine_version: String,
}

impl ScanSummary {
    pub fn entry_count(&self) -> i64 {
        self.file_count + self.dir_count + self.symlink_count + self.other_count
    }
}
