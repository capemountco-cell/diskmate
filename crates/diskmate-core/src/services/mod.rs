//! Application services.
//!
//! These sit between the Tauri command handlers and the engine. A command
//! handler's whole job is to deserialise its arguments, call one method here,
//! and serialise the result; every decision that is not about IPC lives in this
//! layer, which is why it can be exercised by tests with no Tauri present.

mod memory;
mod startup;
mod storage;

pub use memory::{MemoryReport, MemoryService};
pub use startup::{StartupReport, StartupService};
pub use storage::{
    CategoryBucket, ErrorBucket, ScanHandle, StorageService, StorageSnapshot,
};
