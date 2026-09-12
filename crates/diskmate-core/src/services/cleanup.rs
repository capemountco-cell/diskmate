//! The one place DiskMate removes anything.
//!
//! No other module in this crate touches a file destructively, and none should
//! be added. Every feature that wants to remove something — the staged queue,
//! the uninstaller, duplicate cleanup — proposes candidates and routes them
//! through [`CleanupService::remove_approved`].
//!
//! The flow, and what each step is actually for:
//!
//! ```text
//!   CleanupCandidate         a proposal carrying a reason and a risk class
//!         ↓  stage           an explicit act by the user, never automatic
//!         ↓  confirm         the ids and total the user actually saw
//!         ↓  re-validate     still there? still the same? still not protected?
//!         ↓  move to Trash   recoverable, not unlinked
//!         ↓  verify          confirm it actually left
//!         ↓  record          audit log, kept beyond the scan
//! ```
//!
//! Two properties are worth stating plainly because they are the difference
//! between a cleaner people trust and one they don't:
//!
//! * **A `Protected` candidate is never removed.** Not with a flag, not with a
//!   force option, not by a caller that thinks it knows better. The check is
//!   here, at the boundary, and there is no way past it.
//! * **The approval is matched against the queue.** The user confirms a
//!   specific set of ids and a specific total. If either has changed since they
//!   looked, the whole operation is refused rather than partially applied.

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};

use crate::db;
use crate::error::{CoreError, Result};
use crate::model::{
    CandidateGroup, CandidateKind, CandidateState, CleanupCandidate, Confirmation, RemovalOutcome,
    RemovalReport, SafetyClass,
};
use crate::platform;

/// How a file actually leaves.
///
/// Behind a trait for one reason: it makes "there is exactly one destructive
/// operation in DiskMate" a fact the type system can show, and it lets the
/// refusal logic be tested exhaustively without a test ever being able to
/// delete something by accident.
pub trait Remover: Send + Sync {
    fn move_to_trash(&self, path: &Path) -> std::result::Result<(), String>;
    fn name(&self) -> &'static str;
}

/// The real one. Uses the platform's Trash, so everything stays recoverable.
#[derive(Debug, Default)]
pub struct TrashRemover;

impl Remover for TrashRemover {
    fn move_to_trash(&self, path: &Path) -> std::result::Result<(), String> {
        trash::delete(path).map_err(|e| e.to_string())
    }

    fn name(&self) -> &'static str {
        "trash"
    }
}

/// Records what would have happened and touches nothing. Used by the tests
/// that exercise the refusal rules, so a bug in those tests cannot destroy
/// anything.
#[derive(Debug, Default)]
pub struct DryRunRemover {
    pub attempted: Mutex<Vec<std::path::PathBuf>>,
}

impl Remover for DryRunRemover {
    fn move_to_trash(&self, path: &Path) -> std::result::Result<(), String> {
        self.attempted
            .lock()
            .expect("dry run log poisoned")
            .push(path.to_path_buf());
        Ok(())
    }

    fn name(&self) -> &'static str {
        "dry-run"
    }
}

pub struct CleanupService {
    remover: Arc<dyn Remover>,
}

impl Default for CleanupService {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupService {
    pub fn new() -> Self {
        Self {
            remover: Arc::new(TrashRemover),
        }
    }

    pub fn with_remover(remover: Arc<dyn Remover>) -> Self {
        Self { remover }
    }

    pub fn remover_name(&self) -> &'static str {
        self.remover.name()
    }

    /// Record proposed candidates, replacing any earlier proposal for the same
    /// path and rule. Proposing is not staging and is not removal.
    pub fn record_candidates(conn: &Connection, candidates: &[CleanupCandidate]) -> Result<usize> {
        if candidates.is_empty() {
            return Ok(0);
        }
        let tx = conn.unchecked_transaction()?;
        let now = db::now_unix();
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO cleanup_candidates
                     (scan_id, entry_id, rule, safety, reason, reclaimable_bytes, state,
                      created_at, path, kind, modified_at, detail, group_key)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(scan_id, path, rule) DO UPDATE SET
                     safety            = excluded.safety,
                     reason            = excluded.reason,
                     reclaimable_bytes = excluded.reclaimable_bytes,
                     modified_at       = excluded.modified_at,
                     detail            = excluded.detail,
                     group_key         = excluded.group_key
                 WHERE cleanup_candidates.state IN ('proposed', 'staged')",
            )?;
            for candidate in candidates {
                // Defence in depth: a rule that proposes a protected path is a
                // bug in that rule, and it is caught here rather than at
                // removal time.
                if platform::is_protected_path(Path::new(&candidate.path)) {
                    continue;
                }
                stmt.execute(params![
                    candidate.scan_id,
                    candidate.entry_id,
                    candidate.rule,
                    candidate.safety.as_i64(),
                    candidate.reason,
                    candidate.bytes,
                    CandidateState::Proposed.as_str(),
                    now,
                    candidate.path,
                    candidate.kind.as_str(),
                    candidate.modified_at,
                    candidate.detail,
                    candidate.group_key,
                ])?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Put candidates in the queue. Refuses anything protected.
    pub fn stage(conn: &Connection, ids: &[i64]) -> Result<usize> {
        Self::set_state(conn, ids, CandidateState::Staged)
    }

    /// Take candidates back out of the queue.
    pub fn unstage(conn: &Connection, ids: &[i64]) -> Result<usize> {
        Self::set_state(conn, ids, CandidateState::Proposed)
    }

    fn set_state(conn: &Connection, ids: &[i64], state: CandidateState) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let tx = conn.unchecked_transaction()?;
        let mut changed = 0usize;
        {
            let mut stmt = tx.prepare(
                "UPDATE cleanup_candidates
                    SET state = ?2, staged_at = ?3, staged_size = reclaimable_bytes
                  WHERE id = ?1
                    AND state IN ('proposed', 'staged')
                    AND safety != ?4",
            )?;
            for id in ids {
                changed += stmt.execute(params![
                    id,
                    state.as_str(),
                    db::now_unix(),
                    SafetyClass::Protected.as_i64()
                ])?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn candidates(
        conn: &Connection,
        scan_id: i64,
        state: Option<CandidateState>,
        limit: u32,
    ) -> Result<Vec<CleanupCandidate>> {
        let sql = format!(
            "SELECT id, scan_id, entry_id, path, kind, safety, rule, reason, detail,
                    reclaimable_bytes, modified_at, state, group_key
               FROM cleanup_candidates
              WHERE scan_id = ?1 {filter}
              ORDER BY reclaimable_bytes DESC LIMIT ?2",
            filter = match state {
                Some(_) => "AND state = ?3",
                None => "",
            }
        );
        let mut stmt = conn.prepare(&sql)?;
        let limit = limit.clamp(1, 5_000);
        let rows = match state {
            Some(state) => stmt
                .query_map(params![scan_id, limit, state.as_str()], map_candidate)?
                .collect::<rusqlite::Result<Vec<_>>>()?,
            None => stmt
                .query_map(params![scan_id, limit], map_candidate)?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        };
        Ok(rows)
    }

    /// Everything currently in the queue, across every scan.
    pub fn staged(conn: &Connection) -> Result<Vec<CleanupCandidate>> {
        let mut stmt = conn.prepare(
            "SELECT id, scan_id, entry_id, path, kind, safety, rule, reason, detail,
                    reclaimable_bytes, modified_at, state, group_key
               FROM cleanup_candidates
              WHERE state = 'staged'
              ORDER BY reclaimable_bytes DESC",
        )?;
        let rows = stmt
            .query_map([], map_candidate)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Candidates rolled up by kind, which is how the Cleanup screen reads.
    pub fn groups(conn: &Connection, scan_id: i64) -> Result<Vec<CandidateGroup>> {
        let mut stmt = conn.prepare(
            "SELECT kind, safety, count(*), sum(reclaimable_bytes)
               FROM cleanup_candidates
              WHERE scan_id = ?1 AND state IN ('proposed', 'staged')
              GROUP BY kind, safety
              ORDER BY sum(reclaimable_bytes) DESC",
        )?;
        let rows = stmt
            .query_map(params![scan_id], |row| {
                let kind = CandidateKind::from_stored(&row.get::<_, String>(0)?);
                let safety = SafetyClass::from_i64(row.get(1)?);
                let count: i64 = row.get(2)?;
                let bytes: i64 = row.get(3)?;
                Ok(CandidateGroup {
                    kind,
                    label: kind.label().to_string(),
                    safety,
                    count,
                    bytes,
                    summary: describe_group(kind, safety, count),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Remove what the user approved.
    ///
    /// The confirmation is checked against the queue before anything is
    /// touched: the set of ids must match exactly, and the total must match to
    /// the byte. A mismatch refuses the entire operation rather than doing part
    /// of it, because a partial removal of a set somebody did not review is the
    /// worst outcome available.
    pub fn remove_approved(
        &self,
        conn: &Connection,
        confirmation: &Confirmation,
    ) -> Result<RemovalReport> {
        if confirmation.candidate_ids.is_empty() {
            return Ok(RemovalReport::from_outcomes(Vec::new()));
        }

        let staged = Self::staged(conn)?;
        let mut approved: Vec<CleanupCandidate> = Vec::new();
        for id in &confirmation.candidate_ids {
            match staged.iter().find(|c| c.id == *id) {
                Some(candidate) => approved.push(candidate.clone()),
                None => {
                    return Err(CoreError::invalid(format!(
                        "candidate {id} is no longer in the cleanup queue; \
                         review the queue again before removing"
                    )))
                }
            }
        }
        if approved.len() != staged.len() {
            return Err(CoreError::invalid(
                "the cleanup queue changed since you reviewed it; \
                 review it again before removing",
            ));
        }
        let total: i64 = approved.iter().map(|c| c.bytes).sum();
        if total != confirmation.expected_total_bytes {
            return Err(CoreError::invalid(format!(
                "the queue now totals {total} bytes, not the {} you approved; \
                 review it again before removing",
                confirmation.expected_total_bytes
            )));
        }

        let mut outcomes = Vec::with_capacity(approved.len());
        for candidate in &approved {
            outcomes.push(self.remove_one(conn, candidate)?);
        }
        Ok(RemovalReport::from_outcomes(outcomes))
    }

    /// The single destructive path. Everything above funnels here.
    fn remove_one(
        &self,
        conn: &Connection,
        candidate: &CleanupCandidate,
    ) -> Result<RemovalOutcome> {
        let path = candidate.path_buf();

        let refusal = self.refusal_reason(candidate, &path);
        if let Some(detail) = refusal {
            return self.finish(conn, candidate, CandidateState::Refused, Some(detail));
        }

        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return self.finish(
                    conn,
                    candidate,
                    CandidateState::Vanished,
                    Some("it was already gone".to_string()),
                );
            }
            Err(err) => {
                return self.finish(
                    conn,
                    candidate,
                    CandidateState::Failed,
                    Some(format!("could not read it: {err}")),
                );
            }
        };

        // Guard against the file changing between review and approval. A cache
        // that grew is still a cache, but a file that changed is one somebody
        // may now be using, and the honest response is to put it back in front
        // of the user rather than to trash it on stale information.
        if metadata.is_file() && metadata.len() as i64 != candidate.bytes {
            return self.finish(
                conn,
                candidate,
                CandidateState::Refused,
                Some(format!(
                    "it changed since you reviewed it ({} bytes now, {} then)",
                    metadata.len(),
                    candidate.bytes
                )),
            );
        }

        match self.remover.move_to_trash(&path) {
            Ok(()) => {
                // Verify rather than assume. A remover that reports success
                // while leaving the file in place would otherwise show up as a
                // number that does not match the disk.
                if std::fs::symlink_metadata(&path).is_ok() && self.remover.name() != "dry-run" {
                    return self.finish(
                        conn,
                        candidate,
                        CandidateState::Failed,
                        Some("it is still there after the move to the Trash".to_string()),
                    );
                }
                self.finish(conn, candidate, CandidateState::Removed, None)
            }
            Err(detail) => self.finish(conn, candidate, CandidateState::Failed, Some(detail)),
        }
    }

    /// Every reason DiskMate declines to remove something, in one place.
    fn refusal_reason(&self, candidate: &CleanupCandidate, path: &Path) -> Option<String> {
        if candidate.safety == SafetyClass::Protected {
            return Some("DiskMate protects this and will not remove it".to_string());
        }
        if platform::is_protected_path(path) {
            return Some("this is in a location DiskMate protects and will not remove".to_string());
        }
        if !path.is_absolute() {
            return Some("only absolute paths can be removed".to_string());
        }
        // A path with no parent is a filesystem root. Nothing legitimate ever
        // stages one, so treat it as a bug rather than an instruction.
        if path.parent().is_none() || path == Path::new("/") {
            return Some("this is a volume root".to_string());
        }
        if path.components().count() < 3 {
            return Some("this is too close to the root of the disk to remove safely".to_string());
        }
        None
    }

    fn finish(
        &self,
        conn: &Connection,
        candidate: &CleanupCandidate,
        state: CandidateState,
        detail: Option<String>,
    ) -> Result<RemovalOutcome> {
        let now = db::now_unix();
        conn.execute(
            "UPDATE cleanup_candidates SET state = ?2, resolved_at = ?3 WHERE id = ?1",
            params![candidate.id, state.as_str(), now],
        )?;
        conn.execute(
            "INSERT INTO cleanup_operations
                 (candidate_id, path, kind, safety, rule, reason, bytes, outcome, detail,
                  performed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                candidate.id,
                candidate.path,
                candidate.kind.as_str(),
                candidate.safety.as_i64(),
                candidate.rule,
                candidate.reason,
                candidate.bytes,
                state.as_str(),
                detail,
                now
            ],
        )?;
        Ok(RemovalOutcome {
            candidate_id: candidate.id,
            path: candidate.path.clone(),
            state,
            bytes: if state == CandidateState::Removed {
                candidate.bytes
            } else {
                0
            },
            detail,
        })
    }

    /// Everything DiskMate has ever removed or declined to remove.
    pub fn history(conn: &Connection, limit: u32) -> Result<Vec<RemovalOutcome>> {
        let mut stmt = conn.prepare(
            "SELECT candidate_id, path, outcome, bytes, detail
               FROM cleanup_operations ORDER BY performed_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit.clamp(1, 1_000)], |row| {
                Ok(RemovalOutcome {
                    candidate_id: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    path: row.get(1)?,
                    state: CandidateState::from_stored(&row.get::<_, String>(2)?),
                    bytes: row.get(3)?,
                    detail: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

fn map_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<CleanupCandidate> {
    let path: String = row.get(3)?;
    let name = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    Ok(CleanupCandidate {
        id: row.get(0)?,
        scan_id: row.get(1)?,
        entry_id: row.get(2)?,
        path,
        name,
        kind: CandidateKind::from_stored(&row.get::<_, String>(4)?),
        safety: SafetyClass::from_i64(row.get(5)?),
        rule: row.get(6)?,
        reason: row.get(7)?,
        detail: row.get(8)?,
        bytes: row.get(9)?,
        modified_at: row.get(10)?,
        state: CandidateState::from_stored(&row.get::<_, String>(11)?),
        group_key: row.get(12)?,
    })
}

fn describe_group(kind: CandidateKind, safety: SafetyClass, count: i64) -> String {
    let noun = if count == 1 { "item" } else { "items" };
    match (kind, safety) {
        (CandidateKind::Trash, _) => {
            format!("{count} {noun} you already moved to the Trash.")
        }
        (CandidateKind::Cache, _) => {
            format!("{count} {noun} applications will rebuild when they next need them.")
        }
        (CandidateKind::DeveloperArtifact, _) => {
            format!("{count} {noun} your build tools regenerate. Rebuilding takes time, not data.")
        }
        (CandidateKind::OldDownload, _) => format!(
            "{count} {noun} in Downloads you have not opened in a long time. \
             Check them before removing: downloads are not always replaceable."
        ),
        (CandidateKind::BigAndUntouched, _) => format!(
            "{count} large {noun} untouched for over a year. Only you know whether \
             they still matter."
        ),
        (CandidateKind::DuplicateCopy, _) => format!(
            "{count} extra {noun} whose contents are byte-for-byte identical to a copy \
             DiskMate is keeping."
        ),
        (CandidateKind::AppLeftover, _) => {
            format!("{count} {noun} left behind by applications that are no longer installed.")
        }
        (CandidateKind::AppBundle, _) => {
            format!("{count} {noun}. Removing one uninstalls the application.")
        }
        (CandidateKind::Log, _) => format!("{count} log {noun}."),
        (CandidateKind::Other, _) => format!("{count} {noun}."),
    }
}
