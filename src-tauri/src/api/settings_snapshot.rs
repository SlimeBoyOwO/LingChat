use tauri::AppHandle;

#[cfg(target_os = "windows")]
use tauri::Manager;

/// 捕获设置背景快照（Windows 专用）。
///
/// 与 `save::capture_main_window_screenshot` 共用 HWND 截屏链路，但写入独立前缀
/// `lingchat_settings_bg_<pid>_<ts>_<rand>.png`，与存档截图 `lingchat_screenshot_*`
/// 彻底隔离，避免互相覆盖/误删。
#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn capture_settings_snapshot(app: AppHandle) -> Result<String, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let hwnd = window
        .hwnd()
        .map_err(|e| format!("获取窗口句柄失败: {}", e))?;

    let id = hwnd.0 as usize as u32;
    let image = tauri_plugin_screenshots::windows::capture_own_window(id)?;

    let pid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    // 加 4 位随机避免同一毫秒并发碰撞
    let rand: u16 = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        std::thread::current().id().hash(&mut h);
        ts.hash(&mut h);
        (h.finish() & 0xFFFF) as u16
    };

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!(
        "lingchat_settings_bg_{}_{}_{}.png",
        pid, ts, rand
    ));
    image
        .save(&temp_path)
        .map_err(|e| format!("保存设置快照失败: {}", e))?;

    tracing::info!(
        "[capture_settings_snapshot] Captured → {}",
        temp_path.display()
    );
    Ok(temp_path.to_string_lossy().to_string())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn capture_settings_snapshot(_app: AppHandle) -> Result<String, String> {
    Err("capture_settings_snapshot is only available on Windows".to_string())
}

/// 清理设置快照临时文件。
///
/// 仅允许删除文件名含 `lingchat_settings_bg_` 且位于系统临时目录下的文件，
/// 避免误删任意路径。
#[tauri::command]
pub async fn cleanup_settings_snapshot(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);

    // 文件名必须含前缀，否则拒绝
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if !file_name.contains("lingchat_settings_bg_") {
        return Err(format!("拒绝删除非设置快照文件: {}", path));
    }

    // 必须位于 temp_dir 内（或其子目录），防止路径穿越
    let temp_dir = std::env::temp_dir();
    // 规范化比较：若 canonicalize 失败则用前缀字符串比较兜底
    let allowed = match (p.canonicalize(), temp_dir.canonicalize()) {
        (Ok(cp), Ok(ct)) => cp.starts_with(&ct),
        _ => {
            // 回退：检查路径字符串是否以 temp_dir 开头
            let s = p.to_string_lossy();
            let t = temp_dir.to_string_lossy();
            s.starts_with(t.as_ref())
        }
    };
    // 额外允许：未落盘的路径（文件不存在）也视为成功，直接返回
    if !allowed {
        // 若文件不存在，也不宜报错；但若路径不在 temp 内则拒绝
        if p.exists() {
            return Err(format!("拒绝删除非临时目录文件: {}", path));
        } else {
            // 文件已不存在，视为已清理
            return Ok(());
        }
    }

    if p.exists() {
        std::fs::remove_file(p).map_err(|e| format!("删除设置快照失败: {}: {}", path, e))?;
        tracing::info!("[cleanup_settings_snapshot] Removed → {}", path);
    }
    Ok(())
}
