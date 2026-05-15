use std::fs;

use serde::{Deserialize, Serialize};

use super::{backgrounds_dir, game_data_dir, validate_path_in_base};

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

/// 获取脚本资源中的背景文件路径
/// 在 `game_data/scripts/` 下搜索匹配的 Backgrounds 目录
#[tauri::command]
pub fn get_script_background_file(file_path: String) -> Result<String, String> {
    let scripts_dir = game_data_dir().join("scripts");

    if let Ok(entries) = fs::read_dir(&scripts_dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let script_path = entry.path();
            for bg_dir_name in &["Backgrounds", "Background"] {
                let assets_bg = script_path
                    .join("assets")
                    .join(bg_dir_name)
                    .join(&file_path);
                if assets_bg.exists() {
                    let canon = assets_bg
                        .canonicalize()
                        .map_err(|e| format!("路径解析失败: {}", e))?;
                    return Ok(canon.to_string_lossy().into_owned());
                }
            }
            // 检查 character 子脚本
            let char_dir = script_path.join("character");
            if let Ok(char_entries) = fs::read_dir(&char_dir) {
                for char_entry in char_entries.flatten() {
                    if !char_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    if let Ok(adv_entries) = fs::read_dir(char_entry.path()) {
                        for adv_entry in adv_entries.flatten() {
                            if !adv_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                continue;
                            }
                            for bg_dir_name in &["Backgrounds", "Background"] {
                                let bg = adv_entry
                                    .path()
                                    .join("Assets")
                                    .join(bg_dir_name)
                                    .join(&file_path);
                                if bg.exists() {
                                    let canon = bg
                                        .canonicalize()
                                        .map_err(|e| format!("路径解析失败: {}", e))?;
                                    return Ok(canon.to_string_lossy().into_owned());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 回退到用户背景目录
    get_background_file(file_path)
}
