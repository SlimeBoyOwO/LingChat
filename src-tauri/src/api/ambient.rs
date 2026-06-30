use std::fs;
use std::io::Write;

use serde::{Deserialize, Serialize};

use super::{ambient_dir, validate_path_in_base};

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
pub fn upload_ambient(file_name: String, file_data: Vec<u8>) -> Result<Vec<AmbientItemInfo>, String> {
    let ambient_dir = ambient_dir();
    if !ambient_dir.exists() {
        fs::create_dir_all(&ambient_dir).map_err(|e| format!("创建环境音目录失败: {}", e))?;
    }

    // 安全检查：只保留文件名，防止路径遍历
    let safe_name = std::path::Path::new(&file_name)
        .file_name()
        .ok_or_else(|| format!("无效的文件名: {}", file_name))?
        .to_string_lossy()
        .into_owned();

    let file_path = ambient_dir.join(&safe_name);
    let mut f = fs::File::create(&file_path).map_err(|e| format!("创建文件失败: {}", e))?;
    f.write_all(&file_data)
        .map_err(|e| format!("写入文件失败: {}", e))?;
    f.flush().map_err(|e| format!("刷新文件失败: {}", e))?;

    get_ambient_list()
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
