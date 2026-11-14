//! Remembers where the window was and how big, and puts it back next time.
//!
//! The size and position are kept in memory as the window moves, and written
//! to one small file when the window closes or Mr Clean quits. On launch they
//! are restored only if that spot is still on a connected screen, so a
//! window last seen on an unplugged display does not open out of reach.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, WebviewWindow};

const FILE: &str = "window.json";
/// Smaller than this is not a size anyone chose on purpose.
const MIN_WIDTH: f64 = 900.0;
const MIN_HEIGHT: f64 = 600.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The latest geometry of the main window, in points.
#[derive(Default)]
pub struct WindowState(Mutex<Option<Geometry>>);

fn file<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join(FILE))
}

fn read_geometry(window: &WebviewWindow<impl Runtime>) -> Option<Geometry> {
    let scale = window.scale_factor().ok()?;
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some(Geometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

/// Puts the main window back where it was, if that is still on a screen.
pub fn restore<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Some(saved) = file(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Geometry>(&text).ok())
    else {
        return;
    };
    if saved.width < MIN_WIDTH || saved.height < MIN_HEIGHT {
        return;
    }
    let on_screen = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .any(|monitor| {
            let scale = monitor.scale_factor();
            let at = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            // The window's title bar must land on this screen.
            saved.x + 80.0 >= at.x
                && saved.x + 80.0 <= at.x + size.width
                && saved.y >= at.y
                && saved.y + 40.0 <= at.y + size.height
        });
    let _ = window.set_size(LogicalSize::new(saved.width, saved.height));
    if on_screen {
        let _ = window.set_position(LogicalPosition::new(saved.x, saved.y));
    }
    if let Some(state) = app.try_state::<WindowState>() {
        if let Ok(mut current) = state.0.lock() {
            *current = Some(saved);
        }
    }
}

/// Notes the main window's geometry after a move or resize. Cheap: memory only.
pub fn note<R: Runtime>(window: &WebviewWindow<R>) {
    let Some(geometry) = read_geometry(window) else {
        return;
    };
    if let Some(state) = window.app_handle().try_state::<WindowState>() {
        if let Ok(mut current) = state.0.lock() {
            *current = Some(geometry);
        }
    }
}

/// Writes the last noted geometry to disk.
pub fn save<R: Runtime>(app: &AppHandle<R>) {
    let geometry = app
        .get_webview_window("main")
        .and_then(|window| read_geometry(&window))
        .or_else(|| {
            app.try_state::<WindowState>()
                .and_then(|state| state.0.lock().ok().and_then(|current| *current))
        });
    let (Some(geometry), Some(path)) = (geometry, file(app)) else {
        return;
    };
    if geometry.width < MIN_WIDTH || geometry.height < MIN_HEIGHT {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(&geometry) {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::Geometry;

    #[test]
    fn geometry_round_trips_through_its_file_format() {
        let geometry = Geometry {
            x: 120.0,
            y: 80.0,
            width: 1280.0,
            height: 820.0,
        };
        let text = serde_json::to_string(&geometry).unwrap();
        assert_eq!(serde_json::from_str::<Geometry>(&text).unwrap(), geometry);
    }
}
