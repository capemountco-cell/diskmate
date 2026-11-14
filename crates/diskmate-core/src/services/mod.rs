//! Application services.
//!
//! These sit between the Tauri command handlers and the engine. A command
//! handler's whole job is to deserialise its arguments, call one method here,
//! and serialise the result; every decision that is not about IPC lives in this
//! layer, which is why it can be exercised by tests with no Tauri present.

mod cleanup;
mod memory;
mod monitor;
mod process;
mod rules;
mod startup;
mod storage;

pub use cleanup::{CleanupService, DryRunRemover, Remover, TrashRemover, UNINSTALL_BUNDLE_RULE};
pub use memory::{MemoryReport, MemoryService};
pub use monitor::{MonitorReport, MonitorService, Sample};
pub use process::ProcessService;
pub use rules::CleanupRules;
pub use startup::{StartupReport, StartupService};
pub use storage::{CategoryBucket, ErrorBucket, ScanHandle, StorageService, StorageSnapshot};
