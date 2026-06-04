use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

mod commands;

#[cfg(target_os = "windows")]
pub mod windows;

pub use commands::*;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("screenshots")
        .invoke_handler(tauri::generate_handler![
            commands::get_screenshotable_windows,
            commands::get_screenshotable_monitors,
            commands::get_window_screenshot,
            commands::get_monitor_screenshot,
            commands::remove_window_screenshot,
            commands::remove_monitor_screenshot,
            commands::clear_screenshots
        ])
        .build()
}
