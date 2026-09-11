//! IPC command handlers.
//!
//! Every handler here is an adapter: deserialise, call one service method,
//! serialise. Errors become a string because that is what crosses the IPC
//! boundary; the engine's typed errors are preserved up to this point.

mod scan;
mod storage;
mod system;

pub use scan::{active_scan, cancel_scan, start_scan};
pub use storage::{
    app_info, directory_by_path, entry_ancestors, latest_scan, list_children, list_volumes,
    recent_scans, scan_errors, storage_snapshot,
};
pub use system::{memory_report, startup_report};

/// Result type for commands. Tauri needs a serialisable error.
pub type CommandResult<T> = Result<T, String>;

/// Convert an engine error into something the UI can show.
///
/// The message is the engine's own, which is written to be read by a person;
/// there is no separate "user-friendly" rewrite that could drift from reality.
pub(crate) fn to_command_error(error: diskmate_core::CoreError) -> String {
    error.to_string()
}
