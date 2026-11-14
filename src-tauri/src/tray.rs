//! The menu bar readout.
//!
//! Shows memory in use and a pressure dot in the menu bar, all the time, and a
//! menu with the apps using the most memory. Each app that can be asked to quit
//! has a Quit item; that goes through `ProcessService` like the Quit buttons in
//! the window, so the same checks apply.
//!
//! Reading is cheap (one memory snapshot and one process walk), so it refreshes
//! every few seconds for as long as Mr Clean is running. Closing the window
//! keeps Mr Clean in the menu bar; "Quit Mr Clean" in this menu ends it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use diskmate_core::model::{MemoryPressure, MemorySnapshot, ProcessMemory};
use diskmate_core::services::{MemoryService, ProcessService};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime};

const TRAY_ID: &str = "memory";
const REFRESH: Duration = Duration::from_secs(3);
const APP_SLOTS: usize = 5;

/// One row in the "using the most memory" part of the menu.
struct Slot<R: Runtime> {
    menu: Submenu<R>,
    quit: MenuItem<R>,
}

/// Which app each slot currently shows, so a click quits what was on screen.
type Shown = Arc<Mutex<Vec<Option<(i32, String)>>>>;

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let used = MenuItem::with_id(app, "info:used", "Reading memory…", false, None::<&str>)?;
    let pressure = MenuItem::with_id(app, "info:pressure", "", false, None::<&str>)?;
    let swap = MenuItem::with_id(app, "info:swap", "", false, None::<&str>)?;
    let header = MenuItem::with_id(
        app,
        "info:header",
        "Using the most memory",
        false,
        None::<&str>,
    )?;

    let mut slots = Vec::with_capacity(APP_SLOTS);
    for index in 0..APP_SLOTS {
        let quit = MenuItem::with_id(app, format!("quit:{index}"), "Quit", false, None::<&str>)?;
        let menu = Submenu::with_items(app, "", false, &[&quit])?;
        slots.push(Slot { menu, quit });
    }

    let open = MenuItem::with_id(app, "open", "Open Mr Clean", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Quit Mr Clean", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::new(app)?;
    menu.append_items(&[
        &used,
        &pressure,
        &swap,
        &PredefinedMenuItem::separator(app)?,
        &header,
    ])?;
    for slot in &slots {
        menu.append(&slot.menu)?;
    }
    menu.append_items(&[&PredefinedMenuItem::separator(app)?, &open, &exit])?;

    let shown: Shown = Arc::new(Mutex::new(vec![None; APP_SLOTS]));
    let clicked = shown.clone();

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(dot(MemoryPressure::Unknown))
        .icon_as_template(false)
        .tooltip("Mr Clean: memory")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "exit" => app.exit(0),
            id => {
                let Some(index) = id
                    .strip_prefix("quit:")
                    .and_then(|i| i.parse::<usize>().ok())
                else {
                    return;
                };
                let target = clicked
                    .lock()
                    .ok()
                    .and_then(|s| s.get(index).cloned().flatten());
                if let Some((pid, name)) = target {
                    if let Err(error) = ProcessService::new().quit_app(pid, &name) {
                        eprintln!("Mr Clean: quit from the menu bar was refused: {error}");
                    }
                }
            }
        })
        .build(app)?;

    let handle = app.clone();
    std::thread::Builder::new()
        .name("diskmate-menubar".into())
        .spawn(move || {
            let memory = MemoryService::new();
            let processes = ProcessService::new();
            loop {
                refresh(
                    &handle, &memory, &processes, &used, &pressure, &swap, &slots, &shown,
                );
                std::thread::sleep(REFRESH);
            }
        })?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn refresh<R: Runtime>(
    app: &AppHandle<R>,
    memory: &MemoryService,
    processes: &ProcessService,
    used: &MenuItem<R>,
    pressure: &MenuItem<R>,
    swap: &MenuItem<R>,
    slots: &[Slot<R>],
    shown: &Shown,
) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    match memory.snapshot() {
        Ok(snapshot) => {
            let _ = tray.set_title(Some(gb(snapshot.used_bytes)));
            let _ = tray.set_icon(Some(dot(snapshot.pressure)));
            let _ = used.set_text(used_line(&snapshot));
            let _ = pressure.set_text(format!("Pressure: {}", pressure_word(snapshot.pressure)));
            let _ = swap.set_text(match snapshot.swap_used_bytes {
                Some(bytes) => format!("Swap in use: {}", gb(bytes)),
                None => "Swap: not reported".to_string(),
            });
        }
        Err(error) => {
            let _ = tray.set_title(Some("–"));
            let _ = used.set_text(format!("Memory unavailable: {error}"));
        }
    }

    let apps: Vec<ProcessMemory> = processes.top_apps(APP_SLOTS).unwrap_or_default();
    let mut current = vec![None; APP_SLOTS];
    for (index, slot) in slots.iter().enumerate() {
        match apps.get(index) {
            Some(app) => {
                let _ = slot
                    .menu
                    .set_text(format!("{}    {}", app.name, gb(app.resident_bytes)));
                let _ = slot.menu.set_enabled(true);
                if app.can_quit {
                    let _ = slot.quit.set_text(format!("Quit {}", app.name));
                    let _ = slot.quit.set_enabled(true);
                    current[index] = Some((app.pid, app.name.clone()));
                } else {
                    let _ = slot.quit.set_text("Runs in the background; not offered");
                    let _ = slot.quit.set_enabled(false);
                }
            }
            None => {
                let _ = slot.menu.set_text("");
                let _ = slot.menu.set_enabled(false);
                let _ = slot.quit.set_enabled(false);
            }
        }
    }
    if let Ok(mut guard) = shown.lock() {
        *guard = current;
    }
}

pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn used_line(snapshot: &MemorySnapshot) -> String {
    format!(
        "Memory in use: {} of {}",
        gb(snapshot.used_bytes),
        gb(snapshot.total_bytes)
    )
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

/// A filled circle in the pressure colour. Drawn here rather than shipped as
/// image files: three colours of one circle do not need assets.
fn dot(pressure: MemoryPressure) -> Image<'static> {
    let (r, g, b) = match pressure {
        MemoryPressure::Normal => (52, 199, 89),
        MemoryPressure::Warning => (255, 196, 0),
        MemoryPressure::Critical => (255, 69, 58),
        MemoryPressure::Unknown => (142, 142, 147),
    };
    const SIZE: u32 = 36;
    let centre = (SIZE as f64 - 1.0) / 2.0;
    let radius = 10.5;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let distance = ((x as f64 - centre).powi(2) + (y as f64 - centre).powi(2)).sqrt();
            // One pixel of anti-aliasing at the edge.
            let alpha = ((radius + 0.5 - distance).clamp(0.0, 1.0) * 255.0) as u8;
            rgba.extend_from_slice(&[r, g, b, alpha]);
        }
    }
    Image::new_owned(rgba, SIZE, SIZE)
}

#[cfg(test)]
mod tests {
    use super::gb;

    #[test]
    fn gigabytes_read_like_the_window() {
        assert_eq!(gb(11_200_000_000), "11.2 GB");
        assert_eq!(gb(128_000_000_000), "128 GB");
        assert_eq!(gb(3_840_000_000), "3.8 GB");
        assert_eq!(gb(0), "0.0 GB");
        assert_eq!(gb(-5), "0.0 GB");
    }
}
