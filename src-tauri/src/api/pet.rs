use serde::Deserialize;
use std::sync::{Arc, Mutex};
#[cfg(desktop)]
use tauri::{LogicalSize, PhysicalPosition};
use tauri::{AppHandle, Manager};

#[derive(Clone, Deserialize, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct HitTestState {
    pub solid_rects: Arc<Mutex<Vec<Rect>>>,
    pub enabled: Arc<Mutex<bool>>,
}

impl Default for HitTestState {
    fn default() -> Self {
        Self {
            solid_rects: Arc::new(Mutex::new(Vec::new())),
            enabled: Arc::new(Mutex::new(false)),
        }
    }
}

#[tauri::command]
pub fn update_solid_regions(rects: Vec<Rect>, state: tauri::State<'_, HitTestState>) {
    if let Ok(mut locked) = state.solid_rects.lock() {
        *locked = rects;
    }
}

#[tauri::command]
pub fn set_pet_mode(
    enable: bool,
    scale: Option<f64>,
    dialog_height: Option<f64>,
    previous_dialog_height: Option<f64>,
    app_handle: AppHandle,
    state: tauri::State<'_, HitTestState>,
) -> Result<(), String> {
    if let Ok(mut locked_enabled) = state.enabled.lock() {
        *locked_enabled = enable;
    }

    #[cfg(desktop)]
    if let Some(window) = app_handle.get_webview_window("main") {
        if enable {
            let scale_val = scale.unwrap_or(1.0);
            let dialog_h = dialog_height.unwrap_or(75.0);
            let prev_dialog_h = previous_dialog_height.unwrap_or(75.0);

            let width = (240.0 * scale_val) as u32;
            let height = ((240.0 + dialog_h + 45.0) * scale_val) as u32;

            let height_delta = (dialog_h - prev_dialog_h) * scale_val;

            let _ = window.set_skip_taskbar(true);
            let _ = window.set_always_on_top(true);
            let _ = window.set_resizable(false);
            let _ = window.set_decorations(false);
            let _ = window.set_maximizable(false);

            let _ = window.set_size(LogicalSize::new(width, height));

            if height_delta.abs() > f64::EPSILON {
                if let Ok(pos) = window.outer_position() {
                    let sf = window.scale_factor().unwrap_or(1.0);
                    let physical_delta = (height_delta * sf) as i32;
                    let _ =
                        window.set_position(PhysicalPosition::new(pos.x, pos.y - physical_delta));
                }
            }
        } else {
            // Restore normal window
            let _ = window.set_maximizable(true);
            let _ = window.set_skip_taskbar(false);
            let _ = window.set_always_on_top(false);
            let _ = window.set_resizable(true);
            let _ = window.set_decorations(true);
            let _ = window.set_size(LogicalSize::new(1500, 800));
            // Center the window on screen so it doesn't expand from the pet's top-left corner
            let _ = window.center();
            // Always restore cursor ignore to false
            let _ = window.set_ignore_cursor_events(false);
        }
    }
    Ok(())
}
