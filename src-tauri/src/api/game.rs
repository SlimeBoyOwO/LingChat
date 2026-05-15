use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::ai_service::prompt::PromptOptions;
use crate::ai_service::types::CharacterSettings;
use crate::config::{self, AppConfig};
use crate::db::managers::role_repo::RoleRepo;
use crate::init::static_copy;
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
    /// 初始化台词列表（至少包含一条 system 人设台词）
    pub lines: Vec<GameLineInit>,
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
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub async fn reactivate_tts(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    service.game_status.reactivate_all_voice_makers();
    log::info!("TTS 服务已通过 reactivate_tts 命令重新启用");
    Ok(())
}

#[tauri::command]
pub async fn init_game(app: AppHandle) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    build_web_init_data(&service)
}

// ========== 角色切换 ==========

#[tauri::command]
pub async fn select_character(app: AppHandle, character_id: i32) -> Result<WebInitData, String> {
    let data_dir = static_copy::resolve_data_dir();

    // 1. 从 DB 加载角色设定
    let state = app.state::<AppState>();
    let db = &state.db;

    let settings = RoleRepo::get_role_settings_by_id(db, &data_dir, character_id)
        .await
        .map_err(|e| format!("查询角色配置失败: {}", e))?
        .unwrap_or_else(|| {
            log::warn!("角色 {} 无配置文件，使用默认设定", character_id);
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
        service.import_settings(settings.clone(), prompt_options);
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

    log::info!(
        "切换角色成功: id={}, name={}",
        character_id,
        settings.ai_name
    );

    // 5. 返回最新游戏状态（复用 init_game 逻辑）
    //    drop 后再拿锁，避免同一个锁两次借用
    let init = {
        let service = state.ai_service.lock().await;
        build_web_init_data(&service)?
    };
    Ok(init)
}

/// 从 AIService 快照构建 WebInitData（不持锁的函数）
fn build_web_init_data(service: &crate::ai_service::service::AIService) -> Result<WebInitData, String> {
    let settings = service
        .settings
        .as_ref()
        .ok_or_else(|| "AI 服务尚未初始化角色设定".to_string())?;

    let character_settings = CharacterSettingsInit::from(settings);

    let lines: Vec<GameLineInit> = service
        .game_status
        .line_list
        .iter()
        .map(|gl| GameLineInit {
            content: gl.base.content.clone(),
            attribute: gl.base.attribute.as_str().to_string(),
            sender_role_id: gl.base.sender_role_id,
            display_name: gl.base.display_name.clone(),
            original_emotion: gl.base.original_emotion.clone(),
            predicted_emotion: gl.base.predicted_emotion.clone(),
            action_content: gl.base.action_content.clone(),
            audio_file: gl.base.audio_file.clone(),
            perceived_role_ids: gl.perceived_role_ids.clone(),
        })
        .collect();

    Ok(WebInitData {
        character_settings,
        current_interact_role_id: service.game_status.current_role_id,
        onstage_roles_ids: service.game_status.onstage_role_ids.clone(),
        background: service.game_status.background.clone(),
        background_effect: service.game_status.background_effect.clone(),
        background_music: service.game_status.background_music.clone(),
        lines,
    })
}
