//! System inspection: memory, the apps using it, and startup items.
//!
//! Everything here reads, except `quit_app`, which asks one ordinary app to
//! quit after the person clicked Quit on it. That goes through
//! `ProcessService`, which refuses anything that is not that app, or not an
//! app with a Dock icon. No command changes a launch item.

use diskmate_core::model::ProcessMemory;
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

#[tauri::command]
pub fn top_apps(state: State<'_, AppState>, limit: usize) -> CommandResult<Vec<ProcessMemory>> {
    state
        .process
        .top_apps(limit.min(50))
        .map_err(to_command_error)
}

#[tauri::command]
pub fn quit_app(state: State<'_, AppState>, pid: i32, name: String) -> CommandResult<()> {
    state.process.quit_app(pid, &name).map_err(to_command_error)
}
