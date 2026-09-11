//! Starting, watching and cancelling scans.

use std::path::PathBuf;

use diskmate_core::model::{ScanStatus, ScanSummary};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events;
use crate::state::AppState;

use super::CommandResult;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanStarted {
    pub root: String,
}

/// Terminal event for a scan. `summary` is absent only when the scan failed
/// before it produced one.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanFinished {
    pub root: String,
    pub status: ScanStatus,
    pub summary: Option<ScanSummary>,
    pub error: Option<String>,
}

/// Start a scan on a background thread and stream progress to the window.
///
/// The command returns as soon as the scan is accepted. Everything after that
/// arrives as events, so the UI thread is never blocked on a walk that may run
/// for minutes.
#[tauri::command]
pub fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    root: String,
) -> CommandResult<ScanStarted> {
    let root_path = PathBuf::from(&root);
    if state.storage.active_scan().is_some() {
        return Err("a scan is already running".to_string());
    }

    let storage = state.storage.clone();
    let options = storage.default_options(&root_path);
    let response = ScanStarted { root: root.clone() };

    // A plain OS thread, not an async task: the scan is a long CPU-and-syscall
    // bound job, and putting it on the async runtime would occupy an executor
    // thread for its whole duration anyway.
    std::thread::Builder::new()
        .name("diskmate-scan".into())
        .spawn(move || {
            let emitter = app.clone();
            let outcome = storage.scan(options, move |progress| {
                let _ = emitter.emit(events::SCAN_PROGRESS, &progress);
            });

            let finished = match outcome {
                Ok(summary) => ScanFinished {
                    root: root.clone(),
                    status: summary.status,
                    summary: Some(summary),
                    error: None,
                },
                Err(error) => ScanFinished {
                    root: root.clone(),
                    status: ScanStatus::Failed,
                    summary: None,
                    error: Some(error.to_string()),
                },
            };
            let _ = app.emit(events::SCAN_FINISHED, &finished);
        })
        .map_err(|e| format!("could not start the scan thread: {e}"))?;

    Ok(response)
}

/// Ask the running scan to stop. Returns false when nothing was running.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) -> bool {
    state.storage.cancel_active_scan()
}

#[tauri::command]
pub fn active_scan(state: State<'_, AppState>) -> Option<String> {
    state
        .storage
        .active_scan()
        .map(|path| path.to_string_lossy().into_owned())
}

/// Kept so `Manager` stays in scope for the `app.state()` pattern used by
/// future commands that need state from a spawned thread.
#[allow(unused)]
fn _assert_manager_in_scope(app: &AppHandle) -> Option<tauri::State<'_, AppState>> {
    Some(app.state::<AppState>())
}
