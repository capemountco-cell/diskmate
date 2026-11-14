//! The windows: opening the app from the menu bar panel, closing the panel,
//! and quitting. None of these touch files or other apps.

use tauri::{AppHandle, Runtime};

use crate::tray;

#[tauri::command]
pub fn open_main_window<R: Runtime>(app: AppHandle<R>) {
    tray::show_window(&app);
}

#[tauri::command]
pub fn hide_panel<R: Runtime>(app: AppHandle<R>) {
    tray::hide_panel(&app);
}

#[tauri::command]
pub fn quit_mr_clean<R: Runtime>(app: AppHandle<R>) {
    app.exit(0);
}
