//! Batched index writer.
//!
//! The writer owns the only connection that mutates a scan's rows, and it is
//! driven by exactly one thread. That is deliberate: SQLite serialises writers
//! anyway, so a single writer with large batches beats several writers
//! contending for the same lock.
//!
//! Throughput rules encoded here:
//!
//! * Rows are inserted with multi-row `VALUES` tuples, so one prepared
//!   statement covers 128 entries instead of 128 round trips through the VDBE.
//! * Transactions span many batches, not one entry. A transaction per entry
//!   would be an fsync per entry.
//! * Query indexes are created *after* the bulk load. During load only the
//!   rowid primary key is maintained, which turns five incremental B-tree
//!   updates per row into five bulk sorts at the end.

use std::collections::HashMap;

use rusqlite::{params, Connection, Transaction};

use crate::error::Result;
use crate::model::{Category, EntryRecord, ScanError, ScanStatus};

use super::now_unix;

/// Entries per multi-row `INSERT`. 128 x 27 columns stays well inside SQLite's
/// bound-parameter limit while amortising statement overhead.
const INSERT_CHUNK: usize = 128;
/// Entries written before the writer commits and opens a fresh transaction.
/// Bounds how much work an interrupted scan loses and how large the WAL grows.
const COMMIT_EVERY: usize = 50_000;

const ENTRY_COLUMNS: &str = "id, scan_id, parent_id, name, path, depth, kind, \
     logical_size, allocated_size, subtree_logical_size, subtree_allocated_size, \
     subtree_file_count, subtree_dir_count, extension, modified_at, created_at, \
     accessed_at, mode, uid, gid, inode, device, nlink, is_hidden, hardlink_dup, \
     accessible, category";
const ENTRY_COLUMN_COUNT: usize = 27;

/// Aggregate totals for one directory, produced by the scanner's aggregator.
#[derive(Debug, Clone, Copy)]
pub struct DirAggregate {
    pub id: i64,
    pub logical: i64,
    pub allocated: i64,
    pub files: i64,
    pub dirs: i64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WriterStats {
    pub entries_written: u64,
    pub errors_written: u64,
    pub commits: u64,
}

pub struct IndexWriter {
    conn: Connection,
    scan_id: i64,
    in_transaction: bool,
    since_commit: usize,
    stats: WriterStats,
}

impl IndexWriter {
    /// Take ownership of a connection for the duration of one scan.
    pub fn new(conn: Connection, scan_id: i64) -> Self {
        Self {
            conn,
            scan_id,
            in_transaction: false,
            since_commit: 0,
            stats: WriterStats::default(),
        }
    }

    pub fn stats(&self) -> WriterStats {
        self.stats
    }

    pub fn scan_id(&self) -> i64 {
        self.scan_id
    }

    /// Create the scan row and return its id.
    pub fn begin_scan(
        conn: &Connection,
        volume_id: Option<i64>,
        root_path: &str,
        options_json: &str,
        engine_version: &str,
    ) -> Result<i64> {
        conn.execute(
            "INSERT INTO scans (volume_id, root_path, status, started_at, options_json, engine_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                volume_id,
                root_path,
                ScanStatus::Running.as_str(),
                now_unix(),
                options_json,
                engine_version
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Highest `fs_entries.id` in the database, so a new scan can continue the
    /// id sequence without colliding with a retained earlier scan.
    pub fn max_entry_id(conn: &Connection) -> Result<i64> {
        let max: Option<i64> =
            conn.query_row("SELECT max(id) FROM fs_entries", [], |r| r.get(0))?;
        Ok(max.unwrap_or(0))
    }

    fn ensure_transaction(&mut self) -> Result<()> {
        if !self.in_transaction {
            self.conn.execute_batch("BEGIN IMMEDIATE")?;
            self.in_transaction = true;
        }
        Ok(())
    }

    /// Commit the open transaction, if any.
    pub fn commit(&mut self) -> Result<()> {
        if self.in_transaction {
            self.conn.execute_batch("COMMIT")?;
            self.in_transaction = false;
            self.since_commit = 0;
            self.stats.commits += 1;
        }
        Ok(())
    }

    pub fn write_entries(&mut self, batch: &[EntryRecord]) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        self.ensure_transaction()?;
        let scan_id = self.scan_id;
        for chunk in batch.chunks(INSERT_CHUNK) {
            let sql = entry_insert_sql(chunk.len());
            let mut stmt = self.conn.prepare_cached(&sql)?;
            let mut values: Vec<rusqlite::types::Value> =
                Vec::with_capacity(chunk.len() * ENTRY_COLUMN_COUNT);
            for record in chunk {
                push_entry_values(&mut values, scan_id, record);
            }
            stmt.execute(rusqlite::params_from_iter(values))?;
        }
        self.stats.entries_written += batch.len() as u64;
        self.since_commit += batch.len();
        if self.since_commit >= COMMIT_EVERY {
            self.commit()?;
        }
        Ok(())
    }

    pub fn write_errors(&mut self, errors: &[ScanError]) -> Result<()> {
        if errors.is_empty() {
            return Ok(());
        }
        self.ensure_transaction()?;
        let scan_id = self.scan_id;
        let now = now_unix();
        {
            let mut stmt = self.conn.prepare_cached(
                "INSERT INTO scan_errors (scan_id, path, kind, code, message, occurred_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for error in errors {
                stmt.execute(params![
                    scan_id,
                    error.path,
                    error.kind.as_str(),
                    error.code,
                    error.message,
                    now
                ])?;
            }
        }
        self.stats.errors_written += errors.len() as u64;
        Ok(())
    }

    /// Apply directory subtree totals in a single set-based update.
    ///
    /// Directory rows are streamed to disk while walking, before their subtree
    /// is complete, so their totals arrive here at the end. Loading them into a
    /// temporary table and joining beats issuing one `UPDATE` per directory by
    /// roughly an order of magnitude on a large volume.
    pub fn apply_directory_aggregates(&mut self, aggregates: &[DirAggregate]) -> Result<()> {
        if aggregates.is_empty() {
            return Ok(());
        }
        self.commit()?;
        let tx: Transaction<'_> = self.conn.transaction()?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS dir_agg (
                 id        INTEGER PRIMARY KEY,
                 logical   INTEGER NOT NULL,
                 allocated INTEGER NOT NULL,
                 files     INTEGER NOT NULL,
                 dirs      INTEGER NOT NULL
             );
             DELETE FROM dir_agg;",
        )?;
        {
            let mut stmt = tx.prepare("INSERT INTO dir_agg VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for agg in aggregates {
                stmt.execute(params![
                    agg.id,
                    agg.logical,
                    agg.allocated,
                    agg.files,
                    agg.dirs
                ])?;
            }
        }
        tx.execute(
            "UPDATE fs_entries
                SET subtree_logical_size   = dir_agg.logical,
                    subtree_allocated_size = dir_agg.allocated,
                    subtree_file_count     = dir_agg.files,
                    subtree_dir_count      = dir_agg.dirs
               FROM dir_agg
              WHERE fs_entries.id = dir_agg.id",
            [],
        )?;
        tx.execute_batch("DROP TABLE IF EXISTS temp.dir_agg")?;
        tx.commit()?;
        self.stats.commits += 1;
        Ok(())
    }

    pub fn write_breakdowns(
        &mut self,
        categories: &HashMap<Category, (i64, i64)>,
        extensions: &HashMap<String, (i64, i64)>,
    ) -> Result<()> {
        self.commit()?;
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO scan_categories (scan_id, category, file_count, total_bytes)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(scan_id, category) DO UPDATE SET
                     file_count  = file_count + excluded.file_count,
                     total_bytes = total_bytes + excluded.total_bytes",
            )?;
            for (category, (count, bytes)) in categories {
                stmt.execute(params![self.scan_id, category.as_i64(), count, bytes])?;
            }
            let mut stmt = tx.prepare(
                "INSERT INTO scan_extensions (scan_id, extension, file_count, total_bytes)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(scan_id, extension) DO UPDATE SET
                     file_count  = file_count + excluded.file_count,
                     total_bytes = total_bytes + excluded.total_bytes",
            )?;
            for (extension, (count, bytes)) in extensions {
                stmt.execute(params![self.scan_id, extension, count, bytes])?;
            }
        }
        tx.commit()?;
        self.stats.commits += 1;
        Ok(())
    }

    /// Correct `accessible` for directories that turned out to be unreadable.
    ///
    /// The parent writes a child directory's row optimistically while listing,
    /// and only the worker that later opens the child discovers it cannot be
    /// read. These are rare (tens per volume, not thousands), so one targeted
    /// update at the end is cheaper than making the common path conditional.
    pub fn mark_inaccessible(&mut self, entry_ids: &[i64]) -> Result<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }
        self.commit()?;
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE fs_entries SET accessible = 0 WHERE id = ?1")?;
            for id in entry_ids {
                stmt.execute(params![id])?;
            }
        }
        tx.commit()?;
        self.stats.commits += 1;
        Ok(())
    }

    /// Build the query indexes. Called once, after the bulk load.
    ///
    /// `subtree_logical_size` carries the bytes attributed to an entry
    /// *including* its descendants, for every kind, so one index orders mixed
    /// directory-and-file listings without a second sort column.
    pub fn create_query_indexes(&mut self) -> Result<()> {
        self.commit()?;
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_entries_children
                 ON fs_entries(scan_id, parent_id, subtree_logical_size DESC);
             CREATE INDEX IF NOT EXISTS idx_entries_largest_files
                 ON fs_entries(scan_id, logical_size DESC) WHERE kind = 0;
             CREATE INDEX IF NOT EXISTS idx_entries_largest_dirs
                 ON fs_entries(scan_id, subtree_logical_size DESC) WHERE kind = 1;
             CREATE INDEX IF NOT EXISTS idx_entries_path
                 ON fs_entries(scan_id, path) WHERE path IS NOT NULL;",
        )?;
        Ok(())
    }

    /// Close out the scan row and hand the connection back.
    #[allow(clippy::too_many_arguments)]
    pub fn finish_scan(
        mut self,
        status: ScanStatus,
        root_entry_id: Option<i64>,
        duration_ms: i64,
        file_count: i64,
        dir_count: i64,
        symlink_count: i64,
        other_count: i64,
        error_count: i64,
        logical_bytes: i64,
        allocated_bytes: i64,
    ) -> Result<Connection> {
        self.commit()?;
        let entries = file_count + dir_count + symlink_count + other_count;
        let eps = if duration_ms > 0 {
            entries as f64 / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };
        self.conn.execute(
            "UPDATE scans SET
                 status = ?2, finished_at = ?3, duration_ms = ?4,
                 file_count = ?5, dir_count = ?6, symlink_count = ?7, other_count = ?8,
                 error_count = ?9, logical_bytes = ?10, allocated_bytes = ?11,
                 entries_per_second = ?12, root_entry_id = ?13
             WHERE id = ?1",
            params![
                self.scan_id,
                status.as_str(),
                now_unix(),
                duration_ms,
                file_count,
                dir_count,
                symlink_count,
                other_count,
                error_count,
                logical_bytes,
                allocated_bytes,
                eps,
                root_entry_id
            ],
        )?;
        // `PRAGMA optimize` lets SQLite refresh stale statistics for the
        // indexes we just created, which keeps the navigation queries on the
        // plan we designed for.
        let _ = self.conn.execute_batch("PRAGMA optimize;");
        Ok(self.conn)
    }

    /// Roll back cleanly after a fatal error, leaving the scan marked failed.
    pub fn abort(mut self, status: ScanStatus) -> Connection {
        if self.in_transaction {
            let _ = self.conn.execute_batch("COMMIT");
            self.in_transaction = false;
        }
        let _ = self.conn.execute(
            "UPDATE scans SET status = ?2, finished_at = ?3 WHERE id = ?1",
            params![self.scan_id, status.as_str(), now_unix()],
        );
        self.conn
    }
}

fn entry_insert_sql(rows: usize) -> String {
    let mut sql = String::with_capacity(128 + rows * ENTRY_COLUMN_COUNT * 6);
    sql.push_str("INSERT INTO fs_entries (");
    sql.push_str(ENTRY_COLUMNS);
    sql.push_str(") VALUES ");
    let mut index = 1;
    for row in 0..rows {
        if row > 0 {
            sql.push(',');
        }
        sql.push('(');
        for column in 0..ENTRY_COLUMN_COUNT {
            if column > 0 {
                sql.push(',');
            }
            sql.push('?');
            sql.push_str(&index.to_string());
            index += 1;
        }
        sql.push(')');
    }
    sql
}

fn push_entry_values(out: &mut Vec<rusqlite::types::Value>, scan_id: i64, record: &EntryRecord) {
    use rusqlite::types::Value;
    out.push(Value::Integer(record.id));
    out.push(Value::Integer(scan_id));
    out.push(record.parent_id.map(Value::Integer).unwrap_or(Value::Null));
    out.push(Value::Text(record.name.clone()));
    out.push(record.path.clone().map(Value::Text).unwrap_or(Value::Null));
    out.push(Value::Integer(record.depth as i64));
    out.push(Value::Integer(record.kind.as_i64()));
    out.push(Value::Integer(record.logical_size));
    out.push(Value::Integer(record.allocated_size));
    out.push(Value::Integer(record.subtree_logical_size));
    out.push(Value::Integer(record.subtree_allocated_size));
    out.push(Value::Integer(record.subtree_file_count));
    out.push(Value::Integer(record.subtree_dir_count));
    out.push(
        record
            .extension
            .clone()
            .map(Value::Text)
            .unwrap_or(Value::Null),
    );
    out.push(
        record
            .modified_at
            .map(Value::Integer)
            .unwrap_or(Value::Null),
    );
    out.push(record.created_at.map(Value::Integer).unwrap_or(Value::Null));
    out.push(
        record
            .accessed_at
            .map(Value::Integer)
            .unwrap_or(Value::Null),
    );
    out.push(Value::Integer(record.mode));
    out.push(Value::Integer(record.uid));
    out.push(Value::Integer(record.gid));
    out.push(Value::Integer(record.inode));
    out.push(Value::Integer(record.device));
    out.push(Value::Integer(record.nlink));
    out.push(Value::Integer(record.is_hidden as i64));
    out.push(Value::Integer(record.hardlink_dup as i64));
    out.push(Value::Integer(record.accessible as i64));
    out.push(Value::Integer(record.category.as_i64()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_sql_numbers_every_placeholder() {
        let sql = entry_insert_sql(2);
        assert!(sql.contains("?1,"));
        assert!(sql.ends_with(&format!("?{})", ENTRY_COLUMN_COUNT * 2)));
        assert_eq!(sql.matches('?').count(), ENTRY_COLUMN_COUNT * 2);
    }

    #[test]
    fn column_count_matches_the_column_list() {
        assert_eq!(ENTRY_COLUMNS.split(',').count(), ENTRY_COLUMN_COUNT);
    }
}
