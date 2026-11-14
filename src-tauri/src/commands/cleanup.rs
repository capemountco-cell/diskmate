//! Cleanup, duplicates, the uninstaller and snapshots.
//!
//! Adapters only. Every safety decision lives in the engine's cleanup service,
//! and nothing here can bypass it: the removal command hands the confirmation
//! straight through, and the service checks it against the queue.

use diskmate_core::analysis::{Application, Comparison, DuplicateGroup, Leftover, Snapshot};
use diskmate_core::model::{
    CandidateGroup, CandidateState, CleanupCandidate, Confirmation, RemovalOutcome, RemovalReport,
};
use diskmate_core::scanner::CancellationToken;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::events;
use crate::state::AppState;

use super::{to_command_error, CommandResult};

// ------------------------------------------------------------- candidates --

#[tauri::command]
pub fn propose_cleanup(
    state: State<'_, AppState>,
    scan_id: i64,
) -> CommandResult<Vec<CleanupCandidate>> {
    state
        .storage
        .propose_cleanup(scan_id)
        .map_err(to_command_error)
}

#[tauri::command]
pub fn cleanup_groups(
    state: State<'_, AppState>,
    scan_id: i64,
) -> CommandResult<Vec<CandidateGroup>> {
    state
        .storage
        .cleanup_groups(scan_id)
        .map_err(to_command_error)
}

#[tauri::command]
pub fn cleanup_candidates(
    state: State<'_, AppState>,
    scan_id: i64,
    state_filter: Option<CandidateState>,
    limit: u32,
) -> CommandResult<Vec<CleanupCandidate>> {
    state
        .storage
        .cleanup_candidates(scan_id, state_filter, limit)
        .map_err(to_command_error)
}

#[tauri::command]
pub fn stage_cleanup(state: State<'_, AppState>, ids: Vec<i64>) -> CommandResult<usize> {
    state.storage.stage_cleanup(&ids).map_err(to_command_error)
}

#[tauri::command]
pub fn unstage_cleanup(state: State<'_, AppState>, ids: Vec<i64>) -> CommandResult<usize> {
    state
        .storage
        .unstage_cleanup(&ids)
        .map_err(to_command_error)
}

#[tauri::command]
pub fn staged_cleanup(state: State<'_, AppState>) -> CommandResult<Vec<CleanupCandidate>> {
    state.storage.staged_cleanup().map_err(to_command_error)
}

/// Remove what the user approved.
///
/// The confirmation carries the exact ids and the exact total they were shown.
/// The engine refuses the whole operation if either no longer matches the
/// queue, so this command cannot be used to remove something the user did not
/// see.
///
/// Runs off the main thread, so the window keeps drawing (and its progress bar
/// keeps moving) while things move to the Trash.
#[tauri::command(async)]
pub fn remove_approved(
    app: AppHandle,
    state: State<'_, AppState>,
    confirmation: Confirmation,
) -> CommandResult<RemovalReport> {
    state
        .storage
        .remove_approved_with_progress(&confirmation, |progress| {
            let _ = app.emit(events::REMOVAL_PROGRESS, progress);
        })
        .map_err(to_command_error)
}

#[tauri::command]
pub fn cleanup_history(
    state: State<'_, AppState>,
    limit: u32,
) -> CommandResult<Vec<RemovalOutcome>> {
    state
        .storage
        .cleanup_history(limit)
        .map_err(to_command_error)
}

// ------------------------------------------------------------- duplicates --

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProgress {
    pub stage: String,
    pub done: i64,
    pub total: i64,
}

/// Start duplicate detection on a background thread.
///
/// Hashing can take a while on a large volume, so this returns immediately and
/// reports through events, the same shape as a scan.
#[tauri::command]
pub fn start_duplicate_search(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: i64,
) -> CommandResult<()> {
    let storage = state.storage.clone();
    let cancel = state.duplicate_cancel.clone();
    cancel.reset();

    std::thread::Builder::new()
        .name("diskmate-duplicates".into())
        .spawn(move || {
            let emitter = app.clone();
            let outcome = storage.find_duplicates(scan_id, &cancel, move |stage, done, total| {
                let _ = emitter.emit(
                    events::DUPLICATE_PROGRESS,
                    DuplicateProgress {
                        stage: stage.to_string(),
                        done,
                        total,
                    },
                );
            });
            match outcome {
                Ok(stats) => {
                    let _ = storage.propose_duplicate_cleanup(scan_id);
                    let _ = app.emit(events::DUPLICATE_FINISHED, stats);
                }
                Err(error) => {
                    let _ = app.emit(events::DUPLICATE_FAILED, error.to_string());
                }
            }
        })
        .map_err(|e| format!("could not start the duplicate search: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn cancel_duplicate_search(state: State<'_, AppState>) {
    state.duplicate_cancel.cancel();
}

#[tauri::command]
pub fn duplicate_groups(
    state: State<'_, AppState>,
    scan_id: i64,
    limit: u32,
) -> CommandResult<Vec<DuplicateGroup>> {
    state
        .storage
        .duplicate_groups(scan_id, limit)
        .map_err(to_command_error)
}

// ----------------------------------------------------------- applications --

#[tauri::command]
pub fn inspect_applications(state: State<'_, AppState>) -> CommandResult<Vec<Application>> {
    state
        .storage
        .inspect_applications()
        .map_err(to_command_error)
}

#[tauri::command]
pub fn applications(state: State<'_, AppState>) -> CommandResult<Vec<Application>> {
    state.storage.applications().map_err(to_command_error)
}

#[tauri::command]
pub fn application_leftovers(
    state: State<'_, AppState>,
    application_id: i64,
) -> CommandResult<Vec<Leftover>> {
    state
        .storage
        .application_leftovers(application_id)
        .map_err(to_command_error)
}

/// Propose an application and its leftovers. Proposing is not removing: the
/// user still reviews the queue and approves it.
#[tauri::command]
pub fn propose_uninstall(
    state: State<'_, AppState>,
    application_id: i64,
) -> CommandResult<Vec<CleanupCandidate>> {
    state
        .storage
        .propose_uninstall(application_id)
        .map_err(to_command_error)
}

// ------------------------------------------------------------- snapshots --

#[tauri::command]
pub fn capture_snapshot(
    state: State<'_, AppState>,
    scan_id: i64,
    label: String,
) -> CommandResult<Snapshot> {
    state
        .storage
        .capture_snapshot(scan_id, &label)
        .map_err(to_command_error)
}

#[tauri::command]
pub fn snapshots(state: State<'_, AppState>, limit: u32) -> CommandResult<Vec<Snapshot>> {
    state.storage.snapshots(limit).map_err(to_command_error)
}

#[tauri::command]
pub fn delete_snapshot(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    state.storage.delete_snapshot(id).map_err(to_command_error)
}

#[tauri::command]
pub fn compare_snapshots(
    state: State<'_, AppState>,
    base_id: i64,
    target_id: i64,
) -> CommandResult<Comparison> {
    state
        .storage
        .compare_snapshots(base_id, target_id)
        .map_err(to_command_error)
}

/// Kept so the cancellation token type stays referenced from this module.
#[allow(unused)]
fn _token_in_scope(token: &CancellationToken) -> bool {
    token.is_cancelled()
}
