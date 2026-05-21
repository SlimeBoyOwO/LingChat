use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager};

use crate::ai_service::types::{CharacterSettings, LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::db::managers::role_repo::RoleRepo;
use crate::AppState;

use super::{characters_dir, data_dir, game_data_dir};

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
    pub bubble_top: i32,
    pub bubble_left: i32,
    pub clothes: Option<Vec<HashMap<String, String>>>,
    pub clothes_name: String,
    pub body_part: Option<HashMap<String, JsonValue>>,
    pub character_folder: String,
}

// ========== 辅助函数 ==========

/// 读取某个角色的 settings.yml，失败时返回默认值
pub(crate) fn read_character_settings(resource_folder: &str) -> CharacterSettings {
    let yaml_path = characters_dir().join(resource_folder).join("settings.yml");
    if !yaml_path.exists() {
        log::warn!("角色设置文件不存在: {:?}", yaml_path);
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
                log::error!("解析 {:?} 失败: {}", yaml_path, e);
                let mut s = CharacterSettings::default();
                s.character_folder = resource_folder.to_string();
                s
            }
        },
        Err(e) => {
            log::error!("读取 {:?} 失败: {}", yaml_path, e);
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

// ========== Tauri 命令 ==========

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
    let end = (start + page_size as usize).min(all_roles.len());

    let page_roles = &all_roles[start..end];

    let mut items = Vec::new();
    for role in page_roles {
        let folder = role.resource_folder.clone().unwrap_or_default();
        let settings = read_character_settings(&folder);

        items.push(CharacterListItem {
            character_id: role.id,
            title: role.name.clone(),
            name: settings.ai_name,
            sub_name: settings.ai_subtitle.unwrap_or_default(),
            info: settings.info.unwrap_or_default(),
            avatar_path: default_avatar_path(&folder),
            clothes: scan_clothes(&folder),
            adventure_count: 0,
            total_adventures: 0,
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
    let settings = read_character_settings(&folder);

    Ok(RoleInfoResponse {
        character_id: role.id,
        ai_name: settings.ai_name,
        ai_subtitle: settings.ai_subtitle.unwrap_or_default(),
        thinking_message: settings.thinking_message,
        scale: settings.scale,
        offset_x: settings.offset_x,
        offset_y: settings.offset_y,
        bubble_top: settings.bubble_top,
        bubble_left: settings.bubble_left,
        clothes: settings.clothes,
        clothes_name: settings.clothes_name.unwrap_or_default(),
        body_part: settings.body_part,
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

    super::validate_path_in_base(&resolved, &base)?;

    if !resolved.exists() {
        return Err(format!("角色文件不存在: {}", file_path));
    }

    let canon = resolved
        .canonicalize()
        .map_err(|e| format!("路径解析失败: {}", e))?;
    Ok(canon.to_string_lossy().into_owned())
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

    // 2. NPC/脚本角色: scripts/*/characters/{folder}/avatar
    let scripts_dir = game_data_dir().join("scripts");
    if let Ok(entries) = fs::read_dir(&scripts_dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let npc_avatar = entry
                .path()
                .join("characters")
                .join(&character_folder)
                .join("avatar");
            if npc_avatar.exists() {
                candidate_bases.push(npc_avatar);
            }
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
    clothes_name: String,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let mut service = state.ai_service.lock().await;
    let db = &state.db;

    let main_role_id = service
        .game_status
        .main_role_id
        .ok_or_else(|| "角色不存在".to_string())?;

    // 收集角色数据后释放借用，再调用 add_line
    let (_, prompt) = {
        let role = service
            .game_status
            .get_role(db, main_role_id)
            .await
            .map_err(|e| format!("获取角色失败: {}", e))?;

        if role.current_clothes == clothes_name {
            return Ok(serde_json::json!({"success": true, "message": "当前衣服已经是选中状态"}));
        }

        role.current_clothes = clothes_name.clone();

        let ai_name = role.settings.ai_name.clone();
        let prompt = role
            .settings
            .clothes
            .as_ref()
            .and_then(|clothes_list| {
                clothes_list.iter().find_map(|item| {
                    if item.get("name").map(|s| s.as_str()) == Some(clothes_name.as_str()) {
                        item.get("prompt").cloned()
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| format!("{}换上了新服装：{}", ai_name, clothes_name));

        (ai_name, prompt)
    };

    service
        .game_status
        .add_line(
            db,
            LineBase {
                content: format!("{{系统提示: {}}}", prompt),
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("旁白".to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("添加台词失败: {}", e))?;

    Ok(serde_json::json!({"success": true, "message": "衣服更换成功"}))
}
