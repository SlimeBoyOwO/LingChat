use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

use crate::AppState;
use crate::ai_service::game_system::scene_store::{LightingParams, Scene, SceneStore};
use crate::api::data_dir;
use crate::utils::path::{validate_directory_name, validate_path_in_base};

// ========== Response types ==========

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SceneInfo {
    pub id: String,
    pub scene_name: String,
    pub scene_description: String,
    pub background: Option<String>,
    /// 场景所属子分类（背景子文件夹名；根目录为“根目录”）
    pub category: String,
    pub lighting: Option<LightingParams>,
    pub created_at: String,
    pub updated_at: String,
    /// 来源："game" 或提供该场景背景图的插件 id（插件场景写入 scenes.json，带 plugin_id 标签）。
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

// ========== Request types ==========

#[derive(Debug, Deserialize)]
pub struct CreateSceneRequest {
    pub scene_name: String,
    pub scene_description: String,
    pub background: String,
    #[serde(default)]
    pub lighting: Option<LightingParams>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSceneRequest {
    pub id: String,
    pub scene_name: String,
    pub scene_description: String,
    pub background: String,
    #[serde(default)]
    pub lighting: Option<LightingParams>,
}

// ========== Helpers ==========

/// 从任意路径中提取纯文件名。
fn to_background_filename(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let p = std::path::Path::new(raw);
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw.to_string())
}

fn relative_storage_path(path: &Path, base: &Path) -> Option<String> {
    let relative = path.strip_prefix(base).ok()?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(
        relative
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => Some(value.to_string_lossy()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn relative_key(raw: &str) -> String {
    raw.replace('\\', "/")
        .trim_start_matches("./")
        .to_lowercase()
}

fn category_from_storage_path(raw: &str) -> String {
    Path::new(raw)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "根目录".to_string())
}

#[derive(Default)]
struct BackgroundIndex {
    by_relative: HashMap<String, PathBuf>,
    by_name: HashMap<String, Vec<PathBuf>>,
}

impl BackgroundIndex {
    fn build(base: &Path, files: &[PathBuf]) -> Self {
        let mut index = Self::default();
        for path in files {
            let Some(relative) = relative_storage_path(path, base) else {
                continue;
            };
            index
                .by_relative
                .insert(relative_key(&relative), path.clone());
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                index
                    .by_name
                    .entry(name.to_lowercase())
                    .or_default()
                    .push(path.clone());
            }
        }
        index
    }

    fn resolve(&self, raw: &str, category: &str) -> Result<Option<PathBuf>, String> {
        if raw.trim().is_empty() {
            return Ok(None);
        }

        let direct = Path::new(raw);
        if direct.is_absolute() && direct.exists() {
            return Ok(Some(direct.to_path_buf()));
        }

        let normalized = relative_key(raw);
        let has_relative_parent = raw.contains('/') || raw.contains('\\');
        if has_relative_parent {
            if let Some(path) = self.by_relative.get(&normalized) {
                return Ok(Some(path.clone()));
            }
        }

        let file_name = to_background_filename(raw);
        if file_name.is_empty() {
            return Ok(None);
        }
        let category_key = if category.is_empty() || category == "根目录" {
            relative_key(&file_name)
        } else {
            relative_key(&format!("{category}/{file_name}"))
        };
        if let Some(path) = self.by_relative.get(&category_key) {
            return Ok(Some(path.clone()));
        }

        match self.by_name.get(&file_name.to_lowercase()) {
            None => Ok(None),
            Some(matches) if matches.len() == 1 => Ok(matches.first().cloned()),
            Some(_) => Err(format!(
                "背景文件名存在多个匹配，无法确定场景绑定路径: {file_name}"
            )),
        }
    }
}

fn model_to_info_with_background_index(scene: &Scene, index: &BackgroundIndex) -> SceneInfo {
    let background = match index.resolve(&scene.background, &scene.category) {
        Ok(Some(path)) => Some(path.to_string_lossy().into_owned()),
        Ok(None) => None,
        Err(error) => {
            tracing::warn!(scene_id = %scene.id, %error, "场景背景路径存在歧义");
            None
        },
    };
    SceneInfo {
        id: scene.id.clone(),
        scene_name: scene.name.clone(),
        scene_description: scene.description.clone(),
        background,
        category: scene.category.clone(),
        lighting: scene.lighting.clone(),
        created_at: scene.created_at.clone(),
        updated_at: scene.updated_at.clone(),
        source: scene
            .plugin_id
            .clone()
            .unwrap_or_else(|| "game".to_string()),
        plugin_id: scene.plugin_id.clone(),
    }
}

fn scan_backgrounds(base: &Path) -> (Vec<PathBuf>, BackgroundIndex) {
    let mut files = Vec::new();
    if base.exists() {
        super::background::collect_background_files_recursive_pub(base, &mut files);
    }
    let index = BackgroundIndex::build(base, &files);
    (files, index)
}

fn stored_background_from_input(
    raw: &str,
    bg_base: &Path,
    index: &BackgroundIndex,
) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Ok(String::new());
    }
    let path = index
        .resolve(raw, "根目录")?
        .ok_or_else(|| format!("找不到背景文件: {}", to_background_filename(raw)))?;
    validate_path_in_base(&path, bg_base)?;
    relative_storage_path(&path, bg_base).ok_or_else(|| "背景路径不在背景目录内".to_string())
}

fn migrate_scene_background(scene: &mut Scene, bg_base: &Path, index: &BackgroundIndex) -> bool {
    if scene.plugin_id.is_some() || scene.background.trim().is_empty() {
        return false;
    }
    let Ok(Some(path)) = index.resolve(&scene.background, &scene.category) else {
        return false;
    };
    let Some(relative) = relative_storage_path(&path, bg_base) else {
        return false;
    };
    let category = category_from_storage_path(&relative);
    let changed = scene.background != relative || scene.category != category;
    scene.background = relative;
    scene.category = category;
    changed
}

/// 将场景中保存的背景相对路径解析为完整路径。
pub(crate) fn normalize_background(raw: &str) -> String {
    let bg_base = super::backgrounds_dir();
    let (_, index) = scan_backgrounds(&bg_base);
    match index.resolve(raw, "根目录") {
        Ok(Some(path)) => path.to_string_lossy().into_owned(),
        _ => String::new(),
    }
}

pub(crate) fn model_to_info(scene: &Scene) -> SceneInfo {
    let bg_base = super::backgrounds_dir();
    let (_, index) = scan_backgrounds(&bg_base);
    model_to_info_with_background_index(scene, &index)
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_index_uses_category_to_disambiguate_same_name() {
        let base = PathBuf::from("backgrounds");
        let root = base.join("room.png");
        let sub = base.join("古风").join("room.png");
        let index = BackgroundIndex::build(&base, &[root.clone(), sub.clone()]);

        assert_eq!(index.resolve("room.png", "根目录").unwrap(), Some(root));
        assert_eq!(index.resolve("room.png", "古风").unwrap(), Some(sub));
    }

    #[test]
    fn background_index_resolves_stored_relative_path_exactly() {
        let base = PathBuf::from("backgrounds");
        let root = base.join("room.png");
        let sub = base.join("古风").join("room.png");
        let index = BackgroundIndex::build(&base, &[root, sub.clone()]);

        assert_eq!(index.resolve("古风/room.png", "根目录").unwrap(), Some(sub));
    }

    #[test]
    fn background_index_rejects_ambiguous_legacy_filename() {
        let base = PathBuf::from("backgrounds");
        let first = base.join("古风").join("room.png");
        let second = base.join("现代").join("room.png");
        let index = BackgroundIndex::build(&base, &[first, second]);

        assert!(index.resolve("room.png", "根目录").is_err());
    }
}

// ========== Tauri commands ==========

#[tauri::command]
pub async fn list_scenes(_app: AppHandle) -> Result<Vec<SceneInfo>, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    // 一次扫描全部背景，后续迁移、去重和响应组装都复用同一份路径索引。
    let bg_dir = super::backgrounds_dir();
    let (background_files, background_index) = scan_backgrounds(&bg_dir);

    // 旧数据迁移：把纯文件名或旧绝对路径升级为 backgrounds 下的相对路径。
    // 若裸文件名存在多个候选且 category 也无法唯一定位，则保留原值，绝不猜测。
    let mut dirty = false;
    for s in scenes.iter_mut() {
        if migrate_scene_background(s, &bg_dir, &background_index) {
            dirty = true;
        }
    }
    if dirty {
        store
            .save_all(&scenes)
            .map_err(|e| format!("迁移场景分类失败: {}", e))?;
    }

    let existing_bgs: HashSet<String> = scenes
        .iter()
        .filter(|scene| scene.plugin_id.is_none())
        .map(|scene| relative_key(&scene.background))
        .filter(|background| !background.is_empty())
        .collect();

    let allowed = ["png", "jpg", "jpeg", "webp", "bmp", "svg", "tif", "gif"];
    let mut added = false;

    if bg_dir.exists() {
        for path in background_files {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !allowed.contains(&ext.as_str()) {
                continue;
            }
            let Some(relative) = relative_storage_path(&path, &bg_dir) else {
                continue;
            };
            if existing_bgs.contains(&relative_key(&relative)) {
                continue;
            }
            // 自动注册时直接持久化相对路径，使不同分类下的同名文件保持独立身份。
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let category = category_from_storage_path(&relative);
            let now = now_iso();
            scenes.push(Scene {
                id: Uuid::new_v4().to_string(),
                name,
                description: String::new(),
                background: relative,
                lighting: None,
                category,
                created_at: now.clone(),
                updated_at: now,
                plugin_id: None,
            });
            added = true;
        }
    }

    if added {
        store
            .save_all(&scenes)
            .map_err(|e| format!("保存场景失败: {}", e))?;
    }

    Ok(scenes
        .iter()
        .map(|scene| model_to_info_with_background_index(scene, &background_index))
        .collect())
}

#[tauri::command]
pub async fn create_scene(_app: AppHandle, req: CreateSceneRequest) -> Result<SceneInfo, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    let bg_base = super::backgrounds_dir();
    let (_, background_index) = scan_backgrounds(&bg_base);
    let background = stored_background_from_input(&req.background, &bg_base, &background_index)?;
    let category = category_from_storage_path(&background);
    let now = now_iso();
    let scene = Scene {
        id: Uuid::new_v4().to_string(),
        name: req.scene_name,
        description: req.scene_description,
        background,
        category,
        lighting: req.lighting,
        created_at: now.clone(),
        updated_at: now,
        plugin_id: None,
    };
    let info = model_to_info(&scene);
    scenes.push(scene);
    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {}", e))?;
    Ok(info)
}

#[tauri::command]
pub async fn update_scene(_app: AppHandle, req: UpdateSceneRequest) -> Result<SceneInfo, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    let idx = scenes
        .iter()
        .position(|s| s.id == req.id)
        .ok_or_else(|| format!("场景 {} 不存在", req.id))?;

    let bg_base = super::backgrounds_dir();
    let (_, background_index) = scan_backgrounds(&bg_base);
    let background = stored_background_from_input(&req.background, &bg_base, &background_index)?;
    scenes[idx].name = req.scene_name;
    scenes[idx].description = req.scene_description;
    scenes[idx].category = category_from_storage_path(&background);
    scenes[idx].background = background;
    scenes[idx].lighting = req.lighting;
    scenes[idx].updated_at = now_iso();

    let info = model_to_info(&scenes[idx]);
    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {}", e))?;
    Ok(info)
}

#[tauri::command]
pub async fn delete_scene(app: AppHandle, id: String) -> Result<(), String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    let before = scenes.len();
    scenes.retain(|s| s.id != id);
    if scenes.len() == before {
        return Err(format!("场景 {} 不存在", id));
    }

    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {}", e))?;

    // 若删除的是当前选中场景，清除引用
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    if gs.current_scene_id.as_deref() == Some(&id) {
        gs.current_scene_id = None;
    }

    Ok(())
}

/// 一键清除「空白场景」：遍历所有场景，删除那些背景图片已不存在（解析为空）/缺图的场景。
/// 返回被删除的场景数量。用于清理因背景文件被改名/删除/移动而残留的空白场景。
#[tauri::command]
pub async fn clear_empty_scenes(app: AppHandle) -> Result<usize, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    let before = scenes.len();
    let bg_base = super::backgrounds_dir();
    let (_, background_index) = scan_backgrounds(&bg_base);
    // 保留：① 空背景/未设置背景的合法场景；② 能够精确解析（含正面命中）的场景；
    // ③ 同名文件存在多个候选（歧义）的场景（resolve 返回 Err，不应误删）。
    // 仅删除：声明了背景但物理文件已不存在（resolve 返回 Ok(None)）的场景。
    scenes.retain(|scene| {
        if scene.background.trim().is_empty() {
            return true;
        }
        match background_index.resolve(&scene.background, &scene.category) {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(_) => true,
        }
    });
    let removed = before - scenes.len();

    if removed > 0 {
        store
            .save_all(&scenes)
            .map_err(|e| format!("保存场景失败: {}", e))?;

        // 若删除的场景是当前选中场景，清除引用
        let state = app.state::<AppState>();
        let service = state.ai_service.lock().await;
        let mut gs = service.game_status.lock().await;
        if let Some(ref scene_id) = gs.current_scene_id {
            let still_exists = scenes.iter().any(|s| &s.id == scene_id);
            if !still_exists {
                gs.current_scene_id = None;
            }
        }
    }

    Ok(removed)
}

/// 删除/迁移背景分类后同步 scenes.json。
///
/// - `move_to_root`：把引用该分类下背景的相对路径改为纯文件名，category 设为「根目录」。
/// - `delete_all`：删除后若背景还能解析到其他候选则更新为新的相对路径；
///   若彻底丢失则清空背景（保留空背景场景）；歧义时保留场景并把 category 回「根目录」。
pub(crate) fn sync_scenes_after_background_category_change(
    bg_base: &Path,
    category: &str,
    mode: &str,
) -> Result<(), String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("读取场景列表失败: {}", e))?;
    let mut dirty = false;

    let prefix = format!("{category}/");

    if mode == "move_to_root" {
        for scene in scenes.iter_mut() {
            if scene.plugin_id.is_some() {
                continue;
            }
            if !scene.background.starts_with(&prefix) {
                continue;
            }
            scene.background = to_background_filename(&scene.background);
            scene.category = "根目录".to_string();
            scene.updated_at = now_iso();
            dirty = true;
        }
    } else if mode == "delete_all" {
        let (_, remaining_index) = scan_backgrounds(bg_base);
        for scene in scenes.iter_mut() {
            if scene.plugin_id.is_some() {
                continue;
            }
            if !scene.background.starts_with(&prefix) {
                continue;
            }
            match remaining_index.resolve(&scene.background, &scene.category) {
                Ok(Some(path)) => {
                    if let Some(relative) = relative_storage_path(&path, bg_base) {
                        let category = category_from_storage_path(&relative);
                        scene.background = relative;
                        scene.category = category;
                    }
                },
                Err(_) => {
                    // 歧义：保留场景与背景引用，但分类回「根目录」避免指向已删除分类。
                    scene.category = "根目录".to_string();
                },
                Ok(None) => {
                    // 背景文件已彻底丢失：保留空背景场景，但清空背景引用。
                    scene.background = String::new();
                    scene.category = "根目录".to_string();
                },
            }
            scene.updated_at = now_iso();
            dirty = true;
        }
    } else {
        return Err(format!("无效的分类删除模式: {mode}"));
    }

    if dirty {
        store
            .save_all(&scenes)
            .map_err(|e| format!("同步场景列表失败: {}", e))?;
    }
    Ok(())
}

/// 把某个场景的背景文件移动到目标子分类（子文件夹）。
/// category 传「根目录」则移动到 backgrounds 根目录；否则移动到 backgrounds/<category>/ 下。
/// 同时更新场景的 category 与背景相对路径。
#[tauri::command]
pub async fn move_scene_to_category(
    _app: AppHandle,
    id: String,
    category: String,
) -> Result<SceneInfo, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    let idx = scenes
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| format!("场景 {} 不存在", id))?;

    let target = {
        let t = category.trim();
        if t.is_empty() || t == "根目录" {
            "根目录".to_string()
        } else {
            let category = validate_directory_name(t)?;
            if matches!(category.as_str(), "全部" | "插件") {
                return Err("不能移动到保留分类".to_string());
            }
            category
        }
    };

    let file_name = to_background_filename(&scenes[idx].background);
    if file_name.trim().is_empty() {
        return Err("该场景没有关联的背景，无法移动".to_string());
    }

    let bg_base = super::backgrounds_dir();
    let (_, background_index) = scan_backgrounds(&bg_base);
    let src = background_index
        .resolve(&scenes[idx].background, &scenes[idx].category)?
        .ok_or_else(|| format!("找不到背景文件: {file_name}"))?;
    validate_path_in_base(&src, &bg_base)?;
    let dest_dir = if target == "根目录" {
        bg_base.clone()
    } else {
        bg_base.join(&target)
    };
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("创建分类目录失败: {e}"))?;
    validate_path_in_base(&dest_dir, &bg_base)?;
    let dest = dest_dir.join(&file_name);

    if src != dest {
        if dest.exists() {
            return Err(format!("目标分类已存在同名背景文件: {file_name}"));
        }
        std::fs::rename(&src, &dest).map_err(|e| format!("移动背景文件失败: {e}"))?;
    }

    // 多个场景可能共享同一个背景文件。文件物理移动后，需要同步更新所有
    // 解析到该文件（src）的本地场景，避免其余场景的背景路径/分类失效。
    let moved_relative = relative_storage_path(&dest, &bg_base)
        .ok_or_else(|| "移动后的背景路径不在背景目录内".to_string())?;
    let now = now_iso();
    for scene in scenes.iter_mut() {
        if scene.plugin_id.is_some() {
            continue;
        }
        let Ok(Some(path)) = background_index.resolve(&scene.background, &scene.category) else {
            continue;
        };
        if path == src {
            scene.background = moved_relative.clone();
            scene.category = target.clone();
            scene.updated_at = now.clone();
        }
    }
    let info = model_to_info(&scenes[idx]);
    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {}", e))?;
    Ok(info)
}

#[tauri::command]
pub async fn select_scene(app: AppHandle, scene_id: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    gs.current_scene_id = scene_id.clone();

    // 持久化到 store，便于下次启动恢复
    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        let val = match &scene_id {
            Some(id) => serde_json::Value::String(id.clone()),
            None => serde_json::Value::Null,
        };
        store.set(crate::config::session::LAST_SCENE_ID.to_string(), val);
        let _ = store.save();
    }

    Ok(())
}

#[tauri::command]
pub async fn set_scene_awareness(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    gs.scene_awareness_enabled = enabled;

    // 持久化到 store
    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        store.set(
            crate::config::session::SCENE_AWARENESS_ENABLED.to_string(),
            serde_json::Value::Bool(enabled),
        );
        let _ = store.save();
    }

    Ok(())
}
