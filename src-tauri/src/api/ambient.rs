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

    let allowed_extensions = ["mp3", "wav", "flac", "webm", "weba", "ogg", "oga"];

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
pub async fn upload_ambient(
    app: tauri::AppHandle,
    path: String,
    file_name: String,
) -> Result<(), String> {
    // 安全检查：只保留文件名（basename），防止路径遍历。空 basename 直接拒。
    let safe_name = {
        let stem = std::path::Path::new(&file_name)
            .file_name()
            .ok_or_else(|| format!("无效的文件名: {}", file_name))?
            .to_string_lossy()
            .into_owned();
        if stem.is_empty() {
            return Err(format!("无效的文件名: {}", file_name));
        }
        stem
    };

    // Android SAF：先把 content URI 复制到本地 cache，magic sniff 在本地路径上做。
    let src =
        crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &path).await?;

    // magic sniff：与 music.rs 同源策略，用 infer extension() 而非 mime_type()
    // 做权威裁决（绕过 mime 别名问题：audio/x-flac 不匹配 audio/flac）。
    // 环境音不做扩展名修正，命中失败即拒。同时主动放弃 m4a。
    if let Some(k) = infer::get_from_path(&src.path).map_err(|e| format!("读取文件头失败: {e}"))?
    {
        let valid = k.matcher_type() == infer::MatcherType::Audio
            && matches!(k.extension(), "mp3" | "wav" | "flac" | "ogg");
        if !valid {
            if src.cleanup_after_import {
                let _ = tokio::fs::remove_file(&src.path).await;
            }
            return Err("MUSIC_INVALID_FORMAT".into());
        }
    } else {
        if src.cleanup_after_import {
            let _ = tokio::fs::remove_file(&src.path).await;
        }
        return Err("MUSIC_INVALID_FORMAT".into());
    }

    let ambient_dir = ambient_dir();
    if !ambient_dir.exists() {
        tokio::fs::create_dir_all(&ambient_dir)
            .await
            .map_err(|e| format!("创建环境音目录失败: {}", e))?;
    }

    let file_path = ambient_dir.join(&safe_name);

    // 把已通过 sniff 的本地 src 复制到 ambient_dir（Android SAF 路径走 src.path；
    // 桌面端 src.path == 原始 path，效果等价于直接复制源文件）。
    let copy_result =
        std::fs::copy(&src.path, &file_path).map_err(|e| format!("复制文件失败: {}", e));
    if src.cleanup_after_import {
        let _ = tokio::fs::remove_file(&src.path).await;
    }
    copy_result?;

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
pub fn save_ambient_state(app: tauri::AppHandle, tracks_json: String) -> Result<(), String> {
    let store = app
        .store(crate::config::STORE_FILE)
        .map_err(|e| format!("打开存储失败: {e}"))?;
    store.set(
        crate::config::session::LAST_AMBIENT_TRACKS.to_string(),
        serde_json::Value::String(tracks_json),
    );
    store.save().map_err(|e| format!("保存失败: {e}"))
}
