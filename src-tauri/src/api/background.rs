use std::fs;
use std::io::Write;

use serde::{Deserialize, Serialize};

use super::{backgrounds_dir, validate_path_in_base};

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BackgroundItemInfo {
    pub title: String,
    pub url: String,
    pub time: String,
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub fn get_background_list() -> Result<Vec<BackgroundItemInfo>, String> {
    let bg_dir = backgrounds_dir();

    if !bg_dir.exists() {
        return Ok(Vec::new());
    }

    let allowed_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "svg", "tif", "gif"];

    let mut items: Vec<BackgroundItemInfo> = Vec::new();

    let entries = fs::read_dir(&bg_dir).map_err(|e| format!("读取背景目录失败: {}", e))?;

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

        let title = path
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

        items.push(BackgroundItemInfo { title, url, time });
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
pub fn get_background_file(filename: String) -> Result<String, String> {
    let base = backgrounds_dir();
    let resolved = base.join(&filename);

    validate_path_in_base(&resolved, &base)?;

    if !resolved.exists() {
        return Err(format!("背景文件不存在: {}", filename));
    }

    let canon = resolved
        .canonicalize()
        .map_err(|e| format!("路径解析失败: {}", e))?;
    Ok(canon.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn upload_background_image(
    file_name: String,
    file_data: Vec<u8>,
) -> Result<Vec<BackgroundItemInfo>, String> {
    let bg_dir = backgrounds_dir();
    if !bg_dir.exists() {
        fs::create_dir_all(&bg_dir).map_err(|e| format!("创建背景目录失败: {}", e))?;
    }

    // 安全检查：只保留文件名，防止路径遍历
    let safe_name = std::path::Path::new(&file_name)
        .file_name()
        .ok_or_else(|| format!("无效的文件名: {}", file_name))?
        .to_string_lossy()
        .into_owned();

    let file_path = bg_dir.join(&safe_name);
    let mut f = fs::File::create(&file_path).map_err(|e| format!("创建文件失败: {}", e))?;
    f.write_all(&file_data)
        .map_err(|e| format!("写入文件失败: {}", e))?;
    f.flush().map_err(|e| format!("刷新文件失败: {}", e))?;

    get_background_list()
}

#[tauri::command]
pub fn open_backgrounds_folder() -> Result<(), String> {
    let bg_dir = backgrounds_dir();
    if !bg_dir.exists() {
        fs::create_dir_all(&bg_dir).map_err(|e| format!("创建背景目录失败: {}", e))?;
    }

    let path_str = bg_dir.to_string_lossy().into_owned();
    open_folder(&path_str)
}

fn open_folder(path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    Ok(())
}
