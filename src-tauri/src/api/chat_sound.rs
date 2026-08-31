use std::fs;

use serde::{Deserialize, Serialize};

use crate::utils::path::validate_path_in_base;

use super::chat_sounds_dir;

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatSoundItemInfo {
    pub name: String,
    pub url: String,
    pub time: String,
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub fn get_chat_sound_list() -> Result<Vec<ChatSoundItemInfo>, String> {
    let chat_sounds_dir = chat_sounds_dir();

    if !chat_sounds_dir.exists() {
        return Ok(Vec::new());
    }

    // 与前端对话框过滤列表保持一致（SettingsSound.vue triggerChatSoundUpload），
    // 双端白名单漂移会导致选不中的文件悄悄出现在列表里（或反之）
    let allowed_extensions = ["mp3", "wav", "flac", "ogg", "m4a"];

    let mut items: Vec<ChatSoundItemInfo> = Vec::new();

    let entries = fs::read_dir(&chat_sounds_dir).map_err(|e| format!("读取聊天音效目录失败: {}", e))?;

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

        items.push(ChatSoundItemInfo { name, url, time });
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
pub async fn upload_chat_sound(app: tauri::AppHandle, path: String, file_name: String) -> Result<(), String> {
    // 安全检查：只保留文件名，防止路径遍历
    let safe_name = std::path::Path::new(&file_name)
        .file_name()
        .ok_or_else(|| format!("无效的文件名: {}", file_name))?
        .to_string_lossy()
        .into_owned();

    let chat_sounds_dir = chat_sounds_dir();
    if !chat_sounds_dir.exists() {
        tokio::fs::create_dir_all(&chat_sounds_dir)
            .await
            .map_err(|e| format!("创建聊天音效目录失败: {}", e))?;
    }

    let file_path = chat_sounds_dir.join(&safe_name);

    if path.starts_with("content://") {
        // Android SAF：content:// URI 直接复制到目标文件（不经 IPC 传大文件）
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        app.android_fs_async()
            .copy(&FsUri::from_uri(&path), &FsUri::from_path(&file_path))
            .await
            .map_err(|e| format!("SAF 复制聊天音效失败: {}", e))?;
    } else {
        // 桌面端：Rust 直接复制源文件
        tokio::fs::copy(std::path::PathBuf::from(&path), &file_path)
            .await
            .map_err(|e| format!("复制文件失败: {}", e))?;
    }

    Ok(())
}

/// 删除指定聊天音效文件
/// url 参数可以是完整路径或纯文件名，统一从 chat_sounds_dir 中删除
#[tauri::command]
pub fn delete_chat_sound(url: String) -> Result<Vec<ChatSoundItemInfo>, String> {
    let base = chat_sounds_dir();

    // 从路径中提取文件名，兼容完整路径和纯文件名
    let filename = std::path::Path::new(&url)
        .file_name()
        .ok_or_else(|| format!("无效的文件路径: {}", url))?
        .to_string_lossy()
        .into_owned();

    let file_path = base.join(&filename);
    validate_path_in_base(&file_path, &base)?;

    if !file_path.exists() {
        return Err(format!("聊天音效文件不存在: {}", filename));
    }

    fs::remove_file(&file_path).map_err(|e| format!("删除聊天音效失败: {}", e))?;

    get_chat_sound_list()
}
