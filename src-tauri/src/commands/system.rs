//! Read-only system inspection: memory and startup items.
//!
//! Both are strictly read-only in Milestone 01. There is no command here that
//! frees memory, ends a process, or changes a launch item, and adding one would
//! require going through the services layer deliberately.

use diskmate_core::services::{MemoryReport, StartupReport};
use tauri::State;

use crate::state::AppState;

use super::{to_command_error, CommandResult};

#[tauri::command]
pub fn memory_report(state: State<'_, AppState>) -> CommandResult<MemoryReport> {
    state.memory.report().map_err(to_command_error)
}

#[tauri::command]
pub fn startup_report(state: State<'_, AppState>) -> CommandResult<StartupReport> {
    state.startup.report().map_err(to_command_error)
}
