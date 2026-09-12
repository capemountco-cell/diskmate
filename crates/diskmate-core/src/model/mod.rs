//! Domain types shared by the engine, the Tauri adapter and the UI.
//!
//! Everything here is `serde`-serialisable because these types cross the
//! Tauri IPC boundary verbatim. Field names use camelCase on the wire so the
//! TypeScript layer needs no translation shims.

mod classify;
mod cleanup;
mod entry;
mod memory;
mod scan;
mod startup;
mod volume;

pub use classify::{classify_directory, Category, SafetyClass};
pub use cleanup::{
    CandidateGroup, CandidateKind, CandidateState, CleanupCandidate, Confirmation, RemovalOutcome,
    RemovalReport,
};
pub use entry::{EntryKind, EntryRecord, EntryRow, ExtensionBucket};
pub use memory::{MemoryPressure, MemorySnapshot, ProcessMemory};
pub use scan::{ScanError, ScanErrorKind, ScanOptions, ScanProgress, ScanStatus, ScanSummary};
pub use startup::{StartupImpact, StartupItem, StartupScope, StartupSource};
pub use volume::{VolumeInfo, VolumeKind};
