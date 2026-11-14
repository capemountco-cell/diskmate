//! Mr Clean's Tauri adapter.
//!
//! This crate contains no product logic. Its job is to own the application
//! state, expose the engine's services as IPC commands, and forward scan
//! progress to the window as events. Anything that has to make a decision
//! belongs in `diskmate-core`.

mod commands;
mod events;
mod state;
mod tray;
mod window_state;
mod wipe;

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
            app.manage(window_state::WindowState::default());
            window_state::restore(app.handle());
            tray::install(app.handle())?;
            Ok(())
        })
        // Closing the window keeps Mr Clean running in the menu bar, so the
        // memory readout stays visible. "Quit Mr Clean" in that menu ends it.
        .on_window_event(|window, event| match event {
            // Closing the window keeps Mr Clean in the menu bar.
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if window.label() == "main" {
                    window_state::save(window.app_handle());
                }
                let _ = window.hide();
            }
            // Remember where the window is, in memory, as it moves.
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                if window.label() == "main" =>
            {
                if let Some(webview) = window.app_handle().get_webview_window("main") {
                    window_state::note(&webview);
                }
            }
            // The menu bar panel closes when you click anywhere else.
            tauri::WindowEvent::Focused(false) if window.label() == tray::PANEL => {
                let _ = window.hide();
            }
            _ => {}
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
            commands::subtree,
            commands::age_breakdown,
            commands::memory_report,
            commands::top_apps,
            commands::quit_app,
            commands::app_icon,
            commands::startup_report,
            // Cleanup. Every one of these proposes or reviews; only
            // `remove_approved` removes, and the engine checks the approval
            // against the queue before it does.
            commands::propose_cleanup,
            commands::cleanup_groups,
            commands::cleanup_candidates,
            commands::stage_cleanup,
            commands::unstage_cleanup,
            commands::staged_cleanup,
            commands::remove_approved,
            commands::cleanup_history,
            // Duplicates.
            commands::start_duplicate_search,
            commands::cancel_duplicate_search,
            commands::duplicate_groups,
            // Applications and the uninstaller.
            commands::inspect_applications,
            commands::applications,
            commands::application_leftovers,
            commands::propose_uninstall,
            // Snapshots.
            commands::capture_snapshot,
            commands::snapshots,
            commands::delete_snapshot,
            commands::compare_snapshots,
            // The live monitor.
            commands::monitor_start,
            commands::monitor_stop,
            commands::monitor_report,
            commands::monitor_sample,
            commands::open_main_window,
            commands::hide_panel,
            commands::quit_mr_clean,
            wipe::wipe_permission,
            wipe::wipe_request_permission,
            wipe::wipe_start,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Mr Clean")
        .run(|app, event| {
            // Clicking the Dock icon brings the hidden window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                tray::show_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
