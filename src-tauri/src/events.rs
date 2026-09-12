//! Event names the frontend subscribes to.
//!
//! Kept in one place so the TypeScript side has a single list to mirror.

/// Coalesced scan progress. Emitted on a timer by the engine, roughly six to
/// ten times a second, never once per entry.
pub const SCAN_PROGRESS: &str = "diskmate://scan-progress";

/// A scan reached a terminal state: completed, cancelled, or failed. Carries
/// the summary so the UI does not need a follow-up round trip.
pub const SCAN_FINISHED: &str = "diskmate://scan-finished";

/// Progress through the duplicate pipeline: which stage, and how far in.
pub const DUPLICATE_PROGRESS: &str = "diskmate://duplicate-progress";
/// The duplicate search finished, carrying its statistics.
pub const DUPLICATE_FINISHED: &str = "diskmate://duplicate-finished";
pub const DUPLICATE_FAILED: &str = "diskmate://duplicate-failed";
