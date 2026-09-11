// Suppress the console window on Windows release builds. Harmless elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    diskmate_app::run();
}
