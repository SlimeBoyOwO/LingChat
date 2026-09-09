use std::fs;
use std::io::Write;
use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::plugins::ResourceKind;
use crate::utils::path::{move_directory_files_to, validate_directory_name, validate_path_in_base};
use crate::utils::system::open_folder;
use serde::{Deserialize, Serialize};

use super::{backgrounds_dir, default_source, mtime_secs};

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BackgroundItemInfo {
    pub title: String,
    pub url: String,
    pub time: String,
    /// 背景所属子分类（子文件夹名；根目录为“根目录”）
    pub category: String,
    /// 来源："game" 或提供该背景图的插件 id。
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

// ========== 递归扫描背景目录（含子文件夹，即子分类） ==========

/// 递归收集背景文件（只返回路径列表，供 scene.rs 自动注册场景用）。
pub(crate) fn collect_background_files_recursive_pub(
    base: &Path,
    out: &mut Vec<std::path::PathBuf>,
) {
    if !base.exists() {
        return;
    }
    let allowed_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "svg", "tif", "gif"];
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            // 不跟随符号链接 / Junction，避免越界扫描与循环。
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_file() {
                let is_bg = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| allowed_extensions.contains(&e.to_lowercase().as_str()))
                    .unwrap_or(false);
                if is_bg {
                    out.push(path);
                }
            } else if file_type.is_dir() {
                collect_background_files_recursive_pub(&path, out);
            }
        }
    }
}

/// 递归收集背景文件，并记录每个文件所属的子文件夹名（category）。
/// 根目录下的文件 category 为“根目录”。
fn collect_backgrounds_recursive(
    base: &Path,
    category: &str,
    out: &mut Vec<(std::path::PathBuf, String)>,
) {
    if !base.exists() {
        return;
    }
    let allowed_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "svg", "tif", "gif"];
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
                let is_bg = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| allowed_extensions.contains(&e.to_lowercase().as_str()))
                    .unwrap_or(false);
                if is_bg {
                    out.push((path, category.to_string()));
                }
            } else if file_type.is_dir() {
                let sub_cat = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| category.to_string());
                collect_backgrounds_recursive(&path, &sub_cat, out);
            }
        }
    }
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub async fn get_background_list(app: AppHandle) -> Result<Vec<BackgroundItemInfo>, String> {
    let bg_dir = backgrounds_dir();

    let allowed_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "svg", "tif", "gif"];

    let mut items: Vec<BackgroundItemInfo> = Vec::new();

    // 递归扫描背景目录（含子文件夹/子分类），并记录每个文件所属的分类
    let mut collected: Vec<(std::path::PathBuf, String)> = Vec::new();
    collect_backgrounds_recursive(&bg_dir, "根目录", &mut collected);

    for (path, category) in collected {
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

        items.push(BackgroundItemInfo {
            title,
            url,
            time,
            category,
            source: "game".to_string(),
            plugin_id: None,
        });
    }
    // 合并插件背景图（已按游戏优先 + 插件间先注册者赢 + 隐藏过滤）
    let plugin_entries = app
        .state::<crate::AppState>()
        .data()
        .plugin_manager
        .visible_file_entries(ResourceKind::Backgrounds)
        .await;
    for e in plugin_entries {
        items.push(BackgroundItemInfo {
            title: e.name,
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

/// 列出所有背景子分类（去重），供前端选项卡使用。
#[tauri::command]
pub fn list_background_categories() -> Result<Vec<String>, String> {
    let bg_dir = backgrounds_dir();
    let mut cats = std::collections::BTreeSet::new();
    // 插件背景是虚拟分类（不映射到 backgrounds 下的子文件夹），始终可选
    cats.insert("插件".to_string());
    if bg_dir.exists() {
        // 递归扫描子文件夹名
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
        walk(&bg_dir, &mut cats);
    }
    Ok(cats.into_iter().collect())
}

/// 新建一个背景子分类（在 backgrounds/ 下创建子文件夹）。
#[tauri::command]
pub fn create_background_category(name: String) -> Result<(), String> {
    let safe = validate_directory_name(&name)?;
    if matches!(safe.as_str(), "根目录" | "全部" | "插件") {
        return Err("不能使用保留分类名".to_string());
    }
    let bg_dir = backgrounds_dir();
    if !bg_dir.exists() {
        fs::create_dir_all(&bg_dir).map_err(|e| format!("创建背景目录失败: {}", e))?;
    }
    let target = bg_dir.join(&safe);
    if target.exists() {
        return Err(format!("分类「{}」已存在", safe));
    }
    fs::create_dir(&target).map_err(|e| format!("创建分类「{}」失败: {}", safe, e))?;
    validate_path_in_base(&target, &bg_dir)?;
    Ok(())
}

/// 删除一个背景子分类。
/// mode = "move_to_root"：把该分类下所有背景文件移动到 backgrounds/ 根目录；
/// mode = "delete_all"：删除该分类目录及其所有背景文件。
/// 返回受影响（移动/删除）的背景数量。
#[tauri::command]
pub fn delete_background_category(name: String, mode: String) -> Result<usize, String> {
    let safe = validate_directory_name(&name)?;
    if matches!(safe.as_str(), "根目录" | "全部" | "插件") {
        return Err("不能删除保留分类".to_string());
    }
    let bg_dir = backgrounds_dir();
    let sub = bg_dir.join(&safe);
    if !sub.exists() || !sub.is_dir() {
        return Err(format!("分类「{}」不存在", safe));
    }
    validate_path_in_base(&sub, &bg_dir)?;

    // 收集子目录下所有背景文件
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_background_files_recursive_pub(&sub, &mut files);

    let affected = if mode == "move_to_root" {
        if !bg_dir.exists() {
            fs::create_dir_all(&bg_dir).map_err(|e| format!("创建背景目录失败: {}", e))?;
        }
        move_directory_files_to(&sub, &bg_dir)?
    } else if mode == "delete_all" {
        // 全部删除：删除整个子目录
        let affected = files.len();
        fs::remove_dir_all(&sub).map_err(|e| format!("删除分类「{}」失败: {}", safe, e))?;
        affected
    } else {
        return Err(format!("无效的分类删除模式: {mode}"));
    };

    // 同步引用该分类背景的场景，避免 scenes.json 与背景目录脱节。
    super::scene::sync_scenes_after_background_category_change(&bg_dir, &safe, &mode)?;

    Ok(affected)
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
pub async fn upload_background_image(
    app: AppHandle,
    file_name: String,
    file_data: Vec<u8>,
    category: Option<String>,
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

    // 目标目录：若指定了分类，则写入子文件夹（并确保其存在）；否则写入根目录
    let target_dir = match category.as_deref() {
        Some(cat) if !cat.trim().is_empty() => {
            let category = validate_directory_name(cat)?;
            if matches!(category.as_str(), "全部" | "根目录") {
                bg_dir.clone()
            } else if category == "插件" {
                return Err("不能上传到插件分类".to_string());
            } else {
                let sub = bg_dir.join(category);
                fs::create_dir_all(&sub).map_err(|e| format!("创建分类目录失败: {}", e))?;
                validate_path_in_base(&sub, &bg_dir)?;
                sub
            }
        },
        _ => bg_dir.clone(),
    };

    let file_path = target_dir.join(&safe_name);
    let mut f = fs::File::create(&file_path).map_err(|e| format!("创建文件失败: {}", e))?;
    f.write_all(&file_data)
        .map_err(|e| format!("写入文件失败: {}", e))?;
    f.flush().map_err(|e| format!("刷新文件失败: {}", e))?;

    get_background_list(app).await
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
