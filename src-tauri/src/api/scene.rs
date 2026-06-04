use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

use crate::ai_service::game_system::scene_store::{LightingParams, Scene, SceneStore};
use crate::api::data_dir;
use crate::AppState;

// ========== Response types ==========

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SceneInfo {
    pub id: String,
    pub scene_name: String,
    pub scene_description: String,
    pub background: Option<String>,
    pub lighting: Option<LightingParams>,
    pub created_at: String,
    pub updated_at: String,
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

/// 从任意路径中提取纯文件名（用于存储到 scenes.json）
fn to_background_filename(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let p = std::path::Path::new(raw);
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw.to_string())
}

/// 将存储的文件名解析为完整路径（用于返回给前端显示）
fn to_background_fullpath(filename: &str) -> String {
    if filename.is_empty() {
        return String::new();
    }
    // 如果已经是完整路径（旧数据兼容），先提取文件名再拼接
    let name = to_background_filename(filename);
    if name.is_empty() {
        return String::new();
    }
    super::backgrounds_dir().join(&name).to_string_lossy().into_owned()
}

/// 【已废弃】改为仅提取文件名；保留此函数供外部用（game.rs），行为改为解析为完整路径。
pub(crate) fn normalize_background(raw: &str) -> String {
    to_background_fullpath(raw)
}

fn model_to_info(s: &Scene) -> SceneInfo {
    let bg = normalize_background(&s.background);
    SceneInfo {
        id: s.id.clone(),
        scene_name: s.name.clone(),
        scene_description: s.description.clone(),
        background: if bg.is_empty() { None } else { Some(bg) },
        lighting: s.lighting.clone(),
        created_at: s.created_at.clone(),
        updated_at: s.updated_at.clone(),
    }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ========== Tauri commands ==========

#[tauri::command]
pub async fn list_scenes(_app: AppHandle) -> Result<Vec<SceneInfo>, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store.load_all().map_err(|e| format!("加载场景列表失败: {}", e))?;

    // 自动将背景目录中未被注册为场景的图片注册为场景
    let bg_dir = super::backgrounds_dir();
    let existing_bgs: HashSet<String> = scenes
        .iter()
        .map(|s| to_background_filename(&s.background))
        .filter(|b| !b.is_empty())
        .collect();

    let allowed = ["png", "jpg", "jpeg", "webp", "bmp", "svg", "tif", "gif"];
    let mut added = false;

    if bg_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&bg_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if !allowed.contains(&ext.as_str()) {
                    continue;
                }
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if existing_bgs.contains(&file_name) {
                    continue;
                }
                // 自动注册：名称为文件名（不含后缀），仅存文件名
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let now = now_iso();
                scenes.push(Scene {
                    id: Uuid::new_v4().to_string(),
                    name,
                    description: String::new(),
                    background: file_name.clone(),
                    lighting: None,
                    created_at: now.clone(),
                    updated_at: now,
                });
                added = true;
            }
        }
    }

    if added {
        store.save_all(&scenes).map_err(|e| format!("保存场景失败: {}", e))?;
    }

    Ok(scenes.iter().map(|s| model_to_info(s)).collect())
}

#[tauri::command]
pub async fn create_scene(_app: AppHandle, req: CreateSceneRequest) -> Result<SceneInfo, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store.load_all().map_err(|e| format!("加载场景列表失败: {}", e))?;

    let now = now_iso();
    let scene = Scene {
        id: Uuid::new_v4().to_string(),
        name: req.scene_name,
        description: req.scene_description,
        background: to_background_filename(&req.background),
        lighting: req.lighting,
        created_at: now.clone(),
        updated_at: now,
    };
    let info = model_to_info(&scene);
    scenes.push(scene);
    store.save_all(&scenes).map_err(|e| format!("保存场景失败: {}", e))?;
    Ok(info)
}

#[tauri::command]
pub async fn update_scene(_app: AppHandle, req: UpdateSceneRequest) -> Result<SceneInfo, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store.load_all().map_err(|e| format!("加载场景列表失败: {}", e))?;

    let idx = scenes
        .iter()
        .position(|s| s.id == req.id)
        .ok_or_else(|| format!("场景 {} 不存在", req.id))?;

    scenes[idx].name = req.scene_name;
    scenes[idx].description = req.scene_description;
    scenes[idx].background = to_background_filename(&req.background);
    scenes[idx].lighting = req.lighting;
    scenes[idx].updated_at = now_iso();

    let info = model_to_info(&scenes[idx]);
    store.save_all(&scenes).map_err(|e| format!("保存场景失败: {}", e))?;
    Ok(info)
}

#[tauri::command]
pub async fn delete_scene(app: AppHandle, id: String) -> Result<(), String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store.load_all().map_err(|e| format!("加载场景列表失败: {}", e))?;

    let before = scenes.len();
    scenes.retain(|s| s.id != id);
    if scenes.len() == before {
        return Err(format!("场景 {} 不存在", id));
    }

    store.save_all(&scenes).map_err(|e| format!("保存场景失败: {}", e))?;

    // 若删除的是当前选中场景，清除引用
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    if gs.current_scene_id.as_deref() == Some(&id) {
        gs.current_scene_id = None;
    }

    Ok(())
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
        store.set(crate::config::keys::LAST_SCENE_ID.to_string(), val);
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
            crate::config::keys::SCENE_AWARENESS_ENABLED.to_string(),
            serde_json::Value::Bool(enabled),
        );
        let _ = store.save();
    }

    Ok(())
}
