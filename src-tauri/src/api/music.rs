use std::fs;

use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

use crate::utils::path::validate_path_in_base;

use super::music_dir;

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MusicItemInfo {
    pub name: String,
    pub url: String,
    pub time: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UploadMusicResult {
    /// 实际落盘的文件名（含 magic 决定的正确扩展名）
    pub actual_name: String,
    /// 用户原始文件名
    pub original_name: String,
    /// infer 识别的格式：mp3 / wav / flac / ogg / m4a
    pub detected_kind: String,
    /// 是否发生自动修正（原扩展名 != magic 决定的扩展名）
    pub was_corrected: bool,
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub fn get_music_list() -> Result<Vec<MusicItemInfo>, String> {
    let music_dir = music_dir();

    if !music_dir.exists() {
        return Ok(Vec::new());
    }

    let allowed_extensions = ["mp3", "wav", "flac", "webm", "weba", "ogg", "m4a", "oga"];

    let mut items: Vec<MusicItemInfo> = Vec::new();

    let entries = fs::read_dir(&music_dir).map_err(|e| format!("读取音乐目录失败: {}", e))?;

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

        items.push(MusicItemInfo { name, url, time });
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
pub fn get_music_file(filename: String) -> Result<String, String> {
    let base = music_dir();
    let resolved = base.join(&filename);

    validate_path_in_base(&resolved, &base)?;

    if !resolved.exists() {
        return Err(format!("音乐文件不存在: {}", filename));
    }

    let canon = resolved
        .canonicalize()
        .map_err(|e| format!("路径解析失败: {}", e))?;
    Ok(canon.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn upload_music(
    app: tauri::AppHandle,
    path: String,
    file_name: String,
) -> Result<UploadMusicResult, String> {
    // 1. 防路径遍历
    let original_name = std::path::Path::new(&file_name)
        .file_name()
        .ok_or_else(|| format!("无效的文件名: {}", file_name))?
        .to_string_lossy()
        .into_owned();

    // 2. magic sniff 决定真实格式
    let src_path = std::path::PathBuf::from(&path);
    let detected = infer::get_from_path(&src_path)
        .map_err(|e| format!("读取文件头失败: {e}"))?;
    let (kind, correct_ext) = match detected {
        Some(k) if k.matcher_type() == infer::MatcherType::Audio => match k.mime_type() {
            "audio/mpeg" => ("mp3",  "mp3"),
            "audio/wav"  => ("wav",  "wav"),
            "audio/flac" => ("flac", "flac"),
            "audio/ogg"  => ("ogg",  "ogg"),
            "audio/mp4"  => ("m4a",  "m4a"),
            _ => return Err("MUSIC_INVALID_FORMAT".into()),
        },
        _ => return Err("MUSIC_INVALID_FORMAT".into()),
    };

    // 3. 用 magic 决定的扩展名替换原扩展名
    let stem = std::path::Path::new(&original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    let corrected_name = format!("{stem}.{correct_ext}");

    // 4. 确保目标目录存在
    let music_dir = music_dir();
    if !music_dir.exists() {
        tokio::fs::create_dir_all(&music_dir)
            .await
            .map_err(|e| format!("创建音乐目录失败: {}", e))?;
    }

    // 5. 冲突时按 _2/_3/... 后缀
    let mut final_name = corrected_name;
    let mut counter = 2u32;
    while music_dir.join(&final_name).exists() {
        if counter > 999 {
            final_name = format!("{stem}_{}{}", chrono::Utc::now().timestamp_millis(), correct_ext);
            break;
        }
        final_name = format!("{stem}_{counter}.{correct_ext}");
        counter += 1;
    }

    let was_corrected = original_name != final_name;
    let file_path = music_dir.join(&final_name);

    // 6. 复制（桌面 vs Android SAF）
    if path.starts_with("content://") {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        app.android_fs_async()
            .copy(&FsUri::from_uri(&path), &FsUri::from_path(&file_path))
            .await
            .map_err(|e| format!("SAF 复制音乐失败: {}", e))?;
    } else {
        tokio::fs::copy(&src_path, &file_path)
            .await
            .map_err(|e| format!("复制文件失败: {}", e))?;
    }

    Ok(UploadMusicResult {
        actual_name: final_name,
        original_name,
        detected_kind: kind.to_string(),
        was_corrected,
    })
}

/// 删除指定音乐文件
/// url 参数可以是完整路径或纯文件名，统一从 music_dir 中删除
#[tauri::command]
pub fn delete_music(url: String) -> Result<Vec<MusicItemInfo>, String> {
    let base = music_dir();

    // 从路径中提取文件名，兼容完整路径和纯文件名
    let filename = std::path::Path::new(&url)
        .file_name()
        .ok_or_else(|| format!("无效的文件路径: {}", url))?
        .to_string_lossy()
        .into_owned();

    let file_path = base.join(&filename);
    validate_path_in_base(&file_path, &base)?;

    if !file_path.exists() {
        return Err(format!("音乐文件不存在: {}", filename));
    }

    fs::remove_file(&file_path).map_err(|e| format!("删除音乐文件失败: {}", e))?;

    get_music_list()
}

// ========== 会话状态持久化 ==========

/// 持久化背景音乐播放状态到 settings.json，下次启动时自动恢复。
#[tauri::command]
pub fn save_bgm_state(
    app: tauri::AppHandle,
    track: String,
    paused: bool,
    mode: String,
) -> Result<(), String> {
    let store = app
        .store(crate::config::STORE_FILE)
        .map_err(|e| format!("打开存储失败: {e}"))?;
    store.set(
        crate::config::session::LAST_BGM_TRACK.to_string(),
        serde_json::Value::String(track),
    );
    store.set(
        crate::config::session::LAST_BGM_PAUSED.to_string(),
        serde_json::Value::Bool(paused),
    );
    store.set(
        crate::config::session::LAST_BGM_MODE.to_string(),
        serde_json::Value::String(mode),
    );
    store.save().map_err(|e| format!("保存失败: {e}"))
}
