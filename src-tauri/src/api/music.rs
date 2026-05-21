use std::fs;
use std::io::Write;

use serde::{Deserialize, Serialize};

use super::{music_dir, validate_path_in_base};

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MusicItemInfo {
    pub name: String,
    pub url: String,
    pub time: String,
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
pub fn upload_music(file_name: String, file_data: Vec<u8>) -> Result<Vec<MusicItemInfo>, String> {
    let music_dir = music_dir();
    if !music_dir.exists() {
        fs::create_dir_all(&music_dir).map_err(|e| format!("创建音乐目录失败: {}", e))?;
    }

    // 安全检查：只保留文件名，防止路径遍历
    let safe_name = std::path::Path::new(&file_name)
        .file_name()
        .ok_or_else(|| format!("无效的文件名: {}", file_name))?
        .to_string_lossy()
        .into_owned();

    let file_path = music_dir.join(&safe_name);
    let mut f = fs::File::create(&file_path).map_err(|e| format!("创建文件失败: {}", e))?;
    f.write_all(&file_data)
        .map_err(|e| format!("写入文件失败: {}", e))?;
    f.flush().map_err(|e| format!("刷新文件失败: {}", e))?;

    get_music_list()
}
