use std::fs;

use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

use crate::utils::path::validate_path_in_base;

use super::ambient_dir;

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AmbientItemInfo {
    pub name: String,
    pub url: String,
    pub time: String,
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub fn get_ambient_list() -> Result<Vec<AmbientItemInfo>, String> {
    let ambient_dir = ambient_dir();

    if !ambient_dir.exists() {
        return Ok(Vec::new());
    }

    let allowed_extensions = ["mp3", "wav", "flac", "webm", "weba", "ogg", "m4a", "oga"];

    let mut items: Vec<AmbientItemInfo> = Vec::new();

    let entries = fs::read_dir(&ambient_dir).map_err(|e| format!("读取环境音目录失败: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !allowed_extensions.contains(&ext.to_lowercase().as_str()) {
            continue;
        }

        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let time = path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64().to_string())
                    .unwrap_or_else(|_| "0".to_string())
            })
            .unwrap_or_else(|| "0".to_string());

        let url = path.to_string_lossy().into_owned();

        items.push(AmbientItemInfo { name, url, time });
    }

    items.sort_by(|a, b| {
        b.time
            .parse::<f64>()
            .unwrap_or(0.0)
            .partial_cmp(&a.time.parse::<f64>().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(items)
}

#[tauri::command]
pub async fn upload_ambient(app: tauri::AppHandle, path: String, file_name: String) -> Result<(), String> {
    // 安全检查：只保留文件名，防止路径遍历
    let safe_name = std::path::Path::new(&file_name)
        .file_name()
        .ok_or_else(|| format!("无效的文件名: {}", file_name))?
        .to_string_lossy()
        .into_owned();

    let ambient_dir = ambient_dir();
    if !ambient_dir.exists() {
        tokio::fs::create_dir_all(&ambient_dir)
            .await
            .map_err(|e| format!("创建环境音目录失败: {}", e))?;
    }

    let file_path = ambient_dir.join(&safe_name);

    if path.starts_with("content://") {
        // Android SAF：content:// URI 直接复制到目标文件（不经 IPC 传大文件）
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        app.android_fs_async()
            .copy(&FsUri::from_uri(&path), &FsUri::from_path(&file_path))
            .await
            .map_err(|e| format!("SAF 复制环境音失败: {}", e))?;
    } else {
        // 桌面端：Rust 直接复制源文件
        tokio::fs::copy(std::path::PathBuf::from(&path), &file_path)
            .await
            .map_err(|e| format!("复制文件失败: {}", e))?;
    }

    Ok(())
}

/// 删除指定环境音文件
/// url 参数可以是完整路径或纯文件名，统一从 ambient_dir 中删除
#[tauri::command]
pub fn delete_ambient(url: String) -> Result<Vec<AmbientItemInfo>, String> {
    let base = ambient_dir();

    // 从路径中提取文件名，兼容完整路径和纯文件名
    let filename = std::path::Path::new(&url)
        .file_name()
        .ok_or_else(|| format!("无效的文件路径: {}", url))?
        .to_string_lossy()
        .into_owned();

    let file_path = base.join(&filename);
    validate_path_in_base(&file_path, &base)?;

    if !file_path.exists() {
        return Err(format!("环境音文件不存在: {}", filename));
    }

    fs::remove_file(&file_path).map_err(|e| format!("删除环境音文件失败: {}", e))?;

    get_ambient_list()
}

// ========== 会话状态持久化 ==========

/// 持久化环境音轨道列表到 settings.json，下次启动时自动恢复。
#[tauri::command]
pub fn save_ambient_state(
    app: tauri::AppHandle,
    tracks_json: String,
) -> Result<(), String> {
    let store = app
        .store(crate::config::STORE_FILE)
        .map_err(|e| format!("打开存储失败: {e}"))?;
    store.set(
        crate::config::session::LAST_AMBIENT_TRACKS.to_string(),
        serde_json::Value::String(tracks_json),
    );
    store.save().map_err(|e| format!("保存失败: {e}"))
}
