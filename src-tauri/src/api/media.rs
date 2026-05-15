use std::fs;

use super::game_data_dir;

/// 获取脚本媒体文件路径（音乐/音效/图片）
/// media_type: "music" | "sound" | "pic"
#[tauri::command]
pub fn get_script_media_file(
    file_path: String,
    media_type: String,
) -> Result<String, String> {
    let scripts_dir = game_data_dir().join("scripts");

    let dir_candidates: &[&str] = match media_type.as_str() {
        "music" => &["Musics", "BGMs", "Music", "BGM"],
        "sound" => &["Sounds", "SoundEffects", "Sound", "SoundEffect"],
        "pic" => &["Pics", "Pictures", "Pic", "Picture"],
        _ => return Err(format!("不支持的媒体类型: {}", media_type)),
    };

    if let Ok(entries) = fs::read_dir(&scripts_dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let script_path = entry.path();

            // 检查脚本根目录 assets 子目录
            for dir_name in dir_candidates {
                let p = script_path.join("assets").join(dir_name).join(&file_path);
                if p.exists() {
                    let canon = p
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
                            for dir_name in dir_candidates {
                                let p = adv_entry
                                    .path()
                                    .join("Assets")
                                    .join(dir_name)
                                    .join(&file_path);
                                if p.exists() {
                                    let canon = p
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

    Err(format!("未找到脚本媒体文件: {} (type={})", file_path, media_type))
}
