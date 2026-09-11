//! IPC command handlers.
//!
//! Every handler here is an adapter: deserialise, call one service method,
//! serialise. Errors become a string because that is what crosses the IPC
//! boundary; the engine's typed errors are preserved up to this point.

mod scan;
mod storage;
mod system;

// Glob re-exports, not named ones: `#[tauri::command]` generates hidden items
// alongside each function, and `generate_handler!` needs them in this module.
pub use scan::*;
pub use storage::*;
pub use system::*;

/// Result type for commands. Tauri needs a serialisable error.
pub type CommandResult<T> = Result<T, String>;

/// Convert an engine error into something the UI can show.
///
/// The message is the engine's own, which is written to be read by a person;
/// there is no separate "user-friendly" rewrite that could drift from reality.
pub(crate) fn to_command_error(error: diskmate_core::CoreError) -> String {
    error.to_string()
}
