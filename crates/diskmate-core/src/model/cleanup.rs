use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::SafetyClass;

/// What kind of thing a cleanup candidate is.
///
/// The kind decides how it is explained, not whether it may be removed. That
/// is what [`SafetyClass`] is for, and the two are deliberately separate: a
/// cache is usually safe and occasionally not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateKind {
    /// Already in the Trash. Emptying it is the user's own earlier decision.
    Trash,
    /// Regeneratable cache.
    Cache,
    /// Build output, dependency directories, simulator runtimes.
    DeveloperArtifact,
    /// A file in Downloads that has not been touched in a long time.
    OldDownload,
    /// Large and untouched for a year or more.
    BigAndUntouched,
    /// One copy of a confirmed duplicate set.
    DuplicateCopy,
    /// A file an application left behind after it was removed.
    AppLeftover,
    /// The application bundle itself.
    AppBundle,
    /// A log file.
    Log,
    Other,
}

impl CandidateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trash => "trash",
            Self::Cache => "cache",
            Self::DeveloperArtifact => "developerArtifact",
            Self::OldDownload => "oldDownload",
            Self::BigAndUntouched => "bigAndUntouched",
            Self::DuplicateCopy => "duplicateCopy",
            Self::AppLeftover => "appLeftover",
            Self::AppBundle => "appBundle",
            Self::Log => "log",
            Self::Other => "other",
        }
    }

    pub fn from_stored(value: &str) -> Self {
        match value {
            "trash" => Self::Trash,
            "cache" => Self::Cache,
            "developerArtifact" => Self::DeveloperArtifact,
            "oldDownload" => Self::OldDownload,
            "bigAndUntouched" => Self::BigAndUntouched,
            "duplicateCopy" => Self::DuplicateCopy,
            "appLeftover" => Self::AppLeftover,
            "appBundle" => Self::AppBundle,
            "log" => Self::Log,
            _ => Self::Other,
        }
    }

    /// A short heading for a group of these.
    pub fn label(self) -> &'static str {
        match self {
            Self::Trash => "Trash",
            Self::Cache => "Caches",
            Self::DeveloperArtifact => "Developer files",
            Self::OldDownload => "Old downloads",
            Self::BigAndUntouched => "Big and untouched",
            Self::DuplicateCopy => "Duplicates",
            Self::AppLeftover => "Application leftovers",
            Self::AppBundle => "Applications",
            Self::Log => "Logs",
            Self::Other => "Other",
        }
    }
}

/// Where a candidate is in the review flow.
///
/// Nothing moves from `Proposed` to `Removed` without passing through
/// `Staged`, and staging is always an explicit act by the person using
/// DiskMate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateState {
    /// DiskMate found it. Nothing has been decided.
    Proposed,
    /// The user put it in the queue.
    Staged,
    /// Moved to the Trash.
    Removed,
    /// DiskMate declined to remove it, with a reason.
    Refused,
    /// The attempt failed.
    Failed,
    /// It was gone before DiskMate got to it.
    Vanished,
}

impl CandidateState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Staged => "staged",
            Self::Removed => "removed",
            Self::Refused => "refused",
            Self::Failed => "failed",
            Self::Vanished => "vanished",
        }
    }

    pub fn from_stored(value: &str) -> Self {
        match value {
            "staged" => Self::Staged,
            "removed" => Self::Removed,
            "refused" => Self::Refused,
            "failed" => Self::Failed,
            "vanished" => Self::Vanished,
            _ => Self::Proposed,
        }
    }
}

/// A proposal, never an instruction.
///
/// Every candidate carries the reason it was proposed and how risky DiskMate
/// considers it. A candidate without a reason is not a candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupCandidate {
    pub id: i64,
    pub scan_id: i64,
    pub entry_id: Option<i64>,
    pub path: String,
    pub name: String,
    pub kind: CandidateKind,
    pub safety: SafetyClass,
    /// The rule that proposed it, for grouping and for debugging a surprise.
    pub rule: String,
    /// Why, in a sentence a person can act on.
    pub reason: String,
    /// Extra context: how old, how many copies, which application.
    pub detail: Option<String>,
    pub bytes: i64,
    pub modified_at: Option<i64>,
    pub state: CandidateState,
    /// Set for leftovers so the UI can show an application and its files
    /// together.
    pub group_key: Option<String>,
}

impl CleanupCandidate {
    pub fn path_buf(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
}

/// What the user reviewed and approved.
///
/// Removal takes this rather than acting on "whatever is staged right now".
/// If the queue changed between the review and the approval, the totals stop
/// matching and the whole operation is refused, so nobody ever confirms three
/// items and loses five.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Confirmation {
    pub candidate_ids: Vec<i64>,
    pub expected_total_bytes: i64,
}

/// What happened to one candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalOutcome {
    pub candidate_id: i64,
    pub path: String,
    pub state: CandidateState,
    pub bytes: i64,
    /// Present whenever the state is anything but `Removed`.
    pub detail: Option<String>,
}

/// The result of one approved removal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalReport {
    pub outcomes: Vec<RemovalOutcome>,
    pub removed_count: usize,
    pub removed_bytes: i64,
    pub refused_count: usize,
    pub failed_count: usize,
}

impl RemovalReport {
    pub fn from_outcomes(outcomes: Vec<RemovalOutcome>) -> Self {
        let removed_count = outcomes
            .iter()
            .filter(|o| o.state == CandidateState::Removed)
            .count();
        let removed_bytes = outcomes
            .iter()
            .filter(|o| o.state == CandidateState::Removed)
            .map(|o| o.bytes)
            .sum();
        let refused_count = outcomes
            .iter()
            .filter(|o| o.state == CandidateState::Refused)
            .count();
        let failed_count = outcomes
            .iter()
            .filter(|o| matches!(o.state, CandidateState::Failed | CandidateState::Vanished))
            .count();
        Self {
            outcomes,
            removed_count,
            removed_bytes,
            refused_count,
            failed_count,
        }
    }
}

/// A group of candidates the UI shows together.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateGroup {
    pub kind: CandidateKind,
    pub label: String,
    pub safety: SafetyClass,
    pub count: i64,
    pub bytes: i64,
    /// What this whole group is, in one sentence.
    pub summary: String,
}
