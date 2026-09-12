//! Analysis that runs over a completed scan.
//!
//! Everything here reads the index and the files it points at. None of it
//! modifies anything: analysis produces findings, and findings become cleanup
//! candidates, which only the cleanup service can act on.

mod applications;
mod duplicates;
mod snapshots;

pub use applications::{Application, ApplicationInspector, Leftover};
pub use duplicates::{DuplicateFinder, DuplicateGroup, DuplicateMember, DuplicateStats};
pub use snapshots::{ChangeKind, Comparison, Delta, Snapshot, Snapshots};
