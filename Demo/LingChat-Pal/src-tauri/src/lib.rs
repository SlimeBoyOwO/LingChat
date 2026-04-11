use std::sync::{Arc, Mutex};
use serde::Deserialize;
use tauri::{Manager, Emitter, menu::{MenuBuilder, MenuItem}, tray::TrayIconBuilder};
use tauri::async_runtime::spawn;

#[derive(Clone, Deserialize, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct HitTestState {
    pub solid_rects: Arc<Mutex<Vec<Rect>>>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn update_solid_regions(rects: Vec<Rect>, state: tauri::State<'_, HitTestState>) {
    if let Ok(mut locked) = state.solid_rects.lock() {
        *locked = rects;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let hit_test_state = HitTestState {
        solid_rects: Arc::new(Mutex::new(Vec::new())),
    };

    let solid_rects_clone = hit_test_state.solid_rects.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(hit_test_state)
        .invoke_handler(tauri::generate_handler![greet, update_solid_regions])
        .setup(move |app| {
            // 1. Setup tray menu
            let toggle_i = MenuItem::with_id(app, "toggle", "显示/隐藏", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = MenuBuilder::new(app)
                .item(&toggle_i)
                .item(&settings_i)
                .item(&quit_i)
                .build()?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "toggle" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        "settings" => {
                            let _ = app.emit("open-settings", ());
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // 2. Setup mouse poller for click-through
            let window = app.get_webview_window("main").unwrap();
            let rects_arc = solid_rects_clone;

            spawn(async move {
                let mut was_ignored = false;
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    
                    #[cfg(target_os = "windows")]
                    {
                        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
                        use windows::Win32::Foundation::POINT;

                        let mut pt = POINT { x: 0, y: 0 };
                        unsafe {
                            let _ = GetCursorPos(&mut pt);
                        }

                        if let Ok(window_pos) = window.outer_position() {
                            if let Ok(scale_factor) = window.scale_factor() {
                                // Calculate logical mouse coords relative to window
                                let mouse_x = f64::from(pt.x) - f64::from(window_pos.x);
                                let mouse_y = f64::from(pt.y) - f64::from(window_pos.y);
                                
                                let logical_x = mouse_x / scale_factor;
                                let logical_y = mouse_y / scale_factor;

                                let mut is_over_solid = false;
                                if let Ok(rects) = rects_arc.lock() {
                                    for r in rects.iter() {
                                        if logical_x >= r.x && logical_y >= r.y 
                                            && logical_x <= (r.x + r.width) 
                                            && logical_y <= (r.y + r.height) {
                                            is_over_solid = true;
                                            break;
                                        }
                                    }
                                }

                                if is_over_solid {
                                    if was_ignored {
                                        let _ = window.set_ignore_cursor_events(false);
                                        was_ignored = false;
                                    }
                                } else {
                                    if !was_ignored {
                                        let _ = window.set_ignore_cursor_events(true);
                                        was_ignored = true;
                                    }
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
