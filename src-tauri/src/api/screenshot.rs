//! 用户主动截图功能：全屏捕获 → 覆盖窗口框选 → 裁剪结果回传。

use tauri::{AppHandle, Emitter, Manager};
#[cfg(desktop)]
use tauri::{WebviewUrl, WebviewWindowBuilder};

use crate::ai_service::screen_analyzer::capture_screen_raw_jpeg;

/// 启动截图流程：捕获全屏 → 存储到临时状态 → 创建全屏覆盖窗口供用户框选。
#[tauri::command]
pub async fn start_screenshot(app: AppHandle) -> Result<(), String> {
    // 如果已有覆盖窗口，先关闭
    if let Some(overlay) = app.get_webview_window("screenshot-overlay") {
        let _ = overlay.close();
    }

    // 捕获全分辨率桌面截图并编码为 base64
    let jpeg_bytes = capture_screen_raw_jpeg().ok_or("屏幕捕获失败（仅支持 Windows）")?;
    let base64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &jpeg_bytes);

    // 存储到临时状态，供覆盖窗口启动后获取
    {
        let state = app.state::<crate::AppState>();
        let mut capture = state.screenshot_capture.lock().await;
        capture.full_capture_base64 = Some(base64);
        capture.overlay_label = Some("screenshot-overlay".to_string());
    }

    // 创建全屏无边框覆盖窗口（仅桌面端）
    #[cfg(desktop)]
    {
        let w = WebviewWindowBuilder::new(
            &app,
            "screenshot-overlay",
            WebviewUrl::App("screenshot-overlay.html".into()),
        )
        .title("截图选择")
        .fullscreen(true)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .shadow(false)
        .focused(true);

        // transparent 仅 Windows 支持
        #[cfg(target_os = "windows")]
        let w = w.transparent(true);

        w.build()
            .map_err(|e| format!("创建截图覆盖窗口失败: {}", e))?;

        tracing::info!("[Screenshot] Overlay window created, waiting for user selection.");
    }
    Ok(())
}

/// 覆盖窗口调用：获取之前存储的全屏截图 base64。
#[tauri::command]
pub async fn get_overlay_data(app: AppHandle) -> Result<String, String> {
    let state = app.state::<crate::AppState>();
    let capture = state.screenshot_capture.lock().await;
    capture
        .full_capture_base64
        .clone()
        .ok_or_else(|| "没有待处理的截图数据".to_string())
}

/// 覆盖窗口调用：用户确认选区，发送裁剪后的 base64 回主窗口。
#[tauri::command]
pub async fn confirm_screenshot(app: AppHandle, base64_cropped: String) -> Result<(), String> {
    // 验证 base64 数据有效
    let _ = base64::Engine::decode(&base64::prelude::BASE64_STANDARD, &base64_cropped)
        .map_err(|e| format!("截图数据无效: {}", e))?;

    // 发送事件到主窗口
    app.emit(
        "screenshot:captured",
        serde_json::json!({ "base64": base64_cropped }),
    )
    .map_err(|e| format!("发送截图事件失败: {}", e))?;

    // 清理
    cleanup(&app).await;
    tracing::info!("[Screenshot] Captured and sent to main window.");
    Ok(())
}

/// 覆盖窗口调用：用户取消截图。
#[tauri::command]
pub async fn cancel_screenshot(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("screenshot:cancelled", ());
    cleanup(&app).await;
    tracing::info!("[Screenshot] Cancelled by user.");
    Ok(())
}

async fn cleanup(app: &AppHandle) {
    // 关闭覆盖窗口
    if let Some(overlay) = app.get_webview_window("screenshot-overlay") {
        let _ = overlay.close();
    }
    // 清理临时状态
    let state = app.state::<crate::AppState>();
    let mut capture = state.screenshot_capture.lock().await;
    capture.full_capture_base64 = None;
    capture.overlay_label = None;
}
