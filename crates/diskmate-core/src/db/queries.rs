//! Read queries against the index.
//!
//! Every query here is bounded. The UI asks for a page of rows at a time and
//! never receives a tree; aggregation happens in SQL over indexes built for
//! exactly these shapes.
//!
//! File paths are not stored per row. They are reconstructed from the parent
//! directory's stored path with a single join, which is why every listing
//! query joins `fs_entries` to itself once.

use rusqlite::{params, Connection, Row};

use crate::error::{CoreError, Result};
use crate::model::{
    Category, EntryKind, EntryRow, ExtensionBucket, ScanError, ScanErrorKind, ScanStatus,
    ScanSummary,
};

/// Guard against a pathological or corrupt parent chain when walking upwards.
const MAX_BREADCRUMB_DEPTH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    Size,
    Name,
    Modified,
    Items,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

/// A page request for the storage explorer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildQuery {
    pub scan_id: i64,
    pub parent_id: i64,
    pub sort: SortKey,
    pub direction: SortDirection,
    pub limit: u32,
    pub offset: u32,
    /// Exclude dot-files and dot-directories.
    pub hide_hidden: bool,
}

impl ChildQuery {
    pub fn new(scan_id: i64, parent_id: i64) -> Self {
        Self {
            scan_id,
            parent_id,
            sort: SortKey::Size,
            direction: SortDirection::Desc,
            limit: 100,
            offset: 0,
            hide_hidden: false,
        }
    }

    /// Clamp a request from the UI to something the engine will answer.
    /// A caller asking for a million rows is a bug, not a feature.
    fn clamped_limit(&self) -> u32 {
        self.limit.clamp(1, 1_000)
    }

    fn order_clause(&self) -> &'static str {
        let dir = match self.direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        match (self.sort, dir) {
            // Directories sort above files at equal size so a listing reads the
            // way Finder does.
            (SortKey::Size, "ASC") => "ORDER BY e.subtree_logical_size ASC, e.name ASC",
            (SortKey::Size, _) => "ORDER BY e.subtree_logical_size DESC, e.name ASC",
            (SortKey::Name, "ASC") => "ORDER BY e.name COLLATE NOCASE ASC",
            (SortKey::Name, _) => "ORDER BY e.name COLLATE NOCASE DESC",
            (SortKey::Modified, "ASC") => "ORDER BY e.modified_at ASC NULLS LAST, e.name ASC",
            (SortKey::Modified, _) => "ORDER BY e.modified_at DESC NULLS LAST, e.name ASC",
            (SortKey::Items, "ASC") => {
                "ORDER BY (e.subtree_file_count + e.subtree_dir_count) ASC, e.name ASC"
            }
            (SortKey::Items, _) => {
                "ORDER BY (e.subtree_file_count + e.subtree_dir_count) DESC, e.name ASC"
            }
        }
    }
}

/// Volume capacity as recorded alongside a scan.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeUsage {
    pub volume_id: i64,
    pub volume_key: String,
    pub name: String,
    pub mount_point: String,
    pub total_bytes: Option<i64>,
    pub available_bytes: Option<i64>,
}

/// Namespace for the index's read queries.
pub struct StorageQueries;

/// Shared projection. `e` is the entry, `p` its parent directory.
///
/// Two different sizes matter here, because they answer different questions:
///
/// * `size` is what the entry *is*: a directory's subtree total, or a file's
///   own length. It is what a person expects to read next to a name.
/// * `share` comes from the *attributed* bytes, which exclude hard-link
///   repeats. That is what keeps the shares within a folder summing to one
///   when two of its names point at the same inode.
const ENTRY_SELECT: &str = "
    SELECT e.id, e.parent_id, e.name,
           COALESCE(e.path,
                    CASE WHEN p.path = '/' THEN '/' || e.name
                         ELSE p.path || '/' || e.name END,
                    e.name) AS full_path,
           e.kind, e.depth,
           CASE WHEN e.kind = 1 THEN e.subtree_logical_size ELSE e.logical_size END AS size,
           e.subtree_allocated_size,
           e.subtree_file_count, e.subtree_dir_count, e.extension, e.modified_at,
           e.category, e.is_hidden, e.accessible,
           CASE WHEN p.subtree_logical_size > 0
                THEN CAST(e.subtree_logical_size AS REAL) / p.subtree_logical_size
                ELSE 0.0 END AS share,
           e.hardlink_dup
      FROM fs_entries e
      LEFT JOIN fs_entries p ON p.id = e.parent_id";

fn map_entry_row(row: &Row<'_>) -> rusqlite::Result<EntryRow> {
    Ok(EntryRow {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        name: row.get(2)?,
        path: row.get(3)?,
        kind: EntryKind::from_i64(row.get(4)?),
        depth: row.get(5)?,
        size: row.get(6)?,
        allocated_size: row.get(7)?,
        file_count: row.get(8)?,
        dir_count: row.get(9)?,
        extension: row.get(10)?,
        modified_at: row.get(11)?,
        category: Category::from_i64(row.get(12)?),
        is_hidden: row.get::<_, i64>(13)? != 0,
        accessible: row.get::<_, i64>(14)? != 0,
        share_of_parent: row.get::<_, f64>(15)?.clamp(0.0, 1.0),
        shares_storage: row.get::<_, i64>(16)? != 0,
    })
}

impl StorageQueries {
    /// Immediate children of a directory, one page at a time.
    pub fn children(conn: &Connection, query: &ChildQuery) -> Result<Vec<EntryRow>> {
        let sql = format!(
            "{ENTRY_SELECT}
              WHERE e.scan_id = ?1 AND e.parent_id = ?2 {hidden}
              {order} LIMIT ?3 OFFSET ?4",
            hidden = if query.hide_hidden { "AND e.is_hidden = 0" } else { "" },
            order = query.order_clause(),
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt
            .query_map(
                params![query.scan_id, query.parent_id, query.clamped_limit(), query.offset],
                map_entry_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Number of immediate children, for pagination.
    pub fn child_count(conn: &Connection, scan_id: i64, parent_id: i64, hide_hidden: bool) -> Result<i64> {
        let sql = if hide_hidden {
            "SELECT count(*) FROM fs_entries WHERE scan_id = ?1 AND parent_id = ?2 AND is_hidden = 0"
        } else {
            "SELECT count(*) FROM fs_entries WHERE scan_id = ?1 AND parent_id = ?2"
        };
        Ok(conn.prepare_cached(sql)?.query_row(params![scan_id, parent_id], |r| r.get(0))?)
    }

    pub fn entry_by_id(conn: &Connection, scan_id: i64, entry_id: i64) -> Result<EntryRow> {
        let sql = format!("{ENTRY_SELECT} WHERE e.scan_id = ?1 AND e.id = ?2");
        conn.prepare_cached(&sql)?
            .query_row(params![scan_id, entry_id], map_entry_row)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => CoreError::UnknownEntry(entry_id),
                other => other.into(),
            })
    }

    /// Look up a directory by absolute path. Only directories store a path, so
    /// this is the navigation entry point (deep links, rescans, breadcrumbs).
    pub fn directory_by_path(conn: &Connection, scan_id: i64, path: &str) -> Result<Option<EntryRow>> {
        let sql = format!("{ENTRY_SELECT} WHERE e.scan_id = ?1 AND e.path = ?2");
        let mut stmt = conn.prepare_cached(&sql)?;
        let mut rows = stmt.query_map(params![scan_id, path], map_entry_row)?;
        rows.next().transpose().map_err(Into::into)
    }

    /// The scan's root directory row.
    pub fn root_entry(conn: &Connection, scan_id: i64) -> Result<EntryRow> {
        let sql = format!(
            "{ENTRY_SELECT} WHERE e.scan_id = ?1 AND e.parent_id IS NULL ORDER BY e.id LIMIT 1"
        );
        conn.prepare_cached(&sql)?
            .query_row(params![scan_id], map_entry_row)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => CoreError::UnknownScan(scan_id),
                other => other.into(),
            })
    }

    /// Ancestors of an entry, root first, for the breadcrumb bar.
    ///
    /// Walked iteratively rather than with a recursive CTE: the chain is at
    /// most a few dozen rows and each hop is a primary-key lookup, so this is
    /// both faster and easier to bound against a corrupt parent chain.
    pub fn ancestors(conn: &Connection, scan_id: i64, entry_id: i64) -> Result<Vec<EntryRow>> {
        let mut chain = Vec::new();
        let mut current = Some(entry_id);
        let mut guard = 0usize;
        while let Some(id) = current {
            if guard >= MAX_BREADCRUMB_DEPTH {
                break;
            }
            guard += 1;
            let row = Self::entry_by_id(conn, scan_id, id)?;
            current = row.parent_id;
            chain.push(row);
        }
        chain.reverse();
        Ok(chain)
    }

    /// Largest individual files across the whole scan.
    pub fn largest_files(conn: &Connection, scan_id: i64, limit: u32) -> Result<Vec<EntryRow>> {
        let sql = format!(
            "{ENTRY_SELECT} WHERE e.scan_id = ?1 AND e.kind = 0
             ORDER BY e.logical_size DESC LIMIT ?2"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt
            .query_map(params![scan_id, limit.clamp(1, 1_000)], map_entry_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Largest directories by subtree size, with pass-through folders removed.
    ///
    /// Every ancestor of a big folder is also big, so the naive query returns
    /// the same bytes five times over as a parent chain: `home`, `user`,
    /// `project`, `target`, `debug`. None of those rows except the last tells
    /// anyone anything.
    ///
    /// A folder is excluded when a single child folder accounts for nearly all
    /// of it, because then the parent is a container rather than the thing
    /// using the space. What survives is the deepest folder where the space
    /// actually spreads out, which is the folder a person would open.
    ///
    /// The `NOT EXISTS` is an index seek on `idx_entries_children` per
    /// candidate, and candidates are visited in size order, so this typically
    /// touches a few dozen rows before the limit is met.
    pub fn largest_directories(
        conn: &Connection,
        scan_id: i64,
        limit: u32,
        min_depth: i32,
    ) -> Result<Vec<EntryRow>> {
        let sql = format!(
            "{ENTRY_SELECT}
              WHERE e.scan_id = ?1 AND e.kind = 1 AND e.depth >= ?3
                AND e.subtree_logical_size > 0
                AND NOT EXISTS (
                      SELECT 1 FROM fs_entries c
                       WHERE c.scan_id = e.scan_id
                         AND c.parent_id = e.id
                         AND c.kind = 1
                         AND c.subtree_logical_size * 100 >= e.subtree_logical_size * 85
                    )
             ORDER BY e.subtree_logical_size DESC LIMIT ?2"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt
            .query_map(params![scan_id, limit.clamp(1, 1_000), min_depth], map_entry_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// File-type breakdown, read from the totals accumulated during the scan.
    pub fn extension_breakdown(
        conn: &Connection,
        scan_id: i64,
        limit: u32,
    ) -> Result<Vec<ExtensionBucket>> {
        let mut stmt = conn.prepare_cached(
            "SELECT extension, file_count, total_bytes FROM scan_extensions
              WHERE scan_id = ?1 ORDER BY total_bytes DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![scan_id, limit.clamp(1, 500)], |row| {
                let extension: String = row.get(0)?;
                Ok(ExtensionBucket {
                    extension: if extension.is_empty() { None } else { Some(extension) },
                    file_count: row.get(1)?,
                    total_size: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn category_breakdown(conn: &Connection, scan_id: i64) -> Result<Vec<(Category, i64, i64)>> {
        let mut stmt = conn.prepare_cached(
            "SELECT category, file_count, total_bytes FROM scan_categories
              WHERE scan_id = ?1 ORDER BY total_bytes DESC",
        )?;
        let rows = stmt
            .query_map(params![scan_id], |row| {
                Ok((Category::from_i64(row.get(0)?), row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Errors recorded during a scan, most useful kinds first.
    pub fn scan_errors(conn: &Connection, scan_id: i64, limit: u32) -> Result<Vec<ScanError>> {
        let mut stmt = conn.prepare_cached(
            "SELECT path, kind, code, message FROM scan_errors
              WHERE scan_id = ?1 ORDER BY id LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![scan_id, limit.clamp(1, 5_000)], |row| {
                Ok(ScanError {
                    path: row.get(0)?,
                    kind: ScanErrorKind::from_stored(&row.get::<_, String>(1)?),
                    code: row.get(2)?,
                    message: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Error counts grouped by kind, so the UI can say "18 folders needed
    /// permission" without loading every error row.
    pub fn error_summary(conn: &Connection, scan_id: i64) -> Result<Vec<(ScanErrorKind, i64)>> {
        let mut stmt = conn.prepare_cached(
            "SELECT kind, count(*) FROM scan_errors WHERE scan_id = ?1 GROUP BY kind ORDER BY 2 DESC",
        )?;
        let rows = stmt
            .query_map(params![scan_id], |row| {
                Ok((ScanErrorKind::from_stored(&row.get::<_, String>(0)?), row.get(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn scan_summary(conn: &Connection, scan_id: i64) -> Result<ScanSummary> {
        let mut stmt = conn.prepare_cached(&format!("{SCAN_SELECT} WHERE id = ?1"))?;
        stmt.query_row(params![scan_id], map_scan_summary).map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => CoreError::UnknownScan(scan_id),
            other => other.into(),
        })
    }

    /// Most recent completed or cancelled scan for a root path. Cancelled scans
    /// are included because a partial index is still useful, and the UI labels
    /// it as incomplete.
    pub fn latest_scan_for_root(conn: &Connection, root: &str) -> Result<Option<ScanSummary>> {
        let mut stmt = conn.prepare_cached(&format!(
            "{SCAN_SELECT} WHERE root_path = ?1 AND status IN ('completed', 'cancelled')
             ORDER BY started_at DESC, id DESC LIMIT 1"
        ))?;
        let mut rows = stmt.query_map(params![root], map_scan_summary)?;
        rows.next().transpose().map_err(Into::into)
    }

    pub fn recent_scans(conn: &Connection, limit: u32) -> Result<Vec<ScanSummary>> {
        let mut stmt = conn
            .prepare_cached(&format!("{SCAN_SELECT} ORDER BY started_at DESC, id DESC LIMIT ?1"))?;
        let rows = stmt
            .query_map(params![limit.clamp(1, 200)], map_scan_summary)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Mark scans left in `running` by a previous process as interrupted.
    /// Called once at startup so the index never claims a scan is in flight.
    pub fn reap_interrupted_scans(conn: &Connection) -> Result<usize> {
        let changed = conn.execute(
            "UPDATE scans SET status = ?1, finished_at = ?2 WHERE status = 'running'",
            params![ScanStatus::Interrupted.as_str(), super::now_unix()],
        )?;
        Ok(changed)
    }

    /// Delete a scan and everything derived from it.
    pub fn delete_scan(conn: &Connection, scan_id: i64) -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        for sql in [
            "DELETE FROM fs_entries WHERE scan_id = ?1",
            "DELETE FROM scan_errors WHERE scan_id = ?1",
            "DELETE FROM scan_categories WHERE scan_id = ?1",
            "DELETE FROM scan_extensions WHERE scan_id = ?1",
            "DELETE FROM content_hashes WHERE scan_id = ?1",
            "DELETE FROM file_classifications WHERE scan_id = ?1",
            "DELETE FROM cleanup_candidates WHERE scan_id = ?1",
            "DELETE FROM scans WHERE id = ?1",
        ] {
            tx.execute(sql, params![scan_id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Keep only the newest `keep` scans per root path.
    ///
    /// Without this the index grows without bound across rescans. Snapshots and
    /// scan comparisons will later pin the scans they need.
    pub fn prune_scans(conn: &Connection, keep_per_root: u32) -> Result<usize> {
        let stale: Vec<i64> = conn
            .prepare(
                "SELECT id FROM (
                     SELECT id, row_number() OVER (
                         PARTITION BY root_path ORDER BY started_at DESC, id DESC
                     ) AS rank FROM scans
                 ) WHERE rank > ?1",
            )?
            .query_map(params![keep_per_root.max(1)], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for scan_id in &stale {
            Self::delete_scan(conn, *scan_id)?;
        }
        Ok(stale.len())
    }
}

const SCAN_SELECT: &str = "
    SELECT id, volume_id, root_path, status, started_at, finished_at, duration_ms,
           file_count, dir_count, symlink_count, other_count, error_count,
           logical_bytes, allocated_bytes, entries_per_second, engine_version
      FROM scans";

fn map_scan_summary(row: &Row<'_>) -> rusqlite::Result<ScanSummary> {
    Ok(ScanSummary {
        scan_id: row.get(0)?,
        volume_id: row.get(1)?,
        root_path: row.get(2)?,
        status: ScanStatus::from_stored(&row.get::<_, String>(3)?),
        started_at: row.get(4)?,
        finished_at: row.get(5)?,
        duration_ms: row.get(6)?,
        file_count: row.get(7)?,
        dir_count: row.get(8)?,
        symlink_count: row.get(9)?,
        other_count: row.get(10)?,
        error_count: row.get(11)?,
        logical_bytes: row.get(12)?,
        allocated_bytes: row.get(13)?,
        entries_per_second: row.get(14)?,
        index_bytes: 0,
        engine_version: row.get(15)?,
    })
}
