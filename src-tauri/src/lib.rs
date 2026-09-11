//! DiskMate's Tauri adapter.
//!
//! This crate contains no product logic. Its job is to own the application
//! state, expose the engine's services as IPC commands, and forward scan
//! progress to the window as events. Anything that has to make a decision
//! belongs in `diskmate-core`.

mod commands;
mod events;
mod state;

pub use state::AppState;

use tauri::Manager;

/// Build and run the application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Failing to open the index is fatal and must be visible, not
            // swallowed into a window that silently does nothing.
            let state = AppState::initialise()?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_volumes,
            commands::start_scan,
            commands::cancel_scan,
            commands::active_scan,
            commands::latest_scan,
            commands::recent_scans,
            commands::storage_snapshot,
            commands::list_children,
            commands::entry_ancestors,
            commands::directory_by_path,
            commands::scan_errors,
            commands::memory_report,
            commands::startup_report,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start DiskMate");
}
