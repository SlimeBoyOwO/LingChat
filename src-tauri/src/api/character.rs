use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::ai_service::types::CharacterSettings;
use crate::config;
use crate::db::entities::role::RoleType;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::system::open_folder;
use crate::utils::yaml_file::write_json_as_yaml;
use crate::AppState;

use super::{characters_dir, data_dir, game_data_dir};

const LEGACY_VOICE_MODEL_FIELDS: &[&str] = &[
    "sva_speaker_id",
    "sbv2_name",
    "sbv2_speaker_id",
    "bv2_speaker_id",
    "sbv2api_name",
    "sbv2api_speaker_id",
    "gsv_voice_text",
    "gsv_voice_filename",
    "gsv_gpt_model_name",
    "gsv_sovits_model_name",
    "aivis_model_uuid",
    "opentts_voice",
    "fish_s2_voice",
];

fn remove_legacy_voice_model_fields(settings: &mut CharacterSettings) {
    for key in LEGACY_VOICE_MODEL_FIELDS {
        settings.extra.remove(*key);
    }
}

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClothesItem {
    pub title: String,
    /// 绝对文件系统路径
    pub avatar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CharacterListItem {
    pub character_id: i32,
    pub title: String,
    pub name: String,
    pub sub_name: String,
    pub info: String,
    pub avatar_path: String,
    pub clothes: Vec<ClothesItem>,
    pub adventure_count: i32,
    pub total_adventures: i32,
    pub resource_folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CharacterPageResult {
    pub items: Vec<CharacterListItem>,
    pub total: i64,
    pub page: i32,
    pub page_size: i32,
    pub total_pages: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RoleInfoResponse {
    pub character_id: i32,
    pub ai_name: String,
    pub ai_subtitle: String,
    pub thinking_message: String,
    pub scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub scale_p: f64,
    pub offset_x_p: f64,
    pub offset_y_p: f64,
    pub bubble_top: i32,
    pub bubble_left: i32,
    pub clothes: Option<Vec<HashMap<String, String>>>,
    pub clothes_name: String,
    pub body_part: Option<HashMap<String, JsonValue>>,
    pub live2d: Option<crate::ai_service::types::Live2dSettings>,
    pub character_folder: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterImagePayload {
    pub file_name: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterEmotionPayload {
    pub emotion: String,
    pub file_name: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCharacterRequest {
    pub resource_folder: String,
    pub settings: JsonValue,
    pub avatar: CharacterImagePayload,
    pub emotions: Vec<CharacterEmotionPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateCharacterResult {
    pub character_id: i32,
    pub title: String,
    pub resource_folder: String,
}

const REQUIRED_EMOTION_SLOTS: &[&str] = &[
    "兴奋",
    "厌恶",
    "哭泣",
    "害怕",
    "害羞",
    "平静",
    "心动",
    "惊讶",
    "慌张",
    "担心",
    "无奈",
    "生气",
    "疑惑",
    "紧张",
    "自信",
    "认真",
    "调皮",
    "难为情",
    "高兴",
    "正常",
];

const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "tiff", "tif", "webp", "avif", "svg",
];

// ========== 辅助函数 ==========

/// 读取某个角色的 settings.yml，失败时返回默认值
pub(crate) fn read_character_settings(resource_folder: &str) -> CharacterSettings {
    let yaml_path = characters_dir().join(resource_folder).join("settings.yml");
    if !yaml_path.exists() {
        tracing::warn!("角色设置文件不存在: {:?}", yaml_path);
        let mut s = CharacterSettings::default();
        s.character_folder = resource_folder.to_string();
        return s;
    }
    match fs::read_to_string(&yaml_path) {
        Ok(content) => match serde_yaml::from_str::<CharacterSettings>(&content) {
            Ok(mut settings) => {
                settings.character_folder = resource_folder.to_string();
                settings
            }
            Err(e) => {
                tracing::error!("解析 {:?} 失败: {}", yaml_path, e);
                let mut s = CharacterSettings::default();
                s.character_folder = resource_folder.to_string();
                s
            }
        },
        Err(e) => {
            tracing::error!("读取 {:?} 失败: {}", yaml_path, e);
            let mut s = CharacterSettings::default();
            s.character_folder = resource_folder.to_string();
            s
        }
    }
}

/// 在指定目录中查找头像文件（名为"头像"的图片）
fn find_avatar_in_dir(dir: &PathBuf) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }
    for ext in &["png", "webp", "jpg", "jpeg", "gif", "bmp"] {
        let path = dir.join(format!("头像.{}", ext));
        if path.exists() {
            return Some(path);
        }
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("头像") && !entry.file_type().map(|t| t.is_dir()).unwrap_or(true)
            {
                return Some(entry.path());
            }
        }
    }
    None
}

/// 扫描角色头像目录，返回衣服列表（每项包含头像文件的绝对路径）
fn scan_clothes(resource_folder: &str) -> Vec<ClothesItem> {
    let allowed_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "gif"];

    let avatar_dir = characters_dir().join(resource_folder).join("avatar");
    if !avatar_dir.exists() {
        return vec![ClothesItem {
            title: "默认".to_string(),
            avatar: String::new(),
        }];
    }

    let mut clothes: Vec<ClothesItem> = Vec::new();

    let root_avatar = find_emotion_file(&avatar_dir, "正常", &allowed_extensions)
        .map(|p| p.to_string_lossy().into_owned());
    if let Some(avatar_path) = root_avatar {
        clothes.push(ClothesItem {
            title: "默认".to_string(),
            avatar: avatar_path,
        });
    }

    if let Ok(entries) = fs::read_dir(&avatar_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().into_owned();
                let subdir = entry.path();
                let avatar_path = find_emotion_file(&subdir, "正常", &allowed_extensions)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                clothes.push(ClothesItem {
                    title: name,
                    avatar: avatar_path,
                });
            }
        }
    }

    if clothes.is_empty() {
        let default_path = find_avatar_in_dir(&avatar_dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        clothes.push(ClothesItem {
            title: "默认".to_string(),
            avatar: default_path,
        });
    }

    clothes
}

/// 获取角色默认头像的绝对路径
fn default_avatar_path(resource_folder: &str) -> String {
    let avatar_dir = characters_dir().join(resource_folder).join("avatar");
    for ext in &["png", "webp", "jpg", "jpeg", "gif", "bmp"] {
        let path = avatar_dir.join(format!("头像.{}", ext));
        if path.exists() {
            return path.to_string_lossy().into_owned();
        }
    }
    if let Ok(entries) = fs::read_dir(&avatar_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let name = fname.to_string_lossy();
            if name.starts_with("头像") && !entry.file_type().map(|t| t.is_dir()).unwrap_or(true)
            {
                return entry.path().to_string_lossy().into_owned();
            }
        }
    }
    avatar_dir.to_string_lossy().into_owned()
}

/// 在目录中查找文件名（不含扩展名）匹配的图片文件
pub(crate) fn find_emotion_file(dir: &PathBuf, stem: &str, extensions: &[&str]) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !extensions.contains(&ext.to_lowercase().as_str()) {
            continue;
        }
        if path.file_stem().and_then(|s| s.to_str()) == Some(stem) {
            return Some(path);
        }
    }
    None
}

fn validate_resource_folder(value: &str) -> Result<String, String> {
    let folder = value.trim();
    if folder.is_empty() || folder == "." || folder == ".." {
        return Err("resource_folder 无效".to_string());
    }
    if Path::new(folder).file_name().and_then(|name| name.to_str()) != Some(folder)
        || folder.contains('/')
        || folder.contains('\\')
        || folder
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err("resource_folder 包含非法字符".to_string());
    }
    Ok(folder.to_string())
}

fn image_extension(file_name: &str, field: &str) -> Result<String, String> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| format!("{field} 缺少扩展名"))?;
    if !SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!("{field} 图片格式不支持: {extension}"));
    }
    Ok(extension)
}

fn validate_emotions(
    emotions: &[CharacterEmotionPayload],
) -> Result<HashMap<String, &CharacterEmotionPayload>, String> {
    if emotions.len() != REQUIRED_EMOTION_SLOTS.len() {
        return Err(format!(
            "必须提供 {} 个情绪立绘，实际为 {} 个",
            REQUIRED_EMOTION_SLOTS.len(),
            emotions.len()
        ));
    }
    let mut result = HashMap::new();
    for item in emotions {
        if !REQUIRED_EMOTION_SLOTS.contains(&item.emotion.as_str()) {
            return Err(format!("未知情绪槽: {}", item.emotion));
        }
        image_extension(&item.file_name, &format!("{} file", item.emotion))?;
        if item.data.is_empty() {
            return Err(format!("{} file 内容为空", item.emotion));
        }
        if result.insert(item.emotion.clone(), item).is_some() {
            return Err(format!("情绪槽重复: {}", item.emotion));
        }
    }
    for required in REQUIRED_EMOTION_SLOTS {
        if !result.contains_key(*required) {
            return Err(format!("缺少情绪槽: {required}"));
        }
    }
    Ok(result)
}

fn emotion_storage_name(emotion: &str) -> &str {
    match emotion {
        "哭泣" => "伤心",
        "难为情" => "羞耻",
        other => other,
    }
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub async fn create_character_native(
    app: AppHandle,
    request: CreateCharacterRequest,
) -> Result<CreateCharacterResult, String> {
    let folder = validate_resource_folder(&request.resource_folder)?;
    let target = characters_dir().join(&folder);
    if target.exists() {
        return Err(format!("角色目录已存在: {folder}"));
    }

    let state = app.state::<AppState>();
    if RoleRepo::get_all_main_roles(&state.db)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .iter()
        .any(|role| role.resource_folder.as_deref() == Some(folder.as_str()))
    {
        return Err(format!("角色已存在: {folder}"));
    }

    let mut settings: CharacterSettings =
        serde_json::from_value(request.settings).map_err(|e| format!("配置验证失败: {e}"))?;
    remove_legacy_voice_model_fields(&mut settings);
    let avatar_extension = image_extension(&request.avatar.file_name, "avatar_file")?;
    if request.avatar.data.is_empty() {
        return Err("avatar_file 内容为空".to_string());
    }
    let emotions = validate_emotions(&request.emotions)?;
    let title = settings.title.clone().unwrap_or_else(|| folder.clone());

    let staging = characters_dir().join(format!(".create-{}", uuid::Uuid::new_v4()));
    let avatar_dir = staging.join("avatar");
    let create_result = (|| -> Result<(), String> {
        fs::create_dir_all(&avatar_dir).map_err(|e| format!("创建角色目录失败: {e}"))?;
        fs::write(
            avatar_dir.join(format!("头像.{avatar_extension}")),
            &request.avatar.data,
        )
        .map_err(|e| format!("保存头像失败: {e}"))?;

        for emotion in REQUIRED_EMOTION_SLOTS {
            let file = emotions
                .get(*emotion)
                .ok_or_else(|| format!("缺少情绪槽: {emotion}"))?;
            let extension = image_extension(&file.file_name, emotion)?;
            fs::write(
                avatar_dir.join(format!("{}.{}", emotion_storage_name(emotion), extension)),
                &file.data,
            )
            .map_err(|e| format!("保存 {emotion} 立绘失败: {e}"))?;
        }

        let mut save_data =
            serde_json::to_value(&settings).map_err(|e| format!("配置规范化失败: {e}"))?;
        if let Some(object) = save_data.as_object_mut() {
            for transient in [
                "character_id",
                "resource_path",
                "character_folder",
                "script_key",
                "script_role_key",
            ] {
                object.remove(transient);
            }
        }
        write_json_as_yaml(&staging.join("settings.yml"), &save_data)?;
        fs::rename(&staging, &target).map_err(|e| format!("保存角色目录失败: {e}"))?;
        Ok(())
    })();
    if let Err(error) = create_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let role_id = match RoleRepo::find_or_create_role(
        &state.db,
        &title,
        RoleType::Main,
        None,
        None,
        Some(&folder),
    )
    .await
    {
        Ok(id) => id,
        Err(error) => {
            let _ = fs::remove_dir_all(&target);
            return Err(format!("注册角色失败: {error}"));
        }
    };
    let _ = app.emit("role:list-updated", ());

    Ok(CreateCharacterResult {
        character_id: role_id,
        title,
        resource_folder: folder,
    })
}

#[tauri::command]
pub async fn get_character_list(
    app: AppHandle,
    page: i32,
    page_size: i32,
) -> Result<CharacterPageResult, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let all_roles = RoleRepo::get_all_main_roles(db)
        .await
        .map_err(|e| format!("查询角色列表失败: {}", e))?;

    let total = all_roles.len() as i64;
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as i32;
    let start = ((page - 1) * page_size).max(0) as usize;
    // 防御：start 可能 > len（极端：page 远超 total_pages），反向切片会 panic。
    // 这里把 start 钳制到 len，使切片返回空数组而不是崩溃。
    let start = start.min(all_roles.len());
    let end = (start + page_size as usize).min(all_roles.len());
    let page_roles = &all_roles[start..end];

    // Pre-compute adventure counts for all characters on this page
    let mut items = Vec::new();
    for role in page_roles {
        let folder = role.resource_folder.clone().unwrap_or_default();
        let settings = read_character_settings(&folder);

        let (total_adventures, adventure_count) = {
            let service = state.ai_service.lock().await;
            let adventures = service.script_manager.get_character_adventures(&folder);
            let total = adventures.len() as i32;
            let unlocked = {
                let mut count = 0i32;
                for adv in &adventures {
                    if crate::adventures::manager::AdventureManager::is_unlocked(
                        db,
                        &adv.folder_key,
                    )
                    .await
                    .unwrap_or(false)
                    {
                        count += 1;
                    }
                }
                count
            };
            (total, unlocked)
        };

        items.push(CharacterListItem {
            character_id: role.id,
            title: role.name.clone(),
            name: settings.ai_name,
            sub_name: settings.ai_subtitle.unwrap_or_default(),
            info: settings.info.unwrap_or_default(),
            avatar_path: default_avatar_path(&folder),
            clothes: scan_clothes(&folder),
            adventure_count,
            total_adventures,
            resource_folder: folder,
        });
    }

    Ok(CharacterPageResult {
        items,
        total,
        page,
        page_size,
        total_pages,
    })
}

#[tauri::command]
pub async fn get_role_info(app: AppHandle, role_id: i32) -> Result<RoleInfoResponse, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let role = RoleRepo::get_role_by_id(db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {}", e))?
        .ok_or_else(|| format!("角色 {} 不存在", role_id))?;

    let folder = role.resource_folder.clone().unwrap_or_default();
    let settings = RoleRepo::get_role_settings_by_id(db, &data_dir(), role_id)
        .await
        .map_err(|e| format!("读取角色配置失败: {e}"))?
        .unwrap_or_else(|| read_character_settings(&folder));

    Ok(RoleInfoResponse {
        character_id: role.id,
        ai_name: settings.ai_name,
        ai_subtitle: settings.ai_subtitle.unwrap_or_default(),
        thinking_message: settings.thinking_message,
        scale: settings.scale,
        offset_x: settings.offset_x,
        offset_y: settings.offset_y,
        scale_p: settings.scale_p,
        offset_x_p: settings.offset_x_p,
        offset_y_p: settings.offset_y_p,
        bubble_top: settings.bubble_top,
        bubble_left: settings.bubble_left,
        clothes: settings.clothes,
        clothes_name: settings.clothes_name.unwrap_or_default(),
        body_part: settings.body_part,
        live2d: settings.live2d,
        character_folder: folder,
    })
}

#[tauri::command]
pub async fn get_role_settings(app: AppHandle, role_id: i32) -> Result<CharacterSettings, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    RoleRepo::get_role_settings_by_id(db, &data_dir(), role_id)
        .await
        .map_err(|e| format!("读取角色配置失败: {}", e))?
        .ok_or_else(|| format!("角色 {} 不存在或其配置不可用", role_id))
}

#[tauri::command]
pub fn get_character_file(file_path: String) -> Result<String, String> {
    let base = characters_dir();
    let resolved = base.join(&file_path);

    crate::utils::path::validate_path_in_base(&resolved, &base)?;

    if !resolved.exists() {
        return Err(format!("角色文件不存在: {}", file_path));
    }

    let canon = resolved
        .canonicalize()
        .map_err(|e| format!("路径解析失败: {}", e))?;
    Ok(canon.to_string_lossy().into_owned())
}

/// Enumerate every script package directory on disk.
///
/// Mirrors the three layouts `ScriptManager::scan_scripts` accepts:
/// `scripts/character/<角色>/<剧本>/`, `scripts/standalone/<剧本>/` and the
/// legacy flat `scripts/<剧本>/`. The avatar lookup used to only walk one level,
/// so a script NPC living under the two-level `character/<角色>/<剧本>/` layout —
/// which is what every 羁绊冒险 uses — could never have its portrait found.
fn script_package_dirs() -> Vec<PathBuf> {
    let scripts_dir = game_data_dir().join("scripts");
    let mut out = Vec::new();

    let Ok(level1) = fs::read_dir(&scripts_dir) else {
        return out;
    };

    for entry in level1.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        match name.as_str() {
            // scripts/character/<角色>/<剧本>/ —— 需要再下钻两级
            "character" => {
                if let Ok(roles) = fs::read_dir(&path) {
                    for role in roles.flatten() {
                        if !role.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            continue;
                        }
                        if let Ok(scripts) = fs::read_dir(role.path()) {
                            for s in scripts.flatten() {
                                if s.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                    out.push(s.path());
                                }
                            }
                        }
                    }
                }
            }
            // scripts/standalone/<剧本>/ —— 再下钻一级
            "standalone" => {
                if let Ok(scripts) = fs::read_dir(&path) {
                    for s in scripts.flatten() {
                        if s.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            out.push(s.path());
                        }
                    }
                }
            }
            // scripts/<剧本>/ —— 兼容布局，目录本身就是剧本包
            _ => out.push(path),
        }
    }

    out
}

#[tauri::command]
pub fn get_avatar_file(
    character_folder: String,
    emotion: String,
    clothes_name: String,
) -> Result<String, String> {
    let allowed_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "gif"];

    let clothes_subdir = if clothes_name.is_empty() || clothes_name == "default" {
        String::new()
    } else {
        clothes_name.clone()
    };

    let mut candidate_bases: Vec<PathBuf> = Vec::new();

    // 1. 主角色: characters/{folder}/avatar
    let main_avatar = characters_dir().join(&character_folder).join("avatar");
    if main_avatar.exists() {
        candidate_bases.push(main_avatar);
    }

    // 2. NPC/脚本角色: <剧本目录>/characters/{folder}/avatar
    for script_dir in script_package_dirs() {
        let npc_avatar = script_dir
            .join("characters")
            .join(&character_folder)
            .join("avatar");
        if npc_avatar.exists() {
            candidate_bases.push(npc_avatar);
        }
    }

    for base in &candidate_bases {
        let search_dir = if clothes_subdir.is_empty() {
            base.clone()
        } else {
            base.join(&clothes_subdir)
        };

        if !search_dir.exists() {
            continue;
        }

        if let Some(found) = find_emotion_file(&search_dir, &emotion, &allowed_extensions) {
            let canon = found
                .canonicalize()
                .map_err(|e| format!("路径解析失败: {}", e))?;
            return Ok(canon.to_string_lossy().into_owned());
        }

        // 如果情绪是"平静"没找到，回退到"正常"
        if emotion == "平静" {
            if let Some(found) = find_emotion_file(&search_dir, "正常", &allowed_extensions) {
                let canon = found
                    .canonicalize()
                    .map_err(|e| format!("路径解析失败: {}", e))?;
                return Ok(canon.to_string_lossy().into_owned());
            }
        }
    }

    Err(format!(
        "未找到角色头像: folder={}, emotion={}, clothes={}",
        character_folder, emotion, clothes_name
    ))
}

#[tauri::command]
pub async fn select_clothes(
    app: AppHandle,
    role_id: i32,
    clothes_name: String,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;

    let db = &state.db;

    // 持久化该角色的服装选择（按角色 ID 存储）
    if let Ok(store) = app.store(config::STORE_FILE) {
        let key = config::session::last_clothes_key(role_id);
        store.set(key, JsonValue::String(clothes_name.clone()));
        let _ = store.save();
    }

    // 在游戏内记录服装方便复原
    service
        .game_status
        .lock()
        .await
        .role_manager
        .set_character_clothes_override(role_id, clothes_name.clone());

    // 委托给 GameStatus 统一处理换装逻辑（去重 + 旁白生成）
    let switched = service
        .game_status
        .lock()
        .await
        .on_character_change_clothes(db, role_id, &clothes_name)
        .await
        .map_err(|e| format!("切换服装失败: {}", e))?;

    if switched {
        Ok(serde_json::json!({"success": true, "message": "衣服更换成功"}))
    } else {
        Ok(serde_json::json!({"success": true, "message": "当前衣服已经是选中状态"}))
    }
}

#[tauri::command]
pub async fn update_role_settings(
    app: AppHandle,
    role_id: i32,
    settings: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let role = RoleRepo::get_role_by_id(db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {}", e))?
        .ok_or_else(|| format!("角色 {} 不存在", role_id))?;

    let folder = role
        .resource_folder
        .clone()
        .ok_or_else(|| format!("角色 {} 资源不存在", role_id))?;

    let base_path = match role.role_type {
        RoleType::Main => characters_dir().join(&folder),
        RoleType::Npc => {
            let script_key = role
                .script_key
                .clone()
                .ok_or_else(|| format!("角色 {} 缺少剧本关联", role_id))?;
            game_data_dir()
                .join("scripts")
                .join(&script_key)
                .join("characters")
                .join(&folder)
        }
        RoleType::System | RoleType::User => {
            return Err("系统角色不允许修改配置".to_string());
        }
    };

    if !base_path.exists() {
        return Err(format!("角色目录不存在: {:?}", base_path));
    }

    let mut validated: CharacterSettings =
        serde_json::from_value(settings).map_err(|e| format!("配置验证失败: {}", e))?;
    remove_legacy_voice_model_fields(&mut validated);

    let mut save_data =
        serde_json::to_value(&validated).map_err(|e| format!("配置规范化失败: {}", e))?;
    if let Some(obj) = save_data.as_object_mut() {
        obj.remove("character_id");
        obj.remove("resource_path");
        obj.remove("character_folder");
        obj.remove("script_key");
        obj.remove("script_role_key");
    }

    let yaml_path = base_path.join("settings.yml");
    write_json_as_yaml(&yaml_path, &save_data).map_err(|e| format!("保存失败: {e}"))?;

    let runtime_updated = {
        let service = state.ai_service.lock().await;
        let mut gs = service.game_status.lock().await;
        let voice_updated = gs
            .role_manager
            .update_role_voice_settings(role_id, &validated);
        let live2d_updated = gs
            .role_manager
            .update_role_live2d_settings(role_id, &validated);
        voice_updated || live2d_updated
    };

    tracing::info!(
        "角色 {} 配置已保存到 {:?}, runtime_updated={}",
        role_id,
        yaml_path,
        runtime_updated,
    );
    Ok(serde_json::json!({
        "success": true,
        "message": "设置已保存",
        "runtime_updated": runtime_updated,
    }))
}

#[tauri::command]
pub fn open_characters_folder() -> Result<(), String> {
    let char_dir = characters_dir();
    if !char_dir.exists() {
        fs::create_dir_all(&char_dir).map_err(|e| format!("创建角色目录失败: {}", e))?;
    }

    let path_str = char_dir.to_string_lossy().into_owned();
    open_folder(&path_str)
}

// ========== 角色删除 ==========

/// 删除一个 main 类型角色（含关联存档、记忆、对话历史、物理资源目录）。
///
/// 校验链：
/// 1. 角色存在
/// 2. 不在系统保护列表（id ∈ {0, 1, 2}）
/// 3. role_type == Main（NPC 由剧本管，system/user 不允许删）
/// 4. 不在场（game_status.present_role_ids / current_role_id / main_role_id / onstage_role_ids 任一命中即拒绝）
///
/// 删除顺序：先物理资源（可选，用户确认），再 DB 级联（事务）。若失败：
/// - 物理失败：整体放弃，DB 不动
/// - DB 失败：物理已删但下次 rescan 会重新入库（可恢复）
#[tauri::command]
pub async fn delete_character(
    app: AppHandle,
    role_id: i32,
    delete_resource_folder: bool,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    // ---- 1. 角色存在性 ----
    let role = RoleRepo::get_role_by_id(db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {}", e))?
        .ok_or_else(|| format!("角色 {} 不存在", role_id))?;

    // ---- 2. 系统保护 ----
    if RoleRepo::is_system_protected_role(role_id) {
        return Err("无法删除".to_string());
    }

    // ---- 3. 角色类型校验 ----
    if role.role_type != RoleType::Main {
        return Err("只能删除 main 类型的主角色".to_string());
    }

    // ---- 4. 在场校验（后端权威） ----
    {
        let service = state.ai_service.lock().await;
        let gs = service.game_status.lock().await;
        let onstage = gs.present_role_ids.contains(&role_id)
            || gs.current_role_id == Some(role_id)
            || gs.main_role_id == Some(role_id)
            || gs.onstage_role_ids.contains(&role_id);
        if onstage {
            return Err(format!("角色「{}」正在对话中，无法删除", role.name));
        }
    }

    // ---- 5. 先删物理资源（可选） ----
    if delete_resource_folder {
        if let Some(folder) = &role.resource_folder {
            let base = characters_dir();
            let target = base.join(folder);
            // 路径穿越防护
            crate::utils::path::validate_path_in_base(&target, &base)?;
            if target.exists() {
                if let Err(e) = fs::remove_dir_all(&target) {
                    return Err(format!("删除资源目录失败: {}", e));
                }
            }
        }
    }

    // ---- 6. DB 级联删除（事务） ----
    let deleted = RoleRepo::delete_main_role(db, role_id)
        .await
        .map_err(|e| format!("删除角色失败: {}", e))?;
    if !deleted {
        return Err(format!("角色 {} 不存在或已被删除", role_id));
    }

    // ---- 7. 广播角色列表更新事件 ----
    let _ = app.emit("role:list-updated", ());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emotion_payload(emotion: &str) -> CharacterEmotionPayload {
        CharacterEmotionPayload {
            emotion: emotion.to_string(),
            file_name: format!("{emotion}.webp"),
            data: vec![1],
        }
    }

    #[test]
    fn resource_folder_validation_rejects_paths_and_windows_reserved_characters() {
        assert_eq!(validate_resource_folder(" Nori ").unwrap(), "Nori");
        for invalid in ["", ".", "..", "a/b", "a\\b", "a:b", "a*b"] {
            assert!(
                validate_resource_folder(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn emotion_validation_requires_each_slot_exactly_once() {
        let valid = REQUIRED_EMOTION_SLOTS
            .iter()
            .map(|emotion| emotion_payload(emotion))
            .collect::<Vec<_>>();
        assert_eq!(
            validate_emotions(&valid).unwrap().len(),
            REQUIRED_EMOTION_SLOTS.len()
        );

        let mut duplicate = valid;
        duplicate[0].emotion = duplicate[1].emotion.clone();
        assert!(validate_emotions(&duplicate).is_err());
    }

    #[test]
    fn image_validation_rejects_unsupported_extensions() {
        assert_eq!(image_extension("avatar.WEBP", "avatar").unwrap(), "webp");
        assert!(image_extension("avatar.exe", "avatar").is_err());
    }
}
