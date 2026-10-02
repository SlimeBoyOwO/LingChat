const COMMANDS: &[&str] = &[
    "get_screenshotable_windows",
    "get_screenshotable_monitors",
    "get_window_screenshot",
    "get_monitor_screenshot",
    "remove_window_screenshot",
    "remove_monitor_screenshot",
    "clear_screenshots",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
