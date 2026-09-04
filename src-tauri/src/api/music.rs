use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::plugins::ResourceKind;
use crate::utils::path::{move_directory_files_to, validate_directory_name, validate_path_in_base};
use crate::utils::system::open_folder;

use super::{default_source, mtime_secs, music_dir};

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MusicItemInfo {
    pub name: String,
    pub url: String,
    pub time: String,
    /// 音乐所属子分类（子文件夹名；根目录为"根目录"）
    pub category: String,
    /// 来源："game" 或提供该音乐的插件 id。
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UploadMusicResult {
    /// 实际落盘的文件名（含 magic 决定的正确扩展名）
    pub actual_name: String,
    /// 用户原始文件名
    pub original_name: String,
    /// infer 识别的格式：mp3 / wav / flac / ogg
    pub detected_kind: String,
    /// 是否发生自动修正（原扩展名 != magic 决定的扩展名）
    pub was_corrected: bool,
}

// ========== Tauri 命令 ==========

// ========== 递归扫描音乐目录（含子文件夹，即子分类） ==========

/// 递归收集音乐文件，并记录每个文件所属的子文件夹名（category）。
fn collect_music_recursive(
    base: &Path,
    category: &str,
    out: &mut Vec<(std::path::PathBuf, String)>,
) {
    if !base.exists() {
        return;
    }
    let allowed_extensions = ["mp3", "wav", "flac", "webm", "weba", "ogg", "oga"];
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_file() {
                let is_music = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| allowed_extensions.contains(&e.to_lowercase().as_str()))
                    .unwrap_or(false);
                if is_music {
                    out.push((path, category.to_string()));
                }
            } else if file_type.is_dir() {
                let sub_cat = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| category.to_string());
                collect_music_recursive(&path, &sub_cat, out);
            }
        }
    }
}

#[tauri::command]
pub async fn get_music_list(app: AppHandle) -> Result<Vec<MusicItemInfo>, String> {
    let music_dir = music_dir();

    let allowed_extensions = ["mp3", "wav", "flac", "webm", "weba", "ogg", "oga"];

    let mut items: Vec<MusicItemInfo> = Vec::new();

    // 递归扫描音乐目录（含子文件夹/子分类），并记录每个文件所属的分类
    let mut collected: Vec<(std::path::PathBuf, String)> = Vec::new();
    collect_music_recursive(&music_dir, "根目录", &mut collected);

    for (path, category) in collected {
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

        items.push(MusicItemInfo {
            name,
            url,
            time,
            category,
            source: "game".to_string(),
            plugin_id: None,
        });
    }
    // 合并插件背景音乐
    let plugin_entries = app
        .state::<crate::AppState>()
        .data()
        .plugin_manager
        .visible_file_entries(ResourceKind::Musics)
        .await;
    for e in plugin_entries {
        items.push(MusicItemInfo {
            name: e.name,
            url: e.path.to_string_lossy().into_owned(),
            time: mtime_secs(&e.path),
            category: "插件".to_string(),
            source: e.plugin_id.clone(),
            plugin_id: Some(e.plugin_id),
        });
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

/// 列出所有音乐子分类（去重），供前端选项卡使用。
#[tauri::command]
pub fn list_music_categories() -> Result<Vec<String>, String> {
    let music_dir = music_dir();
    let mut cats = std::collections::BTreeSet::new();
    // 插件背景音乐是虚拟分类（不映射到 music 下的子文件夹），始终可选
    cats.insert("插件".to_string());
    if music_dir.exists() {
        fn walk(base: &Path, cats: &mut std::collections::BTreeSet<String>) {
            if let Ok(entries) = fs::read_dir(base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Ok(file_type) = entry.file_type() else {
                        continue;
                    };
                    if file_type.is_symlink() {
                        continue;
                    }
                    if file_type.is_dir() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            cats.insert(name.to_string());
                        }
                        walk(&path, cats);
                    }
                }
            }
        }
        walk(&music_dir, &mut cats);
    }
    Ok(cats.into_iter().collect())
}

/// 新建一个音乐子分类（子文件夹）。
#[tauri::command]
pub fn create_music_category(name: String) -> Result<(), String> {
    let name = validate_directory_name(&name)?;
    if matches!(name.as_str(), "根目录" | "全部" | "插件") {
        return Err("不能使用保留分类名".into());
    }
    let base = music_dir();
    fs::create_dir_all(&base).map_err(|e| format!("创建音乐目录失败: {}", e))?;
    let dir = base.join(name);
    fs::create_dir_all(&dir).map_err(|e| format!("创建分类目录失败: {}", e))?;
    validate_path_in_base(&dir, &base)
}

fn resolve_music_path(base: &Path, url: &str) -> std::path::PathBuf {
    let requested = std::path::PathBuf::from(url);
    if requested.is_absolute() {
        requested
    } else {
        base.join(requested)
    }
}

/// 删除一个音乐子分类：mode = "move" 把其下音乐移到根目录；"delete" 连同音乐一起删除。返回受影响数量。
#[tauri::command]
pub fn delete_music_category(name: String, mode: String) -> Result<usize, String> {
    let name = validate_directory_name(&name)?;
    if matches!(name.as_str(), "根目录" | "全部" | "插件") {
        return Err("不能删除保留分类".into());
    }
    let base = music_dir();
    let dir = base.join(&name);
    if !dir.is_dir() {
        return Ok(0);
    }
    validate_path_in_base(&dir, &base)?;

    match mode.as_str() {
        "move" => move_directory_files_to(&dir, &base),
        "delete" => {
            let mut files = Vec::new();
            collect_music_recursive(&dir, "", &mut files);
            let count = files.len();
            fs::remove_dir_all(&dir).map_err(|e| format!("删除分类「{}」失败: {}", name, e))?;
            Ok(count)
        },
        _ => Err(format!("无效的分类删除模式: {mode}")),
    }
}

/// 打开音乐所在文件夹。
#[tauri::command]
pub fn open_music_folder() -> Result<(), String> {
    let dir = music_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建音乐目录失败: {}", e))?;
    }
    open_folder(&dir.to_string_lossy())
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
    category: Option<String>,
) -> Result<UploadMusicResult, String> {
    // Android SAF：先把 content URI 复制到本地 cache，magic sniff 和后续复制都用本地路径。
    let src =
        crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &path).await?;

    let result: Result<UploadMusicResult, String> = async {
        // 1. 优先用前端传来的 file_name（桌面端是真实文件名；Android 上是
        //    dialog 给的 URI 末段，已经前端 decode 过）。回退到 src 提供的
        //    display_name（也是 SAF 末段）。
        let original_name = if !file_name.is_empty() {
            std::path::Path::new(&file_name)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or(src.display_name.clone())
        } else {
            src.display_name.clone()
        };
        if original_name.is_empty() {
            return Err(format!("无效的文件名: {}", file_name));
        }

        // 2. magic sniff 决定真实格式（src.path 已是本地路径，desktop / SAF 都通）。
        //    注意：用 infer 返回的 extension() 做权威裁决 —— 它是 map.rs 里注册的
        //    字面值，与 match 函数一一对应，不会被 mime 别名（如 audio/x-flac vs
        //    audio/flac）干扰。
        //
        //    不接受 m4a：infer 把 brand=isom/mp42/dash 的 m4a 识别为 video/mp4，
        //    而 brand=M4A 的边缘子集不值得为一个视频污染风险留着。
        let detected =
            infer::get_from_path(&src.path).map_err(|e| format!("读取文件头失败: {e}"))?;
        let (kind, correct_ext) = match detected {
            Some(k) if k.matcher_type() == infer::MatcherType::Audio => match k.extension() {
                "mp3" => ("mp3", "mp3"),
                "wav" => ("wav", "wav"),
                "flac" => ("flac", "flac"),
                "ogg" => ("ogg", "ogg"),
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

        // 4. 确保目标目录存在：若指定分类，则写入对应子文件夹
        let music_dir = music_dir();
        let target_dir = match category.as_deref() {
            Some(cat) if !cat.trim().is_empty() => {
                let category = validate_directory_name(cat)?;
                if matches!(category.as_str(), "根目录" | "全部") {
                    if !music_dir.exists() {
                        tokio::fs::create_dir_all(&music_dir)
                            .await
                            .map_err(|e| format!("创建音乐目录失败: {}", e))?;
                    }
                    music_dir.clone()
                } else if category == "插件" {
                    return Err("不能上传到插件分类".to_string());
                } else {
                    let sub = music_dir.join(category);
                    if !sub.exists() {
                        tokio::fs::create_dir_all(&sub)
                            .await
                            .map_err(|e| format!("创建分类目录失败: {}", e))?;
                    }
                    validate_path_in_base(&sub, &music_dir)?;
                    sub
                }
            },
            _ => {
                if !music_dir.exists() {
                    tokio::fs::create_dir_all(&music_dir)
                        .await
                        .map_err(|e| format!("创建音乐目录失败: {}", e))?;
                }
                music_dir.clone()
            },
        };

        // 5. 冲突时按 _2/_3/... 后缀
        let mut final_name = corrected_name;
        let mut counter = 2u32;
        while target_dir.join(&final_name).exists() {
            if counter > 999 {
                final_name = format!(
                    "{stem}_{}{}",
                    chrono::Utc::now().timestamp_millis(),
                    correct_ext
                );
                break;
            }
            final_name = format!("{stem}_{counter}.{correct_ext}");
            counter += 1;
        }

        // 仅扩展名/名字实质变化才算"自动修正"；纯大小写差异（Song.MP3 → Song.mp3）不算。
        let was_corrected = !original_name.eq_ignore_ascii_case(&final_name);
        let file_path = target_dir.join(&final_name);

        // 6. 复制（src.path 是本地 cache，dest 也是本地路径，用 std::fs::copy）
        std::fs::copy(&src.path, &file_path).map_err(|e| format!("复制文件失败: {}", e))?;

        Ok(UploadMusicResult {
            actual_name: final_name,
            original_name,
            detected_kind: kind.to_string(),
            was_corrected,
        })
    }
    .await;

    if src.cleanup_after_import {
        let _ = tokio::fs::remove_file(&src.path).await;
    }
    result
}

/// 删除指定音乐文件
/// url 参数可以是 music_dir 内的完整路径、相对路径或根目录文件名。
#[tauri::command]
pub async fn delete_music(app: AppHandle, url: String) -> Result<Vec<MusicItemInfo>, String> {
    let base = music_dir();
    let file_path = resolve_music_path(&base, &url);
    validate_path_in_base(&file_path, &base)?;

    if !file_path.is_file() {
        return Err(format!("音乐文件不存在: {}", file_path.display()));
    }

    fs::remove_file(&file_path).map_err(|e| format!("删除音乐文件失败: {}", e))?;

    get_music_list(app).await
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::resolve_music_path;

    #[test]
    fn preserves_category_segments_in_relative_music_urls() {
        let base = Path::new("music");
        assert_eq!(
            resolve_music_path(base, "battle/boss.mp3"),
            base.join("battle").join("boss.mp3")
        );
        assert_eq!(resolve_music_path(base, "boss.mp3"), base.join("boss.mp3"));
    }
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
