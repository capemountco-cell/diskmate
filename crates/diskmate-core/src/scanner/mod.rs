//! The filesystem scanner.
//!
//! See `docs/scanner.md` for the design rationale and the measured numbers.

mod aggregate;
mod cancel;
mod meta;
mod pipeline;
mod progress;
mod walk;

pub use aggregate::{DirAggregator, DirListing};
pub use cancel::CancellationToken;
pub use meta::extension_of;
pub use pipeline::{run_scan, ScanRequest};
pub use progress::{ProgressCounters, ProgressThrottle};
