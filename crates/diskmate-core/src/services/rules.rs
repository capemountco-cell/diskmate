//! The rules that propose cleanup candidates.
//!
//! Each rule answers one question against a completed scan, and each produces
//! candidates carrying the sentence the user will read. A rule that cannot
//! explain itself does not belong here: "you could reclaim 4 GB" is not a
//! reason, it is a number.
//!
//! Rules propose. They never stage and never remove.

use rusqlite::{params, Connection};

use crate::error::Result;
use crate::model::{CandidateKind, CandidateState, CleanupCandidate, SafetyClass};
use crate::platform;

/// Days after which a download is "old". Deliberately generous: a download
/// from last month is not clutter.
const OLD_DOWNLOAD_DAYS: i64 = 90;
/// A file has to be both large and genuinely untouched to be worth mentioning.
const UNTOUCHED_DAYS: i64 = 365;
const BIG_FILE_BYTES: i64 = 100 * 1000 * 1000;
/// Below this a candidate is noise. Offering to reclaim 40 KB wastes attention.
const MIN_CANDIDATE_BYTES: i64 = 1000 * 1000;

/// Directory names that are build output or fetched dependencies, with what
/// regenerates each one. The explanation is the point: "node_modules" means
/// nothing to most people, "npm install rebuilds this" means everything.
const DEVELOPER_DIRS: &[(&str, &str)] = &[
    (
        "node_modules",
        "npm or yarn rebuilds this from package.json",
    ),
    ("target", "cargo rebuilds this"),
    ("DerivedData", "Xcode rebuilds this the next time you build"),
    (".gradle", "Gradle re-downloads this"),
    ("Pods", "CocoaPods reinstalls this from the Podfile"),
    ("__pycache__", "Python regenerates this automatically"),
    (
        ".venv",
        "your virtual environment can be recreated from requirements",
    ),
    (
        "venv",
        "your virtual environment can be recreated from requirements",
    ),
];

pub struct CleanupRules;

impl CleanupRules {
    /// Run every rule against a scan and return what they propose.
    ///
    /// `now` is passed in rather than read, so age-based rules are testable
    /// without waiting a year.
    pub fn propose(conn: &Connection, scan_id: i64, now: i64) -> Result<Vec<CleanupCandidate>> {
        let mut candidates = Vec::new();
        candidates.extend(Self::trash(conn, scan_id)?);
        candidates.extend(Self::caches(conn, scan_id)?);
        candidates.extend(Self::developer_artifacts(conn, scan_id)?);
        candidates.extend(Self::old_downloads(conn, scan_id, now)?);
        candidates.extend(Self::big_and_untouched(conn, scan_id, now)?);
        // A rule proposing something protected is a bug in that rule. Drop it
        // here as well as at the service boundary, so a bad rule cannot even
        // put the idea in front of the user.
        candidates.retain(|c| !platform::is_protected_path(std::path::Path::new(&c.path)));
        Ok(candidates)
    }

    /// What is already in the Trash. The safest thing DiskMate can offer,
    /// because the decision to discard it was already made by the user.
    fn trash(conn: &Connection, scan_id: i64) -> Result<Vec<CleanupCandidate>> {
        let sql = "
            SELECT e.id, e.name,
                   COALESCE(e.path, p.path || '/' || e.name),
                   e.subtree_logical_size, e.modified_at
              FROM fs_entries e
              LEFT JOIN fs_entries p ON p.id = e.parent_id
             WHERE e.scan_id = ?1
               AND p.path LIKE '%/.Trash'
               AND e.subtree_logical_size > 0
             ORDER BY e.subtree_logical_size DESC LIMIT 500";
        Self::collect(conn, sql, scan_id, |name, bytes, _| Proposal {
            kind: CandidateKind::Trash,
            safety: SafetyClass::Safe,
            rule: "trash".into(),
            reason: format!("{name} is already in your Trash."),
            detail: Some("Emptying the Trash is the only way this space comes back.".into()),
            bytes,
            group_key: None,
        })
    }

    /// Caches. Safe because the application that made them will make them
    /// again; the cost of removing one is a slower next launch, not lost data.
    fn caches(conn: &Connection, scan_id: i64) -> Result<Vec<CleanupCandidate>> {
        let sql = "
            SELECT e.id, e.name,
                   COALESCE(e.path, p.path || '/' || e.name),
                   e.subtree_logical_size, e.modified_at
              FROM fs_entries e
              LEFT JOIN fs_entries p ON p.id = e.parent_id
             WHERE e.scan_id = ?1
               AND e.kind = 1
               AND p.name = 'Caches'
               AND e.subtree_logical_size > 1000000
             ORDER BY e.subtree_logical_size DESC LIMIT 300";
        Self::collect(conn, sql, scan_id, |name, bytes, _| Proposal {
            kind: CandidateKind::Cache,
            safety: SafetyClass::Safe,
            rule: "cache".into(),
            reason: format!("{name} is a cache. The app that made it will rebuild it."),
            detail: Some(
                "Removing a cache costs a slower next launch, not any of your data.".into(),
            ),
            bytes,
            group_key: None,
        })
    }

    /// Build output and fetched dependencies.
    fn developer_artifacts(conn: &Connection, scan_id: i64) -> Result<Vec<CleanupCandidate>> {
        let mut out = Vec::new();
        for (dir_name, regenerates) in DEVELOPER_DIRS {
            let sql = "
                SELECT e.id, e.name,
                       COALESCE(e.path, p.path || '/' || e.name),
                       e.subtree_logical_size, e.modified_at
                  FROM fs_entries e
                  LEFT JOIN fs_entries p ON p.id = e.parent_id
                 WHERE e.scan_id = ?1
                   AND e.kind = 1
                   AND e.name = ?2
                   AND e.subtree_logical_size > 1000000
                 ORDER BY e.subtree_logical_size DESC LIMIT 200";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(params![scan_id, dir_name], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (entry_id, name, path, bytes, modified) in rows {
                out.push(build(
                    scan_id,
                    Some(entry_id),
                    path,
                    name.clone(),
                    modified,
                    Proposal {
                        kind: CandidateKind::DeveloperArtifact,
                        safety: SafetyClass::Safe,
                        rule: format!("developer:{dir_name}"),
                        reason: format!("{name} is build output; {regenerates}."),
                        detail: Some(
                            "Removing it costs one rebuild. Nothing you wrote is in here.".into(),
                        ),
                        bytes,
                        group_key: None,
                    },
                ));
            }
        }
        Ok(out)
    }

    /// Downloads nobody has touched in a long time. Review, never safe: a
    /// download can be an installer you can fetch again, or the only copy of
    /// something somebody sent you, and DiskMate cannot tell which.
    fn old_downloads(conn: &Connection, scan_id: i64, now: i64) -> Result<Vec<CleanupCandidate>> {
        let cutoff = now - OLD_DOWNLOAD_DAYS * 86_400;
        let sql = "
            SELECT e.id, e.name,
                   COALESCE(e.path, p.path || '/' || e.name),
                   e.logical_size, e.modified_at
              FROM fs_entries e
              LEFT JOIN fs_entries p ON p.id = e.parent_id
             WHERE e.scan_id = ?1
               AND e.kind = 0
               AND e.category = 4
               AND e.logical_size > 1000000
               AND e.modified_at IS NOT NULL
               AND e.modified_at < ?3
             ORDER BY e.logical_size DESC LIMIT 300";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(params![scan_id, 0i64, cutoff], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .map(|(entry_id, name, path, bytes, modified)| {
                let months = modified.map(|m| (now - m) / (30 * 86_400)).unwrap_or(0);
                build(
                    scan_id,
                    Some(entry_id),
                    path,
                    name.clone(),
                    modified,
                    Proposal {
                        kind: CandidateKind::OldDownload,
                        safety: SafetyClass::Review,
                        rule: "download:old".into(),
                        reason: format!(
                            "{name} has been in Downloads for {months} months without \
                             being opened."
                        ),
                        detail: Some(
                            "Downloads are not always replaceable. Check before removing.".into(),
                        ),
                        bytes,
                        group_key: None,
                    },
                )
            })
            .collect())
    }

    /// Large files nobody has touched in a year. The most useful rule and the
    /// one most in need of restraint, so it only ever proposes review.
    fn big_and_untouched(
        conn: &Connection,
        scan_id: i64,
        now: i64,
    ) -> Result<Vec<CleanupCandidate>> {
        let cutoff = now - UNTOUCHED_DAYS * 86_400;
        let sql = "
            SELECT e.id, e.name,
                   COALESCE(e.path, p.path || '/' || e.name),
                   e.logical_size, e.modified_at
              FROM fs_entries e
              LEFT JOIN fs_entries p ON p.id = e.parent_id
             WHERE e.scan_id = ?1
               AND e.kind = 0
               AND e.logical_size > ?2
               AND e.modified_at IS NOT NULL
               AND e.modified_at < ?3
               AND e.hardlink_dup = 0
             ORDER BY e.logical_size DESC LIMIT 300";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(params![scan_id, BIG_FILE_BYTES, cutoff], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .map(|(entry_id, name, path, bytes, modified)| {
                let years = modified
                    .map(|m| ((now - m) as f64 / (365.0 * 86_400.0)).max(1.0))
                    .unwrap_or(1.0);
                build(
                    scan_id,
                    Some(entry_id),
                    path,
                    name.clone(),
                    modified,
                    Proposal {
                        kind: CandidateKind::BigAndUntouched,
                        safety: SafetyClass::Review,
                        rule: "file:big-untouched".into(),
                        reason: format!(
                            "{name} is large and has not changed in over {years:.0} year(s)."
                        ),
                        detail: Some(
                            "Age is not the same as being unwanted. Only you know whether \
                             this still matters."
                                .into(),
                        ),
                        bytes,
                        group_key: None,
                    },
                )
            })
            .collect())
    }

    fn collect(
        conn: &Connection,
        sql: &str,
        scan_id: i64,
        make: impl Fn(&str, i64, Option<i64>) -> Proposal,
    ) -> Result<Vec<CleanupCandidate>> {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(params![scan_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter(|(_, _, _, bytes, _)| *bytes >= MIN_CANDIDATE_BYTES)
            .map(|(entry_id, name, path, bytes, modified)| {
                let proposal = make(&name, bytes, modified);
                build(scan_id, Some(entry_id), path, name, modified, proposal)
            })
            .collect())
    }
}

/// The parts of a candidate a rule decides.
pub(crate) struct Proposal {
    pub kind: CandidateKind,
    pub safety: SafetyClass,
    pub rule: String,
    pub reason: String,
    pub detail: Option<String>,
    pub bytes: i64,
    pub group_key: Option<String>,
}

pub(crate) fn build(
    scan_id: i64,
    entry_id: Option<i64>,
    path: String,
    name: String,
    modified_at: Option<i64>,
    proposal: Proposal,
) -> CleanupCandidate {
    CleanupCandidate {
        id: 0,
        scan_id,
        entry_id,
        path,
        name,
        kind: proposal.kind,
        safety: proposal.safety,
        rule: proposal.rule,
        reason: proposal.reason,
        detail: proposal.detail,
        bytes: proposal.bytes,
        modified_at,
        state: CandidateState::Proposed,
        group_key: proposal.group_key,
    }
}
