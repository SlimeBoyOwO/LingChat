use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

use crate::AppState;
use crate::ai_service::game_system::scene_store::{
    LightingParams, LocationType, Scene, SceneStore,
};
use crate::ai_service::screen_analyzer::{
    ImageDelivery, ImageIntent, ScreenAnalyzer, ScreenAnalyzerConfig, deliver_image,
};
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
    /// 场景所属子分类；None 表示背景位于根目录或未设置分类。
    pub category: Option<String>,
    pub lighting: Option<LightingParams>,
    pub created_at: String,
    pub updated_at: String,
    /// 来源："game" 或提供该场景背景图的插件 id（插件场景写入 scenes.json，带 plugin_id 标签）。
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    /// 室内外标注："indoor" / "outdoor" / "unknown"；缺省（旧场景）视为 unknown。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_type: Option<String>,
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

fn category_from_storage_path(raw: &str) -> Option<String> {
    Path::new(raw)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
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

    fn resolve(&self, raw: &str, category: Option<&str>) -> Result<Option<PathBuf>, String> {
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
        let category_key = match category.filter(|category| !category.is_empty()) {
            Some(category) => relative_key(&format!("{category}/{file_name}")),
            None => relative_key(&file_name),
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
    let background = match index.resolve(&scene.background, scene.category.as_deref()) {
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
        location_type: scene.location_type.map(|l| l.as_str().to_string()),
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
        .resolve(raw, None)?
        .ok_or_else(|| format!("找不到背景文件: {}", to_background_filename(raw)))?;
    validate_path_in_base(&path, bg_base)?;
    relative_storage_path(&path, bg_base).ok_or_else(|| "背景路径不在背景目录内".to_string())
}

fn migrate_scene_background(scene: &mut Scene, bg_base: &Path, index: &BackgroundIndex) -> bool {
    if scene.plugin_id.is_some() || scene.background.trim().is_empty() {
        return false;
    }
    let Ok(Some(path)) = index.resolve(&scene.background, scene.category.as_deref()) else {
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

pub(crate) fn model_to_info(scene: &Scene) -> SceneInfo {
    let bg_base = super::backgrounds_dir();
    let (_, index) = scan_backgrounds(&bg_base);
    model_to_info_with_background_index(scene, &index)
}

/// 场景当前应展示的背景图（绝对路径；解析不出则为 None）。
pub(crate) fn resolved_background_of(scene: &Scene) -> Option<String> {
    model_to_info(scene).background
}

/// 把 `GameStatus.background` 对齐到某个场景；场景不存在或没有背景时返回 None。
pub(crate) fn resolved_background_of_id(scene_id: &str) -> Option<String> {
    SceneStore::new(&data_dir())
        .find_by_id(scene_id)
        .ok()
        .flatten()
        .and_then(|scene| resolved_background_of(&scene))
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ========== Tauri commands ==========

#[tauri::command]
pub async fn list_scenes(app: AppHandle) -> Result<Vec<SceneInfo>, String> {
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
    let mut new_scene_ids: Vec<String> = Vec::new();

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
            let id = Uuid::new_v4().to_string();
            scenes.push(Scene {
                id: id.clone(),
                name,
                description: String::new(),
                background: relative,
                lighting: None,
                category,
                created_at: now.clone(),
                updated_at: now,
                plugin_id: None,
                location_type: None,
            });
            new_scene_ids.push(id);
            added = true;
        }
    }

    if added {
        store
            .save_all(&scenes)
            .map_err(|e| format!("保存场景失败: {}", e))?;
        // 导入的背景自动注册成场景后同样走一次 AI 室内外标注
        //（批量导入时在单个后台任务里逐个执行；已标注过的场景不会被重标）
        spawn_location_annotations(app, new_scene_ids);
    }

    Ok(scenes
        .iter()
        .map(|scene| model_to_info_with_background_index(scene, &background_index))
        .collect())
}

#[tauri::command]
pub async fn create_scene(app: AppHandle, req: CreateSceneRequest) -> Result<SceneInfo, String> {
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
        location_type: None,
    };
    let scene_id = scene.id.clone();
    let info = model_to_info(&scene);
    scenes.push(scene);
    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {}", e))?;

    // 异步 LLM 标注室内外，不阻塞创建；完成后写回并广播
    spawn_location_annotations(app, vec![scene_id]);

    Ok(info)
}

/// 批量异步标注：单个后台任务里逐个执行，避免批量导入时并发请求 LLM。
fn spawn_location_annotations(app: AppHandle, scene_ids: Vec<String>) {
    if scene_ids.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        for id in scene_ids {
            annotate_scene_location(&app, &id).await;
        }
    });
}

/// LLM 室内外标注（建场景/背景导入后异步执行一次）。
///
/// 配置了「视觉模型」时把背景图连同名称、描述一起发给视觉模型（复用截图
/// 分析的 ScreenAnalyzer 链路）；未配置或视觉请求失败时回退为对话模型纯
/// 文本判断。只输出「室内 / 室外 / 不确定」，任何失败都安静留在 unknown，
/// 用户可在场景卡上点击改写。
async fn annotate_scene_location(app: &AppHandle, scene_id: &str) {
    let store = SceneStore::new(&data_dir());
    let Some(scene) = store
        .find_by_id(scene_id)
        .ok()
        .flatten()
        .filter(|s| s.location_type.is_none())
    else {
        return;
    };

    // 优先视觉模型：与截图分析共用同一份「视觉模型」配置与识图投递链路。
    // 标注结果必须是文本，所以这个意图不走原生识图（见 ImageIntent::SceneStill）。
    let image_path = super::backgrounds_dir().join(&scene.background);
    let mut analyzer = ScreenAnalyzer::new(ScreenAnalyzerConfig::resolve(app));
    let delivery = match std::fs::read(&image_path) {
        Ok(image_bytes) => {
            deliver_image(
                app,
                &mut analyzer,
                &image_bytes,
                ImageIntent::SceneStill {
                    name: &scene.name,
                    desc: &scene.description,
                },
            )
            .await
        },
        Err(e) => {
            tracing::warn!("[SceneLocation] 读取背景图失败: {e}");
            ImageDelivery::default()
        },
    };

    if let Some(answer) = delivery.text {
        if let Some(location) = parse_location_answer(&answer) {
            store_annotation(app, scene_id, location, &scene.name);
            return;
        }
        tracing::info!("[SceneLocation] 视觉标注输出无法解析：{answer}");
    }

    // 回退：对话模型纯文本判断
    let Some(llm) = ({
        let state = app.state::<AppState>();
        crate::ai_service::llm::slot_snapshot(&state.chat.llm).await
    }) else {
        return;
    };
    if !llm.config().is_usable() {
        return;
    }
    let msgs = vec![
        crate::ai_service::types::LlmMessage::system(
            "你是场景分类助手。根据场景名称和描述判断该地点是室内还是室外。\
             只输出三个词之一：室内、室外、不确定",
        ),
        crate::ai_service::types::LlmMessage::user(format!(
            "场景名：{}\n描述：{}",
            scene.name, scene.description
        )),
    ];
    let answer = match llm.complete(&msgs).await {
        Ok(text) => text.trim().to_string(),
        Err(e) => {
            tracing::warn!("[SceneLocation] 场景「{scene_id}」室内外标注失败: {e}");
            return;
        },
    };
    match parse_location_answer(&answer) {
        Some(location) => store_annotation(app, scene_id, location, &scene.name),
        None => tracing::info!("[SceneLocation] 场景「{scene_id}」标注输出无法解析：{answer}"),
    }
}

/// 解析标注输出：「不确定/未知」→ Unknown，其次室外，再次室内。
fn parse_location_answer(answer: &str) -> Option<LocationType> {
    if answer.contains("不确定") || answer.contains("未知") {
        Some(LocationType::Unknown)
    } else if answer.contains("室外") {
        Some(LocationType::Outdoor)
    } else if answer.contains("室内") {
        Some(LocationType::Indoor)
    } else {
        None
    }
}

/// 把标注写回 scenes.json（仍为空才写入，避免覆盖标注期间用户的手动设置），
/// 成功后广播给前端就地更新。
fn store_annotation(app: &AppHandle, scene_id: &str, location: LocationType, scene_name: &str) {
    let store = SceneStore::new(&data_dir());
    let mut scenes = match store.load_all() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[SceneLocation] 读取场景失败: {e}");
            return;
        },
    };
    let mut updated = false;
    if let Some(record) = scenes.iter_mut().find(|s| s.id == scene_id) {
        if record.location_type.is_none() {
            record.location_type = Some(location);
            updated = true;
        }
    }
    if updated {
        if let Err(e) = SceneStore::new(&data_dir()).save_all(&scenes) {
            tracing::warn!("[SceneLocation] 标注写回失败: {e}");
            return;
        }
        if let Err(e) = app.emit(
            "scene:location-updated",
            serde_json::json!({ "id": scene_id, "location_type": location.as_str() }),
        ) {
            tracing::warn!("emit scene:location-updated 失败: {e}");
        }
        tracing::info!(
            "[SceneLocation] 场景「{scene_name}」标注为 {}",
            location.as_str()
        );
    }
}

/// set_scene_location_type：用户在场景卡上点击标注，手动设置室内外。
#[tauri::command]
pub async fn set_scene_location_type(
    app: AppHandle,
    id: String,
    location_type: String,
) -> Result<(), String> {
    let location = LocationType::parse(&location_type)
        .ok_or_else(|| format!("无效的室内外标注: {location_type}"))?;
    let store = SceneStore::new(&data_dir());
    let mut scenes = store.load_all().map_err(|e| format!("加载场景失败: {e}"))?;
    let record = scenes
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("场景 {id} 不存在"))?;
    record.location_type = Some(location);
    record.updated_at = now_iso();
    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {e}"))?;

    // 广播给前端就地更新
    if let Err(e) = app.emit(
        "scene:location-updated",
        serde_json::json!({ "id": id, "location_type": location.as_str() }),
    ) {
        tracing::warn!("emit scene:location-updated 失败: {e}");
    }
    Ok(())
}

// ========== 天气同步（城市 + 自动同步现实） ==========

/// get_weather_sync_config：读取天气同步所需的配置（城市 + 自动同步开关）。
#[tauri::command]
pub async fn get_weather_sync_config(app: AppHandle) -> Result<serde_json::Value, String> {
    let config = crate::config::AppConfig::load(&app).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "city": config.weather_city,
        "auto_sync": config.weather_auto_sync,
    }))
}

/// set_weather_city：保存天气城市；清空城市时一并关闭同步。
/// 同步开着时改城市，立即按新城市拉取一次天气。
#[tauri::command]
pub async fn set_weather_city(app: AppHandle, city: String) -> Result<(), String> {
    let city = city.trim().to_string();
    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        store.set(
            crate::config::keys::WEATHER_CITY.to_string(),
            serde_json::Value::String(city.clone()),
        );
        if city.is_empty() {
            store.set(crate::config::keys::WEATHER_AUTO_SYNC.to_string(), false);
        }
        let _ = store.save();
    }
    let sync_on = crate::config::AppConfig::load(&app)
        .map(|c| c.weather_auto_sync)
        .unwrap_or(false);
    if !city.is_empty() && sync_on {
        tauri::async_runtime::spawn(async move {
            crate::ai_service::tools::weather::fetch_and_apply_now(&app).await;
        });
    }
    Ok(())
}

/// set_weather_auto_sync：切换「自动同步现实天气」。城市为空时拒绝打开；
/// 打开时应用缓存并立即按城市拉取一次最新天气。
#[tauri::command]
pub async fn set_weather_auto_sync(app: AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        let city = crate::config::AppConfig::load(&app)
            .ok()
            .map(|c| c.weather_city)
            .unwrap_or_default();
        if city.trim().is_empty() {
            return Err("请先填写城市，再开启天气同步".into());
        }
    }
    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        store.set(crate::config::keys::WEATHER_AUTO_SYNC.to_string(), enabled);
        let _ = store.save();
    }
    if enabled {
        // 先用缓存立即应用一次（若有），再后台拉最新——两步都不阻塞设置页
        crate::ai_service::tools::weather::sync_weather_effect(&app).await;
        tauri::async_runtime::spawn(async move {
            crate::ai_service::tools::weather::fetch_and_apply_now(&app).await;
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn update_scene(app: AppHandle, req: UpdateSceneRequest) -> Result<SceneInfo, String> {
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

    let background = resolved_background_of(&scenes[idx]);
    let info = model_to_info(&scenes[idx]);
    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {}", e))?;

    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    if gs.current_scene_id.as_deref() == Some(&req.id) {
        gs.clear_background_override();
        gs.sync_background_with_scene(background);
    }

    Ok(info)
}

#[tauri::command]
pub async fn delete_scene(app: AppHandle, id: String) -> Result<(), String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    // 记住被删场景的背景图文件名，删除记录后据此清理孤儿文件。
    // 插件场景随插件生命周期管理（且存绝对路径），不在此删除。
    let deleted_bg = scenes
        .iter()
        .find(|s| s.id == id)
        .filter(|s| s.plugin_id.is_none())
        .map(|s| to_background_filename(&s.background))
        .unwrap_or_default();

    let before = scenes.len();
    scenes.retain(|s| s.id != id);
    if scenes.len() == before {
        return Err(format!("场景 {} 不存在", id));
    }

    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {}", e))?;

    // 若该背景图不再被任何剩余场景引用，删除文件，否则 list_scenes 会把它重新
    // 注册成只有文件名、没有描述的占位场景（删除后“复活”的根因）。
    if !deleted_bg.is_empty()
        && !scenes
            .iter()
            .any(|s| to_background_filename(&s.background) == deleted_bg)
    {
        let base = super::backgrounds_dir();
        let file_path = base.join(&deleted_bg);
        if crate::utils::path::validate_path_in_base(&file_path, &base).is_ok()
            && file_path.exists()
        {
            if let Err(e) = std::fs::remove_file(&file_path) {
                tracing::warn!("删除场景背景图失败 {deleted_bg}: {e}");
            }
        }
    }

    // 若删除的是当前选中场景，清除引用；背景文件刚被删掉，展示背景也要跟着落空
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    if gs.current_scene_id.as_deref() == Some(&id) {
        gs.current_scene_id = None;
        gs.sync_background_with_scene(None);
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
        match background_index.resolve(&scene.background, scene.category.as_deref()) {
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
                gs.sync_background_with_scene(None);
            }
        }
    }

    Ok(removed)
}

/// 删除/迁移背景分类后同步 scenes.json。
///
/// - `move_to_root`：把引用该分类下背景的相对路径改为纯文件名，category 设为 None。
/// - `delete_all`：删除后若背景还能解析到其他候选则更新为新的相对路径；
///   若彻底丢失则清空背景（保留空背景场景）；歧义时保留场景并把 category 设为 None。
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
            scene.category = None;
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
            match remaining_index.resolve(&scene.background, scene.category.as_deref()) {
                Ok(Some(path)) => {
                    if let Some(relative) = relative_storage_path(&path, bg_base) {
                        let category = category_from_storage_path(&relative);
                        scene.background = relative;
                        scene.category = category;
                    }
                },
                Err(_) => {
                    // 歧义：保留场景与背景引用，但清空分类，避免指向已删除分类。
                    scene.category = None;
                },
                Ok(None) => {
                    // 背景文件已彻底丢失：保留空背景场景，但清空背景引用。
                    scene.background = String::new();
                    scene.category = None;
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
/// category 传 None 则移动到 backgrounds 根目录；否则移动到 backgrounds/<category>/ 下。
/// 同时更新场景的 category 与背景相对路径。
#[tauri::command]
pub async fn move_scene_to_category(
    _app: AppHandle,
    id: String,
    category: Option<String>,
) -> Result<SceneInfo, String> {
    let store = SceneStore::new(&data_dir());
    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景列表失败: {}", e))?;

    let idx = scenes
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| format!("场景 {} 不存在", id))?;

    let target = if let Some(t) = category.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        let category = validate_directory_name(t)?;
        if matches!(category.as_str(), "全部" | "插件") {
            return Err("不能移动到保留分类".to_string());
        }
        Some(category)
    } else {
        None
    };

    let file_name = to_background_filename(&scenes[idx].background);
    if file_name.trim().is_empty() {
        return Err("该场景没有关联的背景，无法移动".to_string());
    }

    let bg_base = super::backgrounds_dir();
    let (_, background_index) = scan_backgrounds(&bg_base);
    let src = background_index
        .resolve(&scenes[idx].background, scenes[idx].category.as_deref())?
        .ok_or_else(|| format!("找不到背景文件: {file_name}"))?;
    validate_path_in_base(&src, &bg_base)?;
    let dest_dir = target
        .as_deref()
        .map(|category| bg_base.join(category))
        .unwrap_or_else(|| bg_base.clone());
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
        let Ok(Some(path)) = background_index.resolve(&scene.background, scene.category.as_deref())
        else {
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
    // 背景跟随场景，先按 id 把背景解析出来，别在持锁时读盘
    let background = scene_id.as_deref().and_then(resolved_background_of_id);

    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    gs.current_scene_id = scene_id.clone();

    // 换了场景，剧本临时背景的记录就此作废（它只保留到新场景更新为止）
    gs.clear_background_override();
    gs.sync_background_with_scene(background);

    // 持久化到 store，便于下次启动恢复
    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        let val = match &scene_id {
            Some(id) => serde_json::Value::String(id.clone()),
            None => serde_json::Value::Null,
        };
        store.set(crate::config::session::LAST_SCENE_ID.to_string(), val);
        let _ = store.save();
    }

    // 手动优先：用户手动选场景后，关闭 AI 自动切景（设置页可重新打开）。
    // 取消选中（scene_id 为空，背景转透明）不算选景，不动这个开关。
    if scene_id.is_some() {
        crate::config::AppConfig::lock_auto_switch(&app, crate::config::keys::AI_AUTO_SCENE);
    }

    Ok(())
}

/// 用户手动选择粒子特效（设置-背景页 / 桌宠 PetTab）。
///
/// 此前特效的手动选择只写前端 store、后端 `GameStatus.background_effect`
/// 一无所知；此命令让后端同步状态、经 ambient:effect 直达通道广播（与 AI
/// 工具同一通道），并落下"手动优先"锁。
#[tauri::command]
pub async fn select_background_effect(app: AppHandle, effect: String) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let service = state.ai_service.lock().await;
        service.game_status.lock().await.background_effect = effect.clone();
    }

    let payload = serde_json::json!({
        "type": "ambient_effect",
        "effect": effect,
        // 标记手动选择：前端回声只更新对应层，不清另一层
        "source": "manual",
    });
    if let Err(e) = app.emit("ambient:effect", &payload) {
        tracing::warn!("emit ambient effect 失败: {e}");
    }

    // 手动优先：用户手动选特效后，关闭 AI 自动切特效（设置页可重新打开）
    crate::config::AppConfig::lock_auto_switch(&app, crate::config::keys::AI_AUTO_EFFECT);

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
