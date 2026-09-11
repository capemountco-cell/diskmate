//! The local SQLite index.
//!
//! DiskMate treats SQLite as the source of truth for filesystem knowledge. The
//! UI never receives a filesystem tree; it asks bounded questions ("the hundred
//! largest children of this directory") which this module answers with indexed
//! queries.

mod migrations;
mod queries;
pub mod writer;

pub use migrations::{current_schema_version, migrate, SCHEMA_VERSION};
pub use queries::{
    ChildQuery, SortDirection, SortKey, StorageQueries, VolumeUsage,
};
pub use writer::{DirAggregate, IndexWriter, WriterStats};

use std::path::Path;

use rusqlite::Connection;

use crate::error::{CoreError, Result};

/// Open the index, applying migrations and connection tuning.
///
/// Pragmas are chosen for a desktop application that does huge bursty writes
/// and then many small reads:
///
/// * `journal_mode = WAL` lets the UI read while a scan writes.
/// * `synchronous = NORMAL` is the right durability trade for a rebuildable
///   cache; the worst case is losing the tail of one scan, and a scan is always
///   repeatable.
/// * `temp_store = MEMORY` keeps the sort scratch for aggregation off disk.
/// * `mmap_size` of 256 MB makes the read-heavy navigation queries cheap
///   without loading the index into the heap.
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
    }
    let conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Open a private in-memory index. Used by tests and by benchmark runs that
/// should not touch the user's real database.
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    // `journal_mode` returns a row, so it cannot go through `execute_batch`.
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    conn.execute_batch(
        "PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -65536;
         PRAGMA mmap_size = 268435456;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

/// Size of the index on disk, including the write-ahead log.
pub fn index_size_bytes(path: &Path) -> i64 {
    let mut total = 0i64;
    for suffix in ["", "-wal", "-shm"] {
        let candidate = if suffix.is_empty() {
            path.to_path_buf()
        } else {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            std::path::PathBuf::from(name)
        };
        if let Ok(meta) = std::fs::metadata(&candidate) {
            total += meta.len() as i64;
        }
    }
    total
}

/// Seconds since the Unix epoch.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
