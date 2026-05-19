use std::sync::{Arc, Mutex};
use serde::Deserialize;
use tauri::{AppHandle, Manager, LogicalSize};

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
    app_handle: AppHandle,
    state: tauri::State<'_, HitTestState>,
) -> Result<(), String> {
    if let Ok(mut locked_enabled) = state.enabled.lock() {
        *locked_enabled = enable;
    }

    if let Some(window) = app_handle.get_webview_window("main") {
        if enable {
            let scale_val = scale.unwrap_or(1.0);
            
            // Calculate sizes based on pet dimensions: BASE_AVATAR_SIZE = 240, CHAT_BASE_H = 45, DIALOG_BASE_H = 75
            // But let's check: GameRoleAvatar frame size is Math.round(210 * scale). Let's use standard pet size:
            // Width: 240 * scale, Height: (240 + 75 + 45) * scale = 360 * scale.
            let width = (240.0 * scale_val) as u32;
            let height = ((240.0 + 75.0 + 45.0) * scale_val) as u32;

            let _ = window.set_skip_taskbar(true);
            let _ = window.set_always_on_top(true);
            let _ = window.set_resizable(false);
            let _ = window.set_decorations(false);
            let _ = window.set_size(LogicalSize::new(width, height));
        } else {
            // Restore normal window
            let _ = window.set_skip_taskbar(false);
            let _ = window.set_always_on_top(false);
            let _ = window.set_resizable(true);
            let _ = window.set_decorations(true);
            let _ = window.set_size(LogicalSize::new(1500, 800));
            // Always restore cursor ignore to false
            let _ = window.set_ignore_cursor_events(false);
        }
    }
    Ok(())
}
