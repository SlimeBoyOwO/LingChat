//! Tauri IPC commands for script/story mode.
//!
//! Replaces Python's WebSocket-based script communication.
//! Frontend calls these via `invoke()` instead of `/v1/chat/script/*` HTTP endpoints.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use crate::ai_service::config::AIServiceConfig;
use crate::ai_service::game_system::script_engine::events::{
    create_event, ScriptContext, SharedScriptChannels,
};
use crate::AppState;

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
pub async fn start_script(
    app: AppHandle,
    script_name: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Clone shared handles for the background task
    let ai_service = state.ai_service.clone();
    let channels = state.script_channels.clone();
    let db = state.db.clone();
    let data_dir = state.ai_service.lock().await.data_dir.clone();
    let llm = state.chat.llm.clone();
    let achievement_manager = state.achievement_manager.clone();

    // Check script exists
    {
        let service = ai_service.lock().await;
        if !service.script_manager.all_scripts.contains_key(&script_name) {
            return Err(format!("剧本不存在: '{}'", script_name));
        }
    }

    // Run script in background task (does NOT hold AIService lock across awaits)
    tokio::spawn(async move {
        match run_script_background(
            ai_service,
            channels,
            db,
            data_dir,
            app,
            llm,
            script_name,
            achievement_manager,
        )
        .await
        {
            Ok(()) => tracing::info!("[ScriptAPI] 剧本执行完成"),
            Err(e) => tracing::error!("[ScriptAPI] 剧本执行错误: {}", e),
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn script_submit_input(
    app: AppHandle,
    input: String,
) -> Result<(), String> {
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
pub async fn script_submit_choice(
    app: AppHandle,
    choice: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    if let Some(tx) = channels.choice_tx.take() {
        let _ = tx.send(choice);
        Ok(())
    } else {
        Err("当前没有等待选择的脚本事件".to_string())
    }
}

// ============================================================
// Background script execution
// ============================================================

/// Run a script in a background task.
/// Acquires and releases the AIService lock around each event,
/// so input/choice commands don't deadlock.
pub(crate) async fn run_script_background(
    ai_service: crate::ai_service::service::SharedAIService,
    channels: SharedScriptChannels,
    db: sea_orm::DatabaseConnection,
    data_dir: std::path::PathBuf,
    app: AppHandle,
    llm: Option<Arc<crate::ai_service::llm::LlmClient>>,
    script_name: String,
    achievement_manager: std::sync::Arc<tokio::sync::Mutex<crate::achievements::manager::AchievementManager>>,
) -> anyhow::Result<()> {
    // Acquire lock briefly to get script data and initialize
    let mut service = ai_service.lock().await;
    let config = service.config.clone();

    let script = service
        .script_manager
        .all_scripts
        .get(&script_name)
        .ok_or_else(|| anyhow::anyhow!("剧本不存在: '{}'", script_name))?
        .clone();

    service.game_status.lock().await.script_status = Some(script.clone());
    service.script_manager.is_running = true;

    // Load player info
    if let Some(user_name) = script.settings.get("user_name").and_then(|v| v.as_str()) {
        if !user_name.is_empty() {
            service.game_status.lock().await.player.user_name = user_name.to_string();
        }
    }

    // Register script roles (if characters/ exists)
    {
        let path_key = script.path_key();
        let characters_dir = script.script_path.join("characters");
        if characters_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&characters_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let role_folder = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Check if role exists
                    let existing = crate::db::managers::role_repo::RoleRepo::get_role_by_script_keys(
                        &db, &path_key, &role_folder,
                    )
                    .await?;

                    if existing.is_none() {
                        let settings_path = path.join("settings.yml");
                        if settings_path.exists() {
                            let content = std::fs::read_to_string(&settings_path)?;
                            if let Ok(settings) = serde_yaml::from_str::<crate::ai_service::types::CharacterSettings>(&content) {
                                use crate::db::entities::role::RoleType;
                                let role_id = crate::db::managers::role_repo::RoleRepo::find_or_create_role(
                                    &db,
                                    &settings.ai_name,
                                    RoleType::Npc,
                                    Some(&path_key),
                                    Some(&role_folder),
                                )
                                .await?;

                                let _ = service.game_status.lock().await.get_role(&db, role_id).await?;

                                if let Some(prompt) = settings.system_prompt {
                                    if !prompt.is_empty() {
                                        let sys_line = crate::ai_service::types::LineBase {
                                            content: prompt,
                                            attribute: crate::ai_service::types::LineAttributeExt(
                                                crate::db::entities::line::LineAttribute::System,
                                            ),
                                            sender_role_id: Some(role_id),
                                            display_name: Some(settings.ai_name),
                                            ..Default::default()
                                        };
                                        service.game_status.lock().await.add_line(&db, sys_line).await?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let intro_chapter = script.intro_chapter.clone();
    let script_path = script.script_path.clone();

    // Drop lock before entering the main loop
    drop(service);

    // Chapter loop
    let chapters_dir = script_path.join("Chapters");
    let mut next_chapter = intro_chapter;

    while next_chapter != "end" {
        // Read chapter YAML
        let chapter_path = if next_chapter.ends_with(".yaml") {
            chapters_dir.join(&next_chapter)
        } else {
            chapters_dir.join(format!("{}.yaml", next_chapter))
        };

        let content = std::fs::read_to_string(&chapter_path)
            .map_err(|e| anyhow::anyhow!("无法读取章节文件 {:?}: {}", chapter_path, e))?;

        let chapter_config: serde_json::Value = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("无法解析章节文件 {:?}: {}", chapter_path, e))?;

        let chapter_name = chapter_config
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&next_chapter)
            .to_string();

        let event_list: Vec<serde_json::Value> = chapter_config
            .get("events")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Emit chapter_change
        {
            use crate::ai_service::game_system::script_engine::responses::{
                event_names::SCRIPT_CHAPTER_CHANGE, ChapterChangePayload,
            };
            use crate::ai_service::message_system::events::emit;
            let _ = emit(&app, SCRIPT_CHAPTER_CHANGE, &ChapterChangePayload {
                chapter_name: chapter_name.clone(),
            });
        }

        // Process events one by one
        let mut progress = 0usize;
        let mut chapter_result: Option<String> = None;

        while chapter_result.is_none() && progress < event_list.len() {
            let event_data = event_list[progress].clone();
            progress += 1;

            let event_type = event_data
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("事件缺少 type 字段"))
                .map(|s| s.to_string())?;

            // Check condition
            let condition_met = {
                let service = ai_service.lock().await;
                let gs = service.game_status.lock().await;
                check_event_condition(&event_data, &gs)
            };

            if !condition_met {
                tracing::info!("[ScriptAPI] 跳过事件 type='{}'（条件不满足）", event_type);
                continue;
            }

            // Resolve placeholders
            let event_data = {
                let service = ai_service.lock().await;
                let gs = service.game_status.lock().await;
                resolve_event_placeholders(event_data, &gs)
            };

            // Execute the event
            let handler_result = execute_script_event(
                &ai_service,
                &channels,
                &db,
                &data_dir,
                &app,
                llm.as_ref(),
                &config,
                &event_type,
                event_data,
            )
            .await;

            match handler_result {
                Ok(result) => {
                    if let Some(next) = result {
                        chapter_result = Some(next);
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "[ScriptAPI] 事件执行错误 type='{}': {}",
                        event_type,
                        e
                    );
                    use crate::ai_service::message_system::events::emit;
                    use crate::ai_service::message_system::responses::ErrorResponse;
                    let _ = emit(
                        &app,
                        "ai:error",
                        &ErrorResponse::new("script_event_error", e.to_string()),
                    );
                    break;
                }
            }

            // Update current_event_process
            {
                let service = ai_service.lock().await;
                let mut gs = service.game_status.lock().await;
                if let Some(ref mut ss) = gs.script_status {
                    ss.current_event_process = progress as i32;
                }
            }
        }

        next_chapter = chapter_result.unwrap_or_else(|| "end".to_string());

        // Update current_chapter_key
        {
            let service = ai_service.lock().await;
            let mut gs = service.game_status.lock().await;
            if let Some(ref mut ss) = gs.script_status {
                ss.current_chapter_key = next_chapter.clone();
            }
        }
    }

    // Script ended — extract completion data, then cleanup
    {
        let mut service = ai_service.lock().await;

        // Extract everything we need from script_status BEFORE clearing it
        let (completed_key, adventure_info) = {
            let gs = service.game_status.lock().await;
            let key = gs.script_status.as_ref().map(|ss| ss.path_key());
            let info = gs.script_status.as_ref().map(|ss| {
                (
                    ss.folder_key.clone(),
                    ss.adventure.completion_achievements.clone(),
                    ss.name.clone(),
                    ss.adventure.is_adventure,
                )
            });
            (key, info)
        };

        // Mark script as completed in-memory
        if let Some(key) = completed_key {
            service.game_status.lock().await.completed_scripts.insert(key);
        }

        // Now safe to clear script_status
        service.game_status.lock().await.script_status = None;
        service.script_manager.is_running = false;

        // Emit script end event
        {
            use crate::ai_service::game_system::script_engine::responses::{
                event_names::SCRIPT_END, ScriptEndPayload,
            };
            use crate::ai_service::message_system::events::emit;
            let _ = emit(&app, SCRIPT_END, &ScriptEndPayload {});
        }

        drop(service);

        // Handle adventure completion (achievements, chained unlocks)
        if let Some((folder_key, completion_achievements, name, true)) = adventure_info {
            super::adventure::handle_adventure_completion(
                &db,
                &achievement_manager,
                &app,
                &ai_service,
                &folder_key,
                &completion_achievements,
                &name,
            )
            .await;
        }
    }

    tracing::info!("[ScriptAPI] 剧本 '{}' 执行完成", script_name);
    Ok(())
}

/// Execute a single script event with proper lock management.
/// Clones the GameStatus Arc briefly while holding AIService lock,
/// then drops AIService lock so events can safely call MessageGenerator.
async fn execute_script_event(
    ai_service: &crate::ai_service::service::SharedAIService,
    channels: &SharedScriptChannels,
    db: &sea_orm::DatabaseConnection,
    data_dir: &std::path::Path,
    app: &AppHandle,
    llm: Option<&Arc<crate::ai_service::llm::LlmClient>>,
    config: &AIServiceConfig,
    event_type: &str,
    event_data: serde_json::Value,
) -> anyhow::Result<Option<String>> {
    let mut handler = create_event(event_type, event_data.clone()).ok_or_else(|| {
        anyhow::anyhow!("未注册的事件类型: '{}'", event_type)
    })?;

    // Clone game_status Arc while holding AIService lock, then release.
    // Events hold their own Arc<Mutex<GameStatus>> and can call
    // MessageGenerator without re-locking AIService → no deadlock.
    let game_status = {
        let service = ai_service.lock().await;
        service.game_status.clone()
    };

    let mut ctx = make_script_context(
        game_status, db, data_dir, app, config, llm,
        channels.clone(),
    );
    handler.execute(&mut ctx).await
}

fn make_script_context<'a>(
    game_status: Arc<tokio::sync::Mutex<crate::ai_service::game_system::game_status::GameStatus>>,
    db: &'a sea_orm::DatabaseConnection,
    data_dir: &'a std::path::Path,
    app: &'a AppHandle,
    config: &'a AIServiceConfig,
    llm: Option<&'a Arc<crate::ai_service::llm::LlmClient>>,
    channels: SharedScriptChannels,
) -> ScriptContext<'a> {
    ScriptContext {
        db,
        data_dir,
        app,
        game_status,
        config,
        llm,
        channels,
    }
}

/// Check the optional `condition` field on an event dict.
fn check_event_condition(
    event_data: &serde_json::Value,
    game_status: &crate::ai_service::game_system::game_status::GameStatus,
) -> bool {
    let condition = event_data
        .get("condition")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if condition.is_empty() {
        return true;
    }
    if let Some(ref ss) = game_status.script_status {
        crate::ai_service::game_system::script_engine::events::evaluate_condition(
            condition,
            &ss.vars,
        )
    } else {
        true
    }
}

/// Pre-process event data: replace `%player%` in text fields.
fn resolve_event_placeholders(
    mut event_data: serde_json::Value,
    game_status: &crate::ai_service::game_system::game_status::GameStatus,
) -> serde_json::Value {
    use crate::ai_service::game_system::script_engine::utils::script_function::replace_placeholder;
    if let serde_json::Value::Object(ref mut map) = event_data {
        let text_fields = [
            "text", "prompt", "hint", "end_line", "dialog_prompt", "end_prompt",
            "content", "description",
        ];
        for field in &text_fields {
            if let Some(serde_json::Value::String(s)) = map.get_mut(*field) {
                *s = replace_placeholder(s, game_status);
            }
        }
    }
    event_data
}
