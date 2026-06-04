//! Tauri IPC commands for script/story mode.
//!
//! Replaces Python's WebSocket-based script communication.
//! Frontend calls these via `invoke()` instead of `/v1/chat/script/*` HTTP endpoints.

use crate::ai_service::game_system::script_engine::events::ScriptContext;
use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, Manager};

// ============================================================
// Response types
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ScriptSummary {
    pub script_name: String,
    pub description: String,
    pub folder_key: String,
    pub intro_chapter: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ScriptListResponse {
    pub scripts: Vec<ScriptSummary>,
}

// ============================================================
// Tauri commands
// ============================================================

#[tauri::command]
pub async fn list_scripts(app: AppHandle) -> Result<ScriptListResponse, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let scripts: Vec<ScriptSummary> = service
        .script_manager
        .all_scripts
        .values()
        .map(|s| ScriptSummary {
            script_name: s.name.clone(),
            description: s.description.clone(),
            folder_key: s.folder_key.clone(),
            intro_chapter: s.intro_chapter.clone(),
        })
        .collect();

    Ok(ScriptListResponse { scripts })
}

#[tauri::command]
pub async fn list_standalone_scripts(app: AppHandle) -> Result<ScriptListResponse, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let scripts: Vec<ScriptSummary> = service
        .script_manager
        .all_scripts
        .values()
        .filter(|s| !s.adventure.is_adventure)
        .map(|s| ScriptSummary {
            script_name: s.name.clone(),
            description: s.description.clone(),
            folder_key: s.folder_key.clone(),
            intro_chapter: s.intro_chapter.clone(),
        })
        .collect();

    Ok(ScriptListResponse { scripts })
}

#[tauri::command]
pub async fn start_script(app: AppHandle, script_name: String) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Clone shared handles for the background task
    let ai_service = state.ai_service.clone();
    let channels = state.script_channels.clone();
    let db = state.db.clone();
    let data_dir = state.ai_service.lock().await.data_dir.clone();
    let llm = state.chat.llm.clone();
    let achievement_manager = state.achievement_manager.clone();

    // Lock AIService briefly to validate and extract needed data
    let (script, game_status, config, is_running) = {
        let service = ai_service.lock().await;
        let script = service
            .script_manager
            .all_scripts
            .get(&script_name)
            .ok_or_else(|| format!("剧本不存在: '{}'", script_name))?
            .clone();
        let game_status = service.game_status.clone();
        let config = service.config.clone();
        let is_running = service.script_manager.is_running.clone();
        (script, game_status, config, is_running)
    };

    // Run script in background task (does NOT hold AIService lock across awaits)
    tokio::spawn(async move {
        let mut ctx = ScriptContext {
            db: &db,
            data_dir: &data_dir,
            app: &app,
            game_status,
            config: &config,
            llm: llm.as_ref(),
            channels,
        };

        match ScriptManager::execute_script(&script, &mut ctx, &is_running).await {
            Ok(()) => {
                // Handle adventure completion (achievements, chained unlocks)
                if script.adventure.is_adventure {
                    super::adventure::handle_adventure_completion(
                        &db,
                        &achievement_manager,
                        &app,
                        &ai_service,
                        &script.folder_key,
                        &script.adventure.completion_achievements,
                        &script.name,
                    )
                    .await;
                }
                tracing::info!("[ScriptAPI] 剧本执行完成")
            }
            Err(e) => tracing::error!("[ScriptAPI] 剧本执行错误: {}", e),
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn script_submit_input(app: AppHandle, input: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    if let Some(tx) = channels.input_tx.take() {
        let _ = tx.send(input);
        Ok(())
    } else {
        Err("当前没有等待输入的脚本事件".to_string())
    }
}

#[tauri::command]
pub async fn script_submit_choice(app: AppHandle, choice: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    if let Some(tx) = channels.choice_tx.take() {
        let _ = tx.send(choice);
        Ok(())
    } else {
        Err("当前没有等待选择的脚本事件".to_string())
    }
}
