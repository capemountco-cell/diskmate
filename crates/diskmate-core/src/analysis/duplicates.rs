//! Duplicate detection.
//!
//! Hashing every file is the obvious approach and the wrong one: it reads the
//! whole volume to discover that almost nothing matches. The work is arranged
//! so that each stage is cheaper than the one after it and discards most of
//! what reaches it.
//!
//! ```text
//!   every indexed file
//!         ↓  group by logical size      index-only, reads nothing
//!         ↓  discard unique sizes       removes the large majority
//!         ↓  partial digest             reads a few KB per file
//!         ↓  discard mismatches         removes almost all of the rest
//!         ↓  full digest                reads whole files, rarely
//!         ↓  confirmed duplicate groups
//! ```
//!
//! Two rules that are not negotiable:
//!
//! * **A filename never establishes that two files are duplicates.** Two files
//!   called `invoice.pdf` are two files called `invoice.pdf`.
//! * **A hard link is not a duplicate.** Two names for one inode already share
//!   their bytes; reporting them as reclaimable would promise space that does
//!   not exist.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use rusqlite::{params, Connection};

use crate::db;
use crate::error::Result;
use crate::model::SafetyClass;
use crate::scanner::CancellationToken;

/// Bytes read from each end of a file for the partial stage. Large enough that
/// two different files of the same size almost never collide, small enough
/// that the read is one disk seek rather than a full pass.
const PARTIAL_WINDOW: usize = 16 * 1024;
/// Below this, duplicates are not worth reporting. A thousand identical
/// 2 KB icons is a true fact and a useless one.
const MIN_DUPLICATE_BYTES: i64 = 1000 * 1000;
const ALGORITHM: &str = "blake3";

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateStats {
    /// Files that shared a size with at least one other file.
    pub size_candidates: i64,
    /// Files that needed a partial read.
    pub partially_hashed: i64,
    /// Files that needed a full read.
    pub fully_hashed: i64,
    pub groups: i64,
    /// Bytes held by copies beyond the first in each group.
    pub reclaimable_bytes: i64,
    pub bytes_read: i64,
}

/// One confirmed set of byte-for-byte identical files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: i64,
    pub logical_size: i64,
    pub member_count: i64,
    /// What would be freed by keeping one copy and removing the rest.
    pub reclaimable: i64,
    pub members: Vec<DuplicateMember>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateMember {
    pub entry_id: i64,
    pub path: String,
    pub name: String,
    pub modified_at: Option<i64>,
    /// The copy DiskMate suggests keeping: the oldest, on the grounds that it
    /// is most likely the original and least likely to be a stray copy.
    pub is_keeper: bool,
}

pub struct DuplicateFinder;

impl DuplicateFinder {
    /// Find duplicates among a scan's files and record the confirmed groups.
    ///
    /// `progress` is called with the stage name and how far through it is, so a
    /// long run over a large volume is legible rather than a spinner.
    pub fn run(
        conn: &Connection,
        scan_id: i64,
        cancel: &CancellationToken,
        mut progress: impl FnMut(&str, i64, i64),
    ) -> Result<DuplicateStats> {
        let mut stats = DuplicateStats::default();

        // Stage 1: size grouping. Pure index work, no file is opened.
        let by_size = Self::sizes_with_more_than_one_file(conn, scan_id)?;
        stats.size_candidates = by_size.iter().map(|(_, files)| files.len() as i64).sum();
        progress(
            "grouping by size",
            stats.size_candidates,
            stats.size_candidates,
        );
        if by_size.is_empty() {
            return Ok(stats);
        }

        // Stage 2: partial digests. A few KB per file discards nearly
        // everything that merely shares a size.
        let mut partial_groups: HashMap<(i64, [u8; 32]), Vec<FileRef>> = HashMap::new();
        let total = stats.size_candidates;
        let mut done = 0i64;
        for (size, files) in &by_size {
            for file in files {
                if cancel.is_cancelled() {
                    return Ok(stats);
                }
                done += 1;
                if done % 500 == 0 {
                    progress("reading a sample of each file", done, total);
                }
                match partial_digest(&file.path, *size) {
                    Ok((digest, read)) => {
                        stats.partially_hashed += 1;
                        stats.bytes_read += read;
                        partial_groups
                            .entry((*size, digest))
                            .or_default()
                            .push(file.clone());
                    }
                    // A file that cannot be read is not a duplicate finding.
                    // Skipping it silently is correct here: the scan already
                    // recorded why it was unreadable.
                    Err(_) => continue,
                }
            }
        }
        partial_groups.retain(|_, files| files.len() > 1);

        // Stage 3: full digests, only for what survived.
        let mut full_groups: HashMap<(i64, [u8; 32]), Vec<FileRef>> = HashMap::new();
        let remaining: i64 = partial_groups.values().map(|f| f.len() as i64).sum();
        let mut done = 0i64;
        for ((size, _), files) in &partial_groups {
            for file in files {
                if cancel.is_cancelled() {
                    return Ok(stats);
                }
                done += 1;
                if done % 50 == 0 {
                    progress("confirming byte for byte", done, remaining);
                }
                match full_digest(&file.path) {
                    Ok((digest, read)) => {
                        stats.fully_hashed += 1;
                        stats.bytes_read += read;
                        full_groups
                            .entry((*size, digest))
                            .or_default()
                            .push(file.clone());
                    }
                    Err(_) => continue,
                }
            }
        }
        full_groups.retain(|_, files| files.len() > 1);

        Self::record(conn, scan_id, &full_groups, &mut stats)?;
        progress("done", 1, 1);
        Ok(stats)
    }

    /// Files whose logical size is shared with at least one other file.
    ///
    /// Hard-link repeats are excluded here rather than later: they already
    /// share their bytes, so calling them duplicates would offer space back
    /// that removing them cannot produce.
    fn sizes_with_more_than_one_file(
        conn: &Connection,
        scan_id: i64,
    ) -> Result<Vec<(i64, Vec<FileRef>)>> {
        let mut stmt = conn.prepare(
            "SELECT e.logical_size, e.id,
                    COALESCE(p.path || '/' || e.name, e.name) AS full_path,
                    e.modified_at
               FROM fs_entries e
               LEFT JOIN fs_entries p ON p.id = e.parent_id
              WHERE e.scan_id = ?1
                AND e.kind = 0
                AND e.hardlink_dup = 0
                AND e.logical_size >= ?2
                AND e.logical_size IN (
                      SELECT logical_size FROM fs_entries
                       WHERE scan_id = ?1 AND kind = 0 AND hardlink_dup = 0
                         AND logical_size >= ?2
                       GROUP BY logical_size HAVING count(*) > 1
                  )
              ORDER BY e.logical_size DESC",
        )?;
        let rows = stmt
            .query_map(params![scan_id, MIN_DUPLICATE_BYTES], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    FileRef {
                        entry_id: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        modified_at: row.get(3)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut grouped: Vec<(i64, Vec<FileRef>)> = Vec::new();
        for (size, file) in rows {
            match grouped.last_mut() {
                Some((last_size, files)) if *last_size == size => files.push(file),
                _ => grouped.push((size, vec![file])),
            }
        }
        Ok(grouped)
    }

    fn record(
        conn: &Connection,
        scan_id: i64,
        groups: &HashMap<(i64, [u8; 32]), Vec<FileRef>>,
        stats: &mut DuplicateStats,
    ) -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM duplicate_groups WHERE scan_id = ?1",
            params![scan_id],
        )?;
        let now = db::now_unix();

        for ((size, digest), files) in groups {
            let member_count = files.len() as i64;
            let reclaimable = size * (member_count - 1);
            stats.groups += 1;
            stats.reclaimable_bytes += reclaimable;

            tx.execute(
                "INSERT INTO duplicate_groups
                     (scan_id, logical_size, member_count, reclaimable, algorithm, confirmed,
                      created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
                params![scan_id, size, member_count, reclaimable, ALGORITHM, now],
            )?;
            let group_id = tx.last_insert_rowid();

            // Keep the oldest copy. It is the one most likely to be the
            // original and least likely to be a stray duplicate somebody made
            // without meaning to.
            let keeper = files
                .iter()
                .enumerate()
                .min_by_key(|(_, f)| f.modified_at.unwrap_or(i64::MAX))
                .map(|(index, _)| index)
                .unwrap_or(0);

            for (index, file) in files.iter().enumerate() {
                tx.execute(
                    "INSERT OR REPLACE INTO duplicate_group_members (group_id, entry_id, is_keeper)
                     VALUES (?1, ?2, ?3)",
                    params![group_id, file.entry_id, i64::from(index == keeper)],
                )?;
                tx.execute(
                    "INSERT OR REPLACE INTO content_hashes
                         (entry_id, scan_id, kind, algorithm, offset, length, digest)
                     VALUES (?1, ?2, 'full', ?3, 0, ?4, ?5)",
                    params![file.entry_id, scan_id, ALGORITHM, size, &digest[..]],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Confirmed groups, largest waste first.
    pub fn groups(conn: &Connection, scan_id: i64, limit: u32) -> Result<Vec<DuplicateGroup>> {
        let mut stmt = conn.prepare(
            "SELECT id, logical_size, member_count, reclaimable
               FROM duplicate_groups
              WHERE scan_id = ?1 AND confirmed = 1
              ORDER BY reclaimable DESC LIMIT ?2",
        )?;
        let headers = stmt
            .query_map(params![scan_id, limit.clamp(1, 500)], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut out = Vec::with_capacity(headers.len());
        for (id, logical_size, member_count, reclaimable) in headers {
            let mut stmt = conn.prepare(
                "SELECT m.entry_id,
                        COALESCE(p.path || '/' || e.name, e.name),
                        e.name, e.modified_at, m.is_keeper
                   FROM duplicate_group_members m
                   JOIN fs_entries e ON e.id = m.entry_id
                   LEFT JOIN fs_entries p ON p.id = e.parent_id
                  WHERE m.group_id = ?1
                  ORDER BY m.is_keeper DESC, e.modified_at",
            )?;
            let members = stmt
                .query_map(params![id], |row| {
                    Ok(DuplicateMember {
                        entry_id: row.get(0)?,
                        path: row.get(1)?,
                        name: row.get(2)?,
                        modified_at: row.get(3)?,
                        is_keeper: row.get::<_, i64>(4)? != 0,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            out.push(DuplicateGroup {
                id,
                logical_size,
                member_count,
                reclaimable,
                members,
            });
        }
        Ok(out)
    }

    /// Turn confirmed groups into cleanup candidates: every copy except the
    /// keeper, each explaining which file it duplicates.
    pub fn as_cleanup_candidates(
        conn: &Connection,
        scan_id: i64,
        limit: u32,
    ) -> Result<Vec<crate::model::CleanupCandidate>> {
        let mut out = Vec::new();
        for group in Self::groups(conn, scan_id, limit)? {
            let keeper = group.members.iter().find(|m| m.is_keeper);
            let keeper_path = keeper.map(|k| k.path.clone()).unwrap_or_default();
            for member in group.members.iter().filter(|m| !m.is_keeper) {
                out.push(crate::model::CleanupCandidate {
                    id: 0,
                    scan_id,
                    entry_id: Some(member.entry_id),
                    path: member.path.clone(),
                    name: member.name.clone(),
                    kind: crate::model::CandidateKind::DuplicateCopy,
                    safety: SafetyClass::Review,
                    rule: "duplicate".into(),
                    reason: format!(
                        "{} is byte-for-byte identical to a copy DiskMate is keeping.",
                        member.name
                    ),
                    detail: Some(format!("The copy being kept is {keeper_path}.")),
                    bytes: group.logical_size,
                    modified_at: member.modified_at,
                    state: crate::model::CandidateState::Proposed,
                    group_key: Some(format!("duplicate:{}", group.id)),
                });
            }
        }
        Ok(out)
    }
}

/// A file in flight through the pipeline. Deliberately minimal: this is held
/// for every size-matched file on the volume, and the display name is read
/// back from the index when groups are queried.
#[derive(Debug, Clone)]
struct FileRef {
    entry_id: i64,
    path: PathBuf,
    modified_at: Option<i64>,
}

/// Digest of the first and last window of a file, plus its size.
///
/// Both ends matter: files that share a header (a container format, a common
/// preamble) are common, and files that share both a header and a trailer of
/// the same length rarely differ in between.
fn partial_digest(path: &std::path::Path, size: i64) -> std::io::Result<([u8; 32], i64)> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());

    let window = PARTIAL_WINDOW.min(size.max(0) as usize);
    let mut buffer = vec![0u8; window];
    let read = read_fully(&mut file, &mut buffer)?;
    hasher.update(&buffer[..read]);
    let mut total = read as i64;

    if size as usize > window * 2 {
        file.seek(SeekFrom::End(-(window as i64)))?;
        let read = read_fully(&mut file, &mut buffer)?;
        hasher.update(&buffer[..read]);
        total += read as i64;
    }
    Ok((*hasher.finalize().as_bytes(), total))
}

fn full_digest(path: &std::path::Path) -> std::io::Result<([u8; 32], i64)> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 128 * 1024];
    let mut total = 0i64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as i64;
    }
    Ok((*hasher.finalize().as_bytes(), total))
}

fn read_fully(file: &mut std::fs::File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}
