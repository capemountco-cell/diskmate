//! The live monitor. Read-only, like the rest of the system inspection.

use diskmate_core::services::{MonitorReport, Sample};
use tauri::State;

use crate::state::AppState;

use super::{to_command_error, CommandResult};

#[tauri::command]
pub fn monitor_start(
    state: State<'_, AppState>,
    interval_ms: u64,
    watch_path: Option<String>,
) -> CommandResult<()> {
    state
        .monitor
        .start(interval_ms, watch_path.map(std::path::PathBuf::from));
    Ok(())
}

#[tauri::command]
pub fn monitor_stop(state: State<'_, AppState>) {
    state.monitor.stop();
}

#[tauri::command]
pub fn monitor_report(state: State<'_, AppState>) -> MonitorReport {
    state.monitor.report()
}

#[tauri::command]
pub fn monitor_sample(state: State<'_, AppState>) -> CommandResult<Sample> {
    state.monitor.sample_once().map_err(to_command_error)
}
