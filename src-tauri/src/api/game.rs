use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value as JsonValue;
use sea_orm::*;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::generator::{GeneratorDeps, MessageGenerator};
use crate::ai_service::types::{CharacterSettings, GameLine, LineAttributeExt, LineBase};
use crate::config::{self, AppConfig};
use crate::db::entities::line;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::{sys_prompt_builder_by_settings, PromptOptions, PromptRole};
use crate::AppState;

// ========== 响应类型 ==========

/// 对应前端 `WebInitData`（`src/api/services/game-info.ts`）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WebInitData {
    pub character_settings: CharacterSettingsInit,
    pub current_interact_role_id: Option<i32>,
    pub onstage_roles_ids: Vec<i32>,
    pub background: String,
    pub background_effect: String,
    pub background_music: String,
    pub current_scene_id: Option<String>,
    pub current_scene: Option<super::scene::SceneInfo>,
    /// 在场角色的设定（含主角与非主角），前端据此初始化 gameRoles 与 presentRoleIds
    pub onstage_roles: Vec<CharacterSettingsInit>,
    /// 初始化台词列表（至少包含一条 system 人设台词）
    pub lines: Vec<GameLineInit>,
    /// 场景感知开关（切换场景时是否自动产生旁白）
    pub scene_awareness_enabled: bool,
}

/// 精简的角色设定，匹配前端 `CharacterSettings` 接口
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CharacterSettingsInit {
    pub ai_name: String,
    pub ai_subtitle: String,
    pub user_name: String,
    pub user_subtitle: String,
    pub character_id: Option<i32>,
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
    pub body_part: Option<HashMap<String, serde_json::Value>>,
    pub character_folder: String,
}

impl From<&CharacterSettings> for CharacterSettingsInit {
    fn from(s: &CharacterSettings) -> Self {
        Self {
            ai_name: s.ai_name.clone(),
            ai_subtitle: s.ai_subtitle.clone().unwrap_or_default(),
            user_name: s.user_name.clone(),
            user_subtitle: s.user_subtitle.clone().unwrap_or_default(),
            character_id: s.character_id,
            thinking_message: s.thinking_message.clone(),
            scale: s.scale,
            offset_x: s.offset_x,
            offset_y: s.offset_y,
            scale_p: s.scale_p,
            offset_x_p: s.offset_x_p,
            offset_y_p: s.offset_y_p,
            bubble_top: s.bubble_top,
            bubble_left: s.bubble_left,
            clothes: s.clothes.clone(),
            clothes_name: s.clothes_name.clone().unwrap_or_default(),
            body_part: s.body_part.clone(),
            character_folder: s.character_folder.clone(),
        }
    }
}

/// 前端用台词条目（匹配 `src/api/services/history.ts` 的 GameLine 接口）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GameLineInit {
    pub content: String,
    pub attribute: String,
    pub sender_role_id: Option<i32>,
    pub display_name: Option<String>,
    pub original_emotion: Option<String>,
    pub predicted_emotion: Option<String>,
    pub action_content: Option<String>,
    pub audio_file: Option<String>,
    pub perceived_role_ids: Vec<i32>,
    /// 玩家消息序号（1-indexed），仅对 sender_role_id == Some(0) 的 User 行有值
    pub user_message_seq: Option<u32>,
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub async fn reactivate_tts(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    service
        .game_status
        .lock()
        .await
        .reactivate_all_voice_makers();
    tracing::info!("TTS 服务已通过 reactivate_tts 命令重新启用");
    Ok(())
}

/// 获取所有被台词引用的语音文件名。
async fn get_referenced_voice_files(db: &DatabaseConnection) -> Result<HashSet<String>, String> {
    line::Entity::find()
        .select_only()
        .column(line::Column::AudioFile)
        .filter(line::Column::AudioFile.is_not_null())
        .into_tuple::<Option<String>>()
        .all(db)
        .await
        .map(|v| v.into_iter().filter_map(|x| x).collect())
        .map_err(|e| format!("查询语音文件引用失败: {e}"))
}

/// 统计 voice/ 目录中的孤立文件。
fn count_orphan_files_in_voice_dir(
    voice_dir: &std::path::Path,
    referenced: &HashSet<String>,
) -> (usize, u64) {
    let mut files = 0usize;
    let mut size = 0u64;
    if !voice_dir.exists() {
        return (files, size);
    }
    if let Ok(entries) = std::fs::read_dir(voice_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if referenced.contains(&name) {
                continue;
            }
            files += 1;
            if let Ok(meta) = entry.metadata() {
                size += meta.len();
            }
        }
    }
    (files, size)
}

#[tauri::command]
pub async fn clear_tts_cache(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let db = state.db.clone();

    let data_dir = crate::api::data_dir();
    let voice_dir = data_dir.join("voice");

    if !voice_dir.exists() {
        events::emit_tts_cleanup(&app, 0, 0, 0);
        return Ok(serde_json::json!({
            "success": true,
            "message": "TTS 缓存目录不存在，无需清理",
            "deleted": 0
        }));
    }

    let referenced = get_referenced_voice_files(&db).await?;
    let (orphan_files_before, orphan_size_before) = count_orphan_files_in_voice_dir(&voice_dir, &referenced);

    let mut deleted: u64 = 0;
    let mut failed: usize = 0;

    if let Ok(entries) = std::fs::read_dir(&voice_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            // 只删除未被数据库引用的孤立文件，避免破坏存档语音
            if referenced.contains(&name) {
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => deleted += 1,
                Err(e) => {
                    tracing::warn!("删除 TTS 缓存文件失败 {:?}: {}", path, e);
                    failed += 1;
                }
            }
        }
    }

    let (orphan_files_after, orphan_size_after) = count_orphan_files_in_voice_dir(&voice_dir, &referenced);
    events::emit_tts_cleanup(&app, deleted, orphan_files_after, orphan_size_after);

    tracing::info!("TTS 缓存清理完成: 删除 {} 个孤立文件, 失败 {} 个", deleted, failed);
    Ok(serde_json::json!({
        "success": failed == 0,
        "message": format!("已清理 {} 个孤立 TTS 缓存文件", deleted),
        "deleted": deleted,
        "failed": failed,
        "orphan_files_before": orphan_files_before,
        "orphan_size_before": orphan_size_before,
    }))
}

/// 实时切换指定角色的语音语言，无需保存 settings.yml。
#[tauri::command]
pub async fn update_voice_lang(app: AppHandle, role_id: i32, lang: String) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;

    gs.role_manager.update_role_voice_lang(role_id, &lang);

    tracing::info!("角色 {} 语音语言已实时切换为: {}", role_id, lang);
    Ok(serde_json::json!({
        "success": true,
        "role_id": role_id,
        "lang": lang,
    }))
}

#[tauri::command]
pub async fn get_tts_cache_info(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let db = state.db.clone();

    let data_dir = crate::api::data_dir();
    let voice_dir = data_dir.join("voice");

    let referenced = get_referenced_voice_files(&db).await?;

    let mut total_size: u64 = 0;
    let mut file_count: usize = 0;
    let mut orphan_size: u64 = 0;
    let mut orphan_files: usize = 0;

    if voice_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&voice_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let size = if let Ok(metadata) = std::fs::metadata(&path) {
                    metadata.len()
                } else {
                    0
                };
                file_count += 1;
                total_size += size;
                if !referenced.contains(&name) {
                    orphan_files += 1;
                    orphan_size += size;
                }
            }
        }
    }

    Ok(serde_json::json!({
        "size": total_size,
        "files": file_count,
        "orphan_size": orphan_size,
        "orphan_files": orphan_files,
    }))
}

#[tauri::command]
pub async fn init_game(app: AppHandle) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    build_web_init_data(&service, &app).await
}

// ========== 角色切换 ==========

#[tauri::command]
pub async fn select_character(app: AppHandle, character_id: i32) -> Result<WebInitData, String> {
    let data_dir = crate::api::data_dir();

    // 1. 从 DB 加载角色设定
    let state = app.state::<AppState>();
    let db = &state.db;

    let settings = RoleRepo::get_role_settings_by_id(db, &data_dir, character_id)
        .await
        .map_err(|e| format!("查询角色配置失败: {}", e))?
        .unwrap_or_else(|| {
            tracing::warn!("角色 {} 无配置文件，使用默认设定", character_id);
            let mut s = CharacterSettings::default();
            s.character_id = Some(character_id);
            s
        });

    // 2. 读取 AppConfig 构建 PromptOptions
    let app_config = AppConfig::load(&app).unwrap_or_default();
    let prompt_options = PromptOptions {
        output_sec_lang: app_config.llm_output_sec_lang,
        no_emotion_limit: app_config.no_emotion_limit_prompt,
    };

    // 3. 更新 AIService 状态
    {
        let mut service = state.ai_service.lock().await;
        service
            .import_settings(settings.clone(), prompt_options)
            .await;
        service
            .init_game_status()
            .await
            .map_err(|e| format!("初始化游戏状态失败: {}", e))?;
    }

    // 4. 持久化上次游玩的角色 ID
    if let Ok(store) = app.store(config::STORE_FILE) {
        store.set(
            config::keys::LAST_CHARACTER_ID.to_string(),
            JsonValue::Number((character_id as i64).into()),
        );
        let _ = store.save();
    }

    tracing::info!(
        "切换角色成功: id={}, name={}",
        character_id,
        settings.ai_name
    );

    // 5. 返回最新游戏状态（复用 init_game 逻辑）
    //    drop 后再拿锁，避免同一个锁两次借用
    let init = {
        let service = state.ai_service.lock().await;
        build_web_init_data(&service, &app).await?
    };
    Ok(init)
}

/// 为台词列表计算玩家消息序号（1-indexed）。
/// 玩家消息由 `sender_role_id == Some(0) && attribute == User` 标识。
pub fn compute_user_message_seqs(line_list: &[GameLine]) -> Vec<Option<u32>> {
    let mut count = 0u32;
    line_list
        .iter()
        .map(|gl| {
            if gl.base.sender_role_id == Some(0) && matches!(gl.attribute(), LineAttribute::User) {
                count += 1;
                Some(count)
            } else {
                None
            }
        })
        .collect()
}

/// 从 AIService 快照构建 WebInitData（不持锁的函数）
pub(crate) async fn build_web_init_data(
    service: &crate::ai_service::service::AIService,
    app: &AppHandle,
) -> Result<WebInitData, String> {
    let settings = service
        .settings
        .as_ref()
        .ok_or_else(|| "AI 服务尚未初始化角色设定".to_string())?;

    let character_settings = CharacterSettingsInit::from(settings);

    let (
        lines,
        current_scene_id,
        current_role_id,
        onstage_roles_ids,
        onstage_roles,
        background,
        background_effect,
        background_music,
        scene_awareness_enabled,
    ) = {
        let mut gs = service.game_status.lock().await;
        let seqs = compute_user_message_seqs(&gs.line_list);
        let lines: Vec<GameLineInit> = gs
            .line_list
            .iter()
            .zip(seqs.iter())
            .map(|(gl, &seq)| GameLineInit {
                content: gl.base.content.clone(),
                attribute: gl.base.attribute.as_str().to_string(),
                sender_role_id: gl.base.sender_role_id,
                display_name: gl.base.display_name.clone(),
                original_emotion: gl.base.original_emotion.clone(),
                predicted_emotion: gl.base.predicted_emotion.clone(),
                action_content: gl.base.action_content.clone(),
                audio_file: gl.base.audio_file.clone(),
                perceived_role_ids: gl.perceived_role_ids.clone(),
                user_message_seq: seq,
            })
            .collect();

        let mut sid = gs.current_scene_id.clone();

        // 若无当前场景，尝试从 store 恢复上次选择的场景
        if sid.is_none() {
            if let Ok(store) = app.store(config::STORE_FILE) {
                if let Some(v) = store.get(config::keys::LAST_SCENE_ID) {
                    if let Some(id) = v.as_str() {
                        sid = Some(id.to_string());
                    }
                }
            }
        }

        // 若仍无场景，随机选一个
        if sid.is_none() {
            let store = SceneStore::new(&service.data_dir);
            if let Ok(scenes) = store.load_all() {
                if !scenes.is_empty() {
                    let idx = chrono::Utc::now().timestamp_subsec_nanos() as usize % scenes.len();
                    sid = Some(scenes[idx].id.clone());
                }
            }
        }

        // 若恢复了场景，更新 GameStatus
        if sid != gs.current_scene_id {
            gs.current_scene_id = sid.clone();
            if let Some(ref scene_id) = sid {
                let store = SceneStore::new(&service.data_dir);
                if let Ok(Some(scene)) = store.find_by_id(scene_id) {
                    let bg = super::scene::normalize_background(&scene.background);
                    if !bg.is_empty() {
                        gs.background = bg;
                    }
                }
            }
        }

        // 从 store 恢复场景感知开关
        if let Ok(store) = app.store(config::STORE_FILE) {
            if let Some(v) = store.get(config::keys::SCENE_AWARENESS_ENABLED) {
                gs.scene_awareness_enabled = v.as_bool().unwrap_or(true);
            }
        }
        let scene_awareness = gs.scene_awareness_enabled;

        // 收集在场角色的设定信息，供前端初始化 gameRoles / presentRoleIds
        let onstage_roles: Vec<CharacterSettingsInit> = gs
            .onstage_role_ids
            .iter()
            .filter_map(|&id| {
                gs.role_manager
                    .get_loaded(id)
                    .map(|r| CharacterSettingsInit::from(&r.settings))
            })
            .collect();

        (
            lines,
            sid,
            gs.current_role_id,
            gs.onstage_role_ids.clone(),
            onstage_roles,
            gs.background.clone(),
            gs.background_effect.clone(),
            gs.background_music.clone(),
            scene_awareness,
        )
    };

    // Resolve scene info from SceneStore
    let current_scene = if let Some(ref sid) = current_scene_id {
        let store = SceneStore::new(&service.data_dir);
        store
            .find_by_id(sid)
            .ok()
            .flatten()
            .map(|s| super::scene::SceneInfo {
                id: s.id,
                scene_name: s.name,
                scene_description: s.description,
                background: {
                    let bg = super::scene::normalize_background(&s.background);
                    if bg.is_empty() {
                        None
                    } else {
                        Some(bg)
                    }
                },
                lighting: s.lighting.clone(),
                created_at: s.created_at,
                updated_at: s.updated_at,
            })
    } else {
        None
    };

    let result = WebInitData {
        character_settings,
        current_interact_role_id: current_role_id,
        onstage_roles_ids,
        onstage_roles,
        background,
        background_effect,
        background_music,
        current_scene_id,
        current_scene,
        lines,
        scene_awareness_enabled,
    };
    Ok(result)
}

// ============================================================
// 多人对话：将角色加入场景
// ============================================================

#[tauri::command]
pub async fn add_role_to_scene(app: AppHandle, role_id: i32) -> Result<JsonValue, String> {
    if role_id == 0 {
        return Err("无法添加玩家角色 (role_id=0)".to_string());
    }

    let state = app.state::<AppState>();
    let db = &state.db;

    // 提前加载配置（PromptOptions 在 Phase 1 和 Phase 2 之间共享）
    let app_config = AppConfig::load(&app).unwrap_or_default();
    let prompt_options = PromptOptions {
        output_sec_lang: app_config.llm_output_sec_lang,
        no_emotion_limit: app_config.no_emotion_limit_prompt,
    };

    // Phase 1: 加载角色 → 注入 System prompt（在 onstage_role 之前） → 上台 → 刷新记忆
    let role_name = {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        // 剧本模式下不允许手动添加
        if gs.script_status.is_some() {
            return Err("剧本模式下无法手动添加角色到场景".to_string());
        }

        // 已在场
        if gs.present_role_ids.contains(&role_id) {
            return Ok(serde_json::json!({"success": false, "message": "角色已在场景中"}));
        }

        // 确保角色已加载到 role_manager
        gs.get_role(db, role_id)
            .await
            .map_err(|e| format!("加载角色失败: {}", e))?;

        // 获取角色信息用于 System prompt 和 display_name
        let role = gs
            .role_manager
            .get_loaded(role_id)
            .ok_or_else(|| "角色未加载".to_string())?;
        let name = role
            .display_name
            .clone()
            .unwrap_or_else(|| format!("角色{}", role_id));

        // 构建角色的 system prompt
        let system_prompt = sys_prompt_builder_by_settings(&role.settings, prompt_options);

        // ★ 注入 System 行必须在 onstage_role 之前。
        //    仅当台词表中不存在本角色的 System 行时才添加（避免退出后重入时重复）。
        let already_has_system = gs.line_list.iter().any(|l| {
            matches!(l.attribute(), LineAttribute::System) && l.base.sender_role_id == Some(role_id)
        });
        if !already_has_system {
            gs.add_line(
                db,
                LineBase {
                    content: system_prompt,
                    attribute: LineAttributeExt(LineAttribute::System),
                    sender_role_id: Some(role_id),
                    display_name: Some(name.clone()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("添加角色 system prompt 失败: {}", e))?;
        } else {
            tracing::info!("角色 {} 已有 system prompt，跳过重复注入", role_id);
        }

        // 上台 + 刷新记忆（让新角色感知后续台词）
        gs.onstage_role(role_id);
        gs.refresh_memories(db)
            .await
            .map_err(|e| format!("刷新记忆失败: {}", e))?;

        tracing::info!("角色 {} ({}) 加入场景", role_id, name);
        name
    }; // 释放 GameStatus 锁

    // Phase 2: 添加旁白台词
    {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        let prompt = PromptRole::Narrator.build_prompt(&format!("{}加入了对话", role_name));
        gs.add_line(
            db,
            LineBase {
                content: prompt,
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("系统".to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("添加系统台词失败: {}", e))?;
    }

    Ok(serde_json::json!({"success": true, "message": format!("{} 已加入对话", role_name)}))
}

// ============================================================
// 多人对话：将角色移出场景
// ============================================================

#[tauri::command]
pub async fn remove_role_from_scene(app: AppHandle, role_id: i32) -> Result<JsonValue, String> {
    if role_id == 0 {
        return Err("无法移除玩家角色 (role_id=0)".to_string());
    }

    let state = app.state::<AppState>();
    let db = &state.db;

    let (role_name, switched_to) = {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        // 剧本模式下不允许手动移除
        if gs.script_status.is_some() {
            return Err("剧本模式下无法手动让角色退场".to_string());
        }

        // 主角不可退场
        if gs.main_role_id == Some(role_id) {
            return Err("无法移除主角".to_string());
        }

        // 不在场
        if !gs.present_role_ids.contains(&role_id) {
            return Ok(serde_json::json!({"success": false, "message": "角色不在场景中"}));
        }

        let name = gs
            .role_manager
            .get_loaded(role_id)
            .and_then(|r| r.display_name.clone())
            .unwrap_or_else(|| format!("角色{}", role_id));

        // 从舞台和在场集合中移除
        gs.offstage_role(role_id);

        // 如果退场角色是当前说话者，切回主角（避免后续对话指向已退场角色）
        let mut switched_to: Option<(i32, String)> = None;
        if gs.current_role_id == Some(role_id) {
            gs.current_role_id = gs.main_role_id;
            // 收集主角信息，用于 lock 外 emit character:switch
            if let Some(main_id) = gs.main_role_id {
                let main_name = gs
                    .role_manager
                    .get_loaded(main_id)
                    .and_then(|r| r.display_name.clone())
                    .unwrap_or_else(|| format!("角色{}", main_id));
                switched_to = Some((main_id, main_name));
            }
        }

        gs.refresh_memories(db)
            .await
            .map_err(|e| format!("刷新记忆失败: {}", e))?;

        tracing::info!("角色 {} ({}) 退出场景", role_id, name);
        (name, switched_to)
    }; // 释放 GameStatus 锁

    // 若切换了 current_role_id，通知前端
    if let Some((target_id, target_name)) = switched_to {
        let payload = serde_json::json!({
            "type": "character_switch",
            "roleId": target_id,
            "characterName": target_name,
        });
        if let Err(e) = app.emit("character:switch", &payload) {
            tracing::warn!("emit character:switch 失败: {e}");
        }
    }

    // 添加退场旁白
    {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        let prompt = PromptRole::Narrator.build_prompt(&format!("{}离开了对话", role_name));
        gs.add_line(
            db,
            LineBase {
                content: prompt,
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("系统".to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("添加系统台词失败: {}", e))?;
    }

    Ok(serde_json::json!({"success": true, "message": format!("{} 已离开对话", role_name)}))
}

// ============================================================
// 玩家入场问候
// ============================================================

/// 玩家进入主界面时调用（仅会话内首次有效）。
///
/// 根据 `active_save_id` 判断是首次见面还是回归：
/// - 无存档 → 旁白："{AI名称}看到{玩家名称}过来了"
/// - 有存档 → 旁白："{玩家名称}回来了"
///
/// 添加台词后自动触发一轮 AI 生成（无需用户输入），
/// 且不 emit `ai:thinking` 事件（入场问候为后台触发）。
#[tauri::command]
pub async fn notify_player_entry(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Phase 1: 去重 & 添加旁白台词
    {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        if gs.player_entered {
            return Ok(());
        }
        gs.player_entered = true;

        let current_role_id = match gs.current_role_id {
            Some(id) => id,
            None => {
                tracing::info!("[Entry] 没有当前角色，跳过问候");
                return Ok(());
            }
        };

        let ai_name = gs
            .role_manager
            .get_loaded(current_role_id)
            .and_then(|r| r.display_name.clone())
            .unwrap_or_else(|| format!("角色{}", current_role_id));

        let player_name = if gs.player.user_name.is_empty() {
            "玩家".to_string()
        } else {
            gs.player.user_name.clone()
        };

        let greeting = if gs.active_save_id.is_some() {
            format!("{}回来了", player_name)
        } else {
            format!("{}看到{}过来了", ai_name, player_name)
        };

        let time_info = chrono::Local::now()
            .format("（现在是%_m月%_d日，%H:%M）")
            .to_string()
            .replace(' ', "");
        let greeting_with_time = format!("{}，{}", greeting, time_info);

        let prompt = PromptRole::Narrator.build_prompt(&greeting_with_time);
        gs.add_line(
            &state.db,
            LineBase {
                content: prompt,
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("旁白".to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("添加入场问候台词失败: {}", e))?;

        tracing::info!("[Entry] 已添加问候台词: {}", greeting);
    } // 释放锁

    // Phase 2: 触发 AI 响应（suppress_thinking=true，不显示思考指示器）
    let llm = state
        .chat
        .llm
        .clone()
        .ok_or_else(|| "LLM 未配置".to_string())?;
    let concurrency = AppConfig::load(&app)
        .map(|c| c.consumers as usize)
        .unwrap_or(1)
        .max(1);
    let game_status = {
        let svc = state.ai_service.lock().await;
        svc.game_status.clone()
    };

    let deps = GeneratorDeps {
        app: app.clone(),
        db: state.db.clone(),
        game_status,
        processor: state.chat.processor.clone(),
        translator: state.chat.translator.clone(),
        llm,
        concurrency,
        god_agent: state.god_agent.clone(),
        suppress_thinking: true,
    };

    let generator = MessageGenerator::new(deps);
    let gen_lock = state.generation_lock.clone();

    tokio::spawn(async move {
        let _lock = gen_lock.lock().await;
        match generator.process_message(None).await {
            Ok(acc) => tracing::info!("[Entry] 入场问候生成完成，长度: {}", acc.len()),
            Err(e) => tracing::error!("[Entry] 入场问候生成失败: {:#}", e),
        }
    });

    Ok(())
}
