//! 系统托盘与轻量模式（demo）。
//!
//! 轻量模式不销毁 webview：前端先卸载角色舞台等渲染资源，再把主窗口
//! `hide()`。页面仍在内存里，事件监听与游戏状态保持连续；左键点击托盘
//! 图标还原窗口，并通过 `lite-mode:exit` 通知前端重新挂载渲染资源。

use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

const MENU_SHOW: &str = "tray-show";
const MENU_QUIT: &str = "tray-quit";

/// 创建托盘图标，桌面端在 setup 末尾调用一次。
pub fn setup(app: &tauri::App<tauri::Wry>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "显示主界面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出 LingChat", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        // 左键留给“还原窗口”，菜单走右键
        .show_menu_on_left_click(false)
        .tooltip("LingChat")
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_SHOW => restore_main_window(app),
            MENU_QUIT => app.exit(0),
            _ => {},
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                restore_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    // TrayIcon 被 Drop 时会移除图标，交给 Tauri 管理其生命周期。
    let tray = builder.build(app)?;
    app.manage(tray);
    Ok(())
}

/// 显示并聚焦主窗口，同时通知前端退出轻量模式。
fn restore_main_window(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_focus();
    let _ = app.emit_to("main", "lite-mode:exit", ());
}

/// 进入托盘轻量模式：隐藏主窗口。前端应先完成渲染资源卸载再调用。
#[tauri::command]
pub fn enter_tray_mode(app: AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    win.hide().map_err(|e| format!("隐藏主窗口失败: {e}"))
}
