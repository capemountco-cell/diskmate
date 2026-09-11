//! Storage queries: volumes, scan history, and navigation over the index.

use diskmate_core::db::ChildQuery;
use diskmate_core::model::{EntryRow, ScanError, ScanSummary, VolumeInfo};
use diskmate_core::services::StorageSnapshot;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

use super::{to_command_error, CommandResult};

/// What the UI needs to know about this installation on launch.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub engine: String,
    pub platform: String,
    pub os: String,
    pub arch: String,
    pub data_dir: String,
    pub database_path: String,
    pub storage_mode: String,
    /// Set when DiskMate wanted to run portably but the media was not
    /// writable, so the technician knows state landed on the host instead.
    pub portable_fallback_reason: Option<String>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    let paths = state.storage.paths();
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        engine: diskmate_core::ENGINE_VERSION.to_string(),
        platform: state.storage.platform_name().to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        data_dir: paths.data_dir.to_string_lossy().into_owned(),
        database_path: paths.database.to_string_lossy().into_owned(),
        storage_mode: paths.kind.to_string(),
        portable_fallback_reason: paths.portable_fallback_reason.clone(),
    }
}

#[tauri::command]
pub fn list_volumes(state: State<'_, AppState>) -> CommandResult<Vec<VolumeInfo>> {
    state.storage.volumes().map_err(to_command_error)
}

#[tauri::command]
pub fn latest_scan(state: State<'_, AppState>, root: String) -> CommandResult<Option<ScanSummary>> {
    state
        .storage
        .latest_scan(std::path::Path::new(&root))
        .map_err(to_command_error)
}

#[tauri::command]
pub fn recent_scans(state: State<'_, AppState>, limit: u32) -> CommandResult<Vec<ScanSummary>> {
    state.storage.recent_scans(limit).map_err(to_command_error)
}

#[tauri::command]
pub fn storage_snapshot(
    state: State<'_, AppState>,
    scan_id: i64,
) -> CommandResult<StorageSnapshot> {
    state.storage.snapshot(scan_id).map_err(to_command_error)
}

/// One page of a directory listing, plus the total so the UI can page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildPage {
    pub rows: Vec<EntryRow>,
    pub total: i64,
}

#[tauri::command]
pub fn list_children(state: State<'_, AppState>, query: ChildQuery) -> CommandResult<ChildPage> {
    let rows = state.storage.children(&query).map_err(to_command_error)?;
    let total = state
        .storage
        .child_count(query.scan_id, query.parent_id, query.hide_hidden)
        .map_err(to_command_error)?;
    Ok(ChildPage { rows, total })
}

#[tauri::command]
pub fn entry_ancestors(
    state: State<'_, AppState>,
    scan_id: i64,
    entry_id: i64,
) -> CommandResult<Vec<EntryRow>> {
    state.storage.ancestors(scan_id, entry_id).map_err(to_command_error)
}

#[tauri::command]
pub fn directory_by_path(
    state: State<'_, AppState>,
    scan_id: i64,
    path: String,
) -> CommandResult<Option<EntryRow>> {
    state.storage.directory_by_path(scan_id, &path).map_err(to_command_error)
}

#[tauri::command]
pub fn scan_errors(
    state: State<'_, AppState>,
    scan_id: i64,
    limit: u32,
) -> CommandResult<Vec<ScanError>> {
    state.storage.scan_errors(scan_id, limit).map_err(to_command_error)
}
