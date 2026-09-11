use std::path::PathBuf;

/// Errors raised by the DiskMate engine.
///
/// Filesystem errors encountered *during* a scan are not represented here:
/// a scan must survive them, so they are recorded as [`crate::model::ScanError`]
/// rows instead. `CoreError` is reserved for failures that abort an operation.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("database migration failed at version {version}: {source}")]
    Migration {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },

    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("i/o error: {0}")]
    PlainIo(#[from] std::io::Error),

    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),

    #[error("scan {0} was not found in the index")]
    UnknownScan(i64),

    #[error("entry {0} was not found in the index")]
    UnknownEntry(i64),

    #[error("no indexed scan is available for this volume")]
    NoScanAvailable,

    #[error("the scan was cancelled")]
    Cancelled,

    #[error("{feature} is not available on this platform")]
    Unsupported { feature: &'static str },

    #[error("{0}")]
    Invalid(String),
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
