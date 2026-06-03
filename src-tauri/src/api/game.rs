use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::types::{CharacterSettings, GameLine};
use crate::db::entities::line::LineAttribute;
use crate::config::{self, AppConfig};
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;
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

        (
            lines,
            sid,
            gs.current_role_id,
            gs.onstage_role_ids.clone(),
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
