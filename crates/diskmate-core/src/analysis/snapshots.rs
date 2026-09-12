//! Snapshots, and what changed between two of them.
//!
//! A storage tool that only shows the present answers "what is big". A tool
//! that remembers answers "what got big", which is the question people
//! actually have when a disk fills up over a month.
//!
//! A snapshot is deliberately small: the totals of a scan plus its identity.
//! The comparison reads the two scans' directory rows, so the detail lives in
//! the index and is not duplicated per snapshot.

use rusqlite::{params, Connection};

use crate::db;
use crate::error::{CoreError, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: i64,
    pub scan_id: i64,
    pub label: String,
    pub root_path: String,
    pub captured_at: i64,
    pub indexed_bytes: i64,
    pub entry_count: i64,
    pub file_count: i64,
    pub dir_count: i64,
    /// Capacity of the volume at the time, where it was known.
    pub total_bytes: Option<i64>,
    pub used_bytes: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Grew,
    Shrank,
    Added,
    Removed,
}

impl ChangeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Grew => "grew",
            Self::Shrank => "shrank",
            Self::Added => "added",
            Self::Removed => "removed",
        }
    }

    fn from_stored(value: &str) -> Self {
        match value {
            "shrank" => Self::Shrank,
            "added" => Self::Added,
            "removed" => Self::Removed,
            _ => Self::Grew,
        }
    }
}

/// One folder that differs between two scans.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub path: String,
    pub name: String,
    pub change: ChangeKind,
    pub before_bytes: i64,
    pub after_bytes: i64,
    /// Positive when it grew. This is the column worth sorting on.
    pub delta_bytes: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub id: i64,
    pub base: Snapshot,
    pub target: Snapshot,
    pub bytes_added: i64,
    pub bytes_removed: i64,
    pub entries_added: i64,
    pub entries_removed: i64,
    /// The net change, which is what a person means by "it grew by".
    pub net_bytes: i64,
    pub deltas: Vec<Delta>,
}

/// Folders smaller than this are noise in a comparison.
const MIN_DELTA_BYTES: i64 = 1000 * 1000;

pub struct Snapshots;

impl Snapshots {
    /// Record a scan as a snapshot.
    pub fn capture(conn: &Connection, scan_id: i64, label: &str) -> Result<Snapshot> {
        let summary = db::StorageQueries::scan_summary(conn, scan_id)?;
        let volume = conn
            .query_row(
                "SELECT total_bytes, available_bytes FROM volumes WHERE id = (
                     SELECT volume_id FROM scans WHERE id = ?1
                 )",
                params![scan_id],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .ok();
        let (total_bytes, used_bytes) = match volume {
            Some((Some(total), Some(available))) => (Some(total), Some(total - available)),
            _ => (None, None),
        };

        conn.execute(
            "INSERT INTO snapshots
                 (scan_id, label, captured_at, total_bytes, used_bytes, indexed_bytes,
                  entry_count, root_path, file_count, dir_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                scan_id,
                label,
                db::now_unix(),
                total_bytes,
                used_bytes,
                summary.logical_bytes,
                summary.entry_count(),
                summary.root_path,
                summary.file_count,
                summary.dir_count
            ],
        )?;
        Self::snapshot(conn, conn.last_insert_rowid())
    }

    pub fn snapshot(conn: &Connection, id: i64) -> Result<Snapshot> {
        conn.query_row(
            "SELECT id, scan_id, label, root_path, captured_at, indexed_bytes, entry_count,
                    file_count, dir_count, total_bytes, used_bytes
               FROM snapshots WHERE id = ?1",
            params![id],
            map_snapshot,
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => {
                CoreError::invalid(format!("snapshot {id} does not exist"))
            }
            other => other.into(),
        })
    }

    pub fn list(conn: &Connection, limit: u32) -> Result<Vec<Snapshot>> {
        let mut stmt = conn.prepare(
            "SELECT id, scan_id, label, root_path, captured_at, indexed_bytes, entry_count,
                    file_count, dir_count, total_bytes, used_bytes
               FROM snapshots ORDER BY captured_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit.clamp(1, 200)], map_snapshot)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("DELETE FROM snapshots WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Compare two snapshots and record the result.
    ///
    /// Folders are matched by absolute path, which is why directories store one
    /// and files do not: a file that moved is a removal and an addition, but a
    /// folder that grew is the finding worth reporting.
    pub fn compare(conn: &Connection, base_id: i64, target_id: i64) -> Result<Comparison> {
        let base = Self::snapshot(conn, base_id)?;
        let target = Self::snapshot(conn, target_id)?;
        if base.root_path != target.root_path {
            return Err(CoreError::invalid(
                "those two snapshots are of different folders, so comparing them \
                 would not mean anything",
            ));
        }

        // Presence and size are different facts, and conflating them reports
        // an emptied folder as a deleted one. `a.path IS NOT NULL` says the
        // folder existed; its size says how much was in it.
        let mut stmt = conn.prepare(
            "SELECT COALESCE(b.path, a.path) AS path,
                    COALESCE(b.name, a.name) AS name,
                    a.path IS NOT NULL AS existed_before,
                    b.path IS NOT NULL AS exists_after,
                    COALESCE(a.subtree_logical_size, 0) AS before_bytes,
                    COALESCE(b.subtree_logical_size, 0) AS after_bytes
               FROM (SELECT path, name, subtree_logical_size FROM fs_entries
                      WHERE scan_id = ?1 AND kind = 1 AND path IS NOT NULL) a
               FULL OUTER JOIN
                    (SELECT path, name, subtree_logical_size FROM fs_entries
                      WHERE scan_id = ?2 AND kind = 1 AND path IS NOT NULL) b
                 ON a.path = b.path
              WHERE abs(COALESCE(b.subtree_logical_size, 0)
                        - COALESCE(a.subtree_logical_size, 0)) >= ?3
              ORDER BY abs(COALESCE(b.subtree_logical_size, 0)
                           - COALESCE(a.subtree_logical_size, 0)) DESC
              LIMIT 500",
        )?;
        let deltas = stmt
            .query_map(
                params![base.scan_id, target.scan_id, MIN_DELTA_BYTES],
                |row| {
                    let path: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let existed_before: bool = row.get::<_, i64>(2)? != 0;
                    let exists_after: bool = row.get::<_, i64>(3)? != 0;
                    let before: i64 = row.get(4)?;
                    let after: i64 = row.get(5)?;
                    let change = match (existed_before, exists_after) {
                        (false, true) => ChangeKind::Added,
                        (true, false) => ChangeKind::Removed,
                        // Still there either way: it only grew or shrank, even if
                        // what is left is nothing at all.
                        _ if after > before => ChangeKind::Grew,
                        _ => ChangeKind::Shrank,
                    };
                    Ok(Delta {
                        path,
                        name,
                        change,
                        before_bytes: before,
                        after_bytes: after,
                        delta_bytes: after - before,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let bytes_added: i64 = deltas
            .iter()
            .filter(|d| d.delta_bytes > 0)
            .map(|d| d.delta_bytes)
            .sum();
        let bytes_removed: i64 = deltas
            .iter()
            .filter(|d| d.delta_bytes < 0)
            .map(|d| -d.delta_bytes)
            .sum();
        let entries_added = target.entry_count - base.entry_count;

        conn.execute(
            "INSERT INTO scan_comparisons
                 (base_scan_id, target_scan_id, created_at, bytes_added, bytes_removed,
                  entries_added, entries_removed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(base_scan_id, target_scan_id) DO UPDATE SET
                 created_at = excluded.created_at,
                 bytes_added = excluded.bytes_added,
                 bytes_removed = excluded.bytes_removed,
                 entries_added = excluded.entries_added,
                 entries_removed = excluded.entries_removed",
            params![
                base.scan_id,
                target.scan_id,
                db::now_unix(),
                bytes_added,
                bytes_removed,
                entries_added.max(0),
                (-entries_added).max(0)
            ],
        )?;
        let comparison_id: i64 = conn.query_row(
            "SELECT id FROM scan_comparisons WHERE base_scan_id = ?1 AND target_scan_id = ?2",
            params![base.scan_id, target.scan_id],
            |row| row.get(0),
        )?;

        conn.execute(
            "DELETE FROM snapshot_deltas WHERE comparison_id = ?1",
            params![comparison_id],
        )?;
        {
            let mut stmt = conn.prepare(
                "INSERT INTO snapshot_deltas
                     (comparison_id, path, name, change, before_bytes, after_bytes, delta_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for delta in &deltas {
                stmt.execute(params![
                    comparison_id,
                    delta.path,
                    delta.name,
                    delta.change.as_str(),
                    delta.before_bytes,
                    delta.after_bytes,
                    delta.delta_bytes
                ])?;
            }
        }

        Ok(Comparison {
            id: comparison_id,
            net_bytes: target.indexed_bytes - base.indexed_bytes,
            base,
            target,
            bytes_added,
            bytes_removed,
            entries_added: entries_added.max(0),
            entries_removed: (-entries_added).max(0),
            deltas,
        })
    }

    /// Read back a stored comparison without recomputing it.
    pub fn stored_deltas(conn: &Connection, comparison_id: i64) -> Result<Vec<Delta>> {
        let mut stmt = conn.prepare(
            "SELECT path, name, change, before_bytes, after_bytes, delta_bytes
               FROM snapshot_deltas WHERE comparison_id = ?1
              ORDER BY delta_bytes DESC",
        )?;
        let rows = stmt
            .query_map(params![comparison_id], |row| {
                Ok(Delta {
                    path: row.get(0)?,
                    name: row.get(1)?,
                    change: ChangeKind::from_stored(&row.get::<_, String>(2)?),
                    before_bytes: row.get(3)?,
                    after_bytes: row.get(4)?,
                    delta_bytes: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

fn map_snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<Snapshot> {
    Ok(Snapshot {
        id: row.get(0)?,
        scan_id: row.get(1)?,
        label: row.get(2)?,
        root_path: row.get(3)?,
        captured_at: row.get(4)?,
        indexed_bytes: row.get(5)?,
        entry_count: row.get(6)?,
        file_count: row.get(7)?,
        dir_count: row.get(8)?,
        total_bytes: row.get(9)?,
        used_bytes: row.get(10)?,
    })
}
