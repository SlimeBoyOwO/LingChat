//! Tauri IPC commands for script/story mode.
//!
//! Replaces Python's WebSocket-based script communication.
//! Frontend calls these via `invoke()` instead of `/v1/chat/script/*` HTTP endpoints.

use crate::AppState;
use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::ai_service::game_system::script_engine::events::ScriptContext;
use crate::ai_service::types::PLAYER_ROLE_ID;
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
    /// 来源："game" 或提供该剧本的插件 id。
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

fn summary_of(s: &crate::ai_service::types::ScriptStatus) -> ScriptSummary {
    ScriptSummary {
        script_name: s.name.clone(),
        description: s.description.clone(),
        folder_key: s.folder_key.clone(),
        intro_chapter: s.intro_chapter.clone(),
        source: s.plugin_id.clone().unwrap_or_else(|| "game".to_string()),
        plugin_id: s.plugin_id.clone(),
    }
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
        .map(summary_of)
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
        .map(summary_of)
        .collect();

    Ok(ScriptListResponse { scripts })
}

/// 剧本/冒险启动前的互斥校验（命令层第一道闸）。
///
/// 附身态启动会让玩家身份与剧本场次互相污染；已有 run 在跑时重复启动会让两个
/// run 争抢同一份 `script_status` 与输入通道，前端也收不到明确的结束信号。
/// 注意：读档续跑（`api/save.rs`）直连 `spawn_script_execution` 恢复被中止的引擎，
/// 不复用本校验——`is_running` 会因为任务被 abort 而残留 true。
pub(crate) async fn ensure_script_start_allowed(
    app: &AppHandle,
    kind: &str,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;

    // 带附身启动会让玩家身份与剧本场次互相污染，先要求解除扮演
    {
        let gs = service.game_status.lock().await;
        if gs.possessed_role_id != PLAYER_ROLE_ID {
            return Err(format!("请先解除扮演（切回默认身份）后再开始{kind}"));
        }
    }

    // 同一时刻只允许一个剧本/冒险占用引擎
    if service
        .script_manager
        .is_running
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("已有剧本或冒险正在进行中".to_string());
    }

    Ok(())
}

/// 在后台任务中执行剧本（含羁绊完成处理），并把任务句柄登记到 AppState。
/// 新引擎起跑前先中止上一个登记的引擎任务并清理其输入通道——
/// 阻塞中的旧引擎靠 oneshot 挂起，不中止就会往新会话的共享 GameStatus 里写台词。
pub(crate) async fn spawn_script_execution(
    app: AppHandle,
    script: crate::ai_service::types::ScriptStatus,
) {
    let state = app.state::<AppState>();

    if let Some(handle) = state.script_task.lock().await.take() {
        handle.abort();
    }
    {
        let mut ch = state.script_channels.lock().await;
        let _ = ch.input_tx.take();
        let _ = ch.choice_tx.take();
        ch.choice_allow_free = false;
    }

    // Clone shared handles for the background task
    let ai_service = state.ai_service.clone();
    let channels = state.script_channels.clone();
    let db = state.db.clone();
    let data_dir = state.ai_service.lock().await.data_dir.clone();
    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;
    let achievement_manager = state.achievement_manager.clone();

    // Lock AIService briefly to extract needed data
    let (game_status, config, is_running) = {
        let service = ai_service.lock().await;
        (
            service.game_status.clone(),
            service.config.clone(),
            service.script_manager.is_running.clone(),
        )
    };

    // `app` 仍需被 `state` 借用（登记任务句柄要用），后台任务持有一份克隆
    let task_app = app.clone();
    let handle = tokio::spawn(async move {
        let mut ctx = ScriptContext {
            db: &db,
            data_dir: &data_dir,
            app: &task_app,
            game_status,
            config: &config,
            llm: llm.as_ref(),
            channels,
            is_preview: false,
        };

        match ScriptManager::execute_script(&script, &mut ctx, &is_running).await {
            Ok(()) => {
                // Handle adventure completion (achievements, chained unlocks)
                if script.adventure.is_adventure {
                    super::adventure::handle_adventure_completion(
                        &db,
                        &achievement_manager,
                        &task_app,
                        &ai_service,
                        &script.folder_key,
                        &script.adventure.completion_achievements,
                        &script.name,
                    )
                    .await;
                }
                tracing::info!("[ScriptAPI] 剧本执行完成")
            },
            Err(e) => tracing::error!("[ScriptAPI] 剧本执行错误: {}", e),
        }
    });

    *state.script_task.lock().await = Some(handle);
}

#[tauri::command]
pub async fn start_script(app: AppHandle, script_name: String) -> Result<(), String> {
    // 先过附身/并发互斥校验：被拒时命令返回 Err，前端不会进入剧本模式
    ensure_script_start_allowed(&app, "剧本").await?;

    let script = {
        let state = app.state::<AppState>();
        let service = state.ai_service.lock().await;
        service
            .script_manager
            .all_scripts
            .get(&script_name)
            .ok_or_else(|| format!("剧本不存在: '{}'", script_name))?
            .clone()
    };

    spawn_script_execution(app, script).await;

    Ok(())
}

#[tauri::command]
pub async fn script_submit_input(app: AppHandle, input: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;

    if let Some(tx) = channels.input_tx.take() {
        let _ = tx.send(input);
        return Ok(());
    }

    // No `input` event pending. If a `choices` event with `allow_free: true` is
    // waiting, the user typing into the dialogue box *is* their choice — route it
    // to the choice channel. Previously this returned Err, the frontend only
    // logged it, and the script blocked on `choice_tx` forever.
    if channels.choice_allow_free {
        if let Some(tx) = channels.choice_tx.take() {
            channels.choice_allow_free = false;
            let _ = tx.send(input);
            return Ok(());
        }
    }

    if channels.choice_tx.is_some() {
        // A choice is pending but does not accept free input. Reject without
        // consuming the sender so the option buttons stay usable.
        return Err("当前的选项不接受自由输入，请点击一个选项".to_string());
    }

    Err("当前没有等待输入的脚本事件".to_string())
}

#[tauri::command]
pub async fn script_submit_choice(app: AppHandle, choice: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    if let Some(tx) = channels.choice_tx.take() {
        channels.choice_allow_free = false;
        let _ = tx.send(choice);
        Ok(())
    } else {
        Err("当前没有等待选择的脚本事件".to_string())
    }
}
