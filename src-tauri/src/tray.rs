//! The menu bar: Mr Clean's broom, memory in use beside it, and a panel.
//!
//! Clicking the broom opens a small window drawn by the same interface as the
//! app (the "panel" window): memory now, the last few minutes, the apps using
//! the most with a Quit for each, free space, and a way into wipe-down mode.
//! Clicking away closes it. Right-clicking shows a plain menu with Open and
//! Quit, for anyone who expects one.
//!
//! The broom is a template image, so macOS draws it white or black to match
//! the menu bar. It turns orange when memory is under pressure and red when
//! pressure is high: colour only when something is true.
//!
//! Reading is cheap (one memory snapshot), so the title refreshes every few
//! seconds for as long as Mr Clean is running. Closing the main window keeps
//! Mr Clean in the menu bar; Quit in the panel or the menu ends it.

use std::time::Duration;

use diskmate_core::model::MemoryPressure;
use diskmate_core::services::MemoryService;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

const TRAY_ID: &str = "memory";
pub const PANEL: &str = "panel";
const REFRESH: Duration = Duration::from_secs(3);
/// The panel's size in points. The interface lays itself out to fit.
const PANEL_WIDTH: f64 = 360.0;
const PANEL_HEIGHT: f64 = 660.0;
/// The broom: 36 × 36 RGBA, black with alpha, drawn for 18 points at 2x.
const BROOM: &[u8] = include_bytes!("../icons/tray-broom.rgba");
const BROOM_SIZE: u32 = 36;

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    build_panel(app)?;

    let open = MenuItem::with_id(app, "open", "Open Mr Clean", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Quit Mr Clean", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &exit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(broom(MemoryPressure::Unknown))
        .icon_as_template(true)
        .tooltip("Mr Clean")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "exit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                toggle_panel(tray.app_handle(), rect);
            }
        })
        .build(app)?;

    let handle = app.clone();
    std::thread::Builder::new()
        .name("mrclean-menubar".into())
        .spawn(move || {
            let memory = MemoryService::new();
            loop {
                refresh(&handle, &memory);
                std::thread::sleep(REFRESH);
            }
        })?;

    Ok(())
}

/// The panel window, made once and hidden; the broom shows and hides it.
fn build_panel<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(app, PANEL, WebviewUrl::App("index.html".into()))
        .title("Mr Clean")
        .inner_size(PANEL_WIDTH, PANEL_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .shadow(true)
        .visible(false)
        .build()
}

fn toggle_panel<R: Runtime>(app: &AppHandle<R>, icon: tauri::Rect) {
    let Some(panel) = app.get_webview_window(PANEL) else {
        return;
    };
    if panel.is_visible().unwrap_or(false) {
        let _ = panel.hide();
        return;
    }
    let scale = panel.scale_factor().unwrap_or(2.0);
    let at = icon.position.to_physical::<f64>(scale);
    let size = icon.size.to_physical::<f64>(scale);
    let width = PANEL_WIDTH * scale;
    // Centred under the broom, a few points below the menu bar, and kept on
    // screen at the right-hand edge.
    let mut x = at.x + size.width / 2.0 - width / 2.0;
    if let Ok(Some(monitor)) = panel.current_monitor() {
        let right = f64::from(monitor.position().x) + f64::from(monitor.size().width);
        x = x.min(right - width - 8.0 * scale);
    }
    let y = at.y + size.height + 6.0 * scale;
    let _ = panel.set_position(PhysicalPosition::new(x.max(8.0 * scale), y));
    let _ = panel.show();
    let _ = panel.set_focus();
}

pub fn hide_panel<R: Runtime>(app: &AppHandle<R>) {
    if let Some(panel) = app.get_webview_window(PANEL) {
        let _ = panel.hide();
    }
}

fn refresh<R: Runtime>(app: &AppHandle<R>, memory: &MemoryService) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match memory.snapshot() {
        Ok(snapshot) => {
            let alert = matches!(
                snapshot.pressure,
                MemoryPressure::Warning | MemoryPressure::Critical
            );
            let _ = tray.set_title(Some(gb(snapshot.used_bytes)));
            let _ = tray.set_icon(Some(broom(snapshot.pressure)));
            // Template images take the menu bar's own colour; an alert
            // needs its colour kept.
            let _ = tray.set_icon_as_template(!alert);
            let _ = tray.set_tooltip(Some(format!(
                "Mr Clean: {} of {} memory in use, pressure {}",
                gb(snapshot.used_bytes),
                gb(snapshot.total_bytes),
                pressure_word(snapshot.pressure)
            )));
        }
        Err(_) => {
            let _ = tray.set_title(Some("–"));
        }
    }
}

pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    hide_panel(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn pressure_word(pressure: MemoryPressure) -> &'static str {
    match pressure {
        MemoryPressure::Normal => "normal",
        MemoryPressure::Warning => "elevated",
        MemoryPressure::Critical => "high",
        MemoryPressure::Unknown => "unknown",
    }
}

/// Decimal gigabytes, matching the figures in the window.
fn gb(bytes: i64) -> String {
    let value = bytes.max(0) as f64 / 1e9;
    if value >= 100.0 {
        format!("{value:.0} GB")
    } else {
        format!("{value:.1} GB")
    }
}

/// The broom, black for the template, or in a status colour when pressure is
/// up (the same orange and red the app uses).
fn broom(pressure: MemoryPressure) -> Image<'static> {
    let colour = match pressure {
        MemoryPressure::Warning => Some((251, 146, 60)),
        MemoryPressure::Critical => Some((248, 113, 113)),
        MemoryPressure::Normal | MemoryPressure::Unknown => None,
    };
    let mut rgba = BROOM.to_vec();
    if let Some((r, g, b)) = colour {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
    }
    Image::new_owned(rgba, BROOM_SIZE, BROOM_SIZE)
}

#[cfg(test)]
mod tests {
    use super::{broom, gb, BROOM, BROOM_SIZE};
    use diskmate_core::model::MemoryPressure;

    #[test]
    fn gigabytes_read_like_the_window() {
        assert_eq!(gb(11_200_000_000), "11.2 GB");
        assert_eq!(gb(128_000_000_000), "128 GB");
        assert_eq!(gb(3_840_000_000), "3.8 GB");
        assert_eq!(gb(0), "0.0 GB");
        assert_eq!(gb(-5), "0.0 GB");
    }

    #[test]
    fn the_broom_is_a_whole_image() {
        assert_eq!(BROOM.len(), (BROOM_SIZE * BROOM_SIZE * 4) as usize);
        let _ = broom(MemoryPressure::Critical);
    }
}
