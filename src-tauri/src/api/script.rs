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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_warning: Option<String>,
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
            content_warning: s.content_warning.clone(),
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
            content_warning: s.content_warning.clone(),
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
    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;
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
            is_preview: false,
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

/// Clear one script's persisted runtime state (playthrough memory), so the
/// next entry starts from the first-run route again. Refused while any script
/// is still running to avoid yanking state out from under a live run.
#[tauri::command]
pub async fn reset_script_state(app: AppHandle, script_name: String) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    if service
        .script_manager
        .is_running
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("剧本正在运行，请先退出再重置记忆".to_string());
    }
    let script = service
        .script_manager
        .all_scripts
        .get(&script_name)
        .ok_or_else(|| format!("剧本不存在: '{}'", script_name))?;
    crate::ai_service::game_system::script_engine::persistent_state::reset_playthrough(
        &service.data_dir,
        &script.path_key(),
    )
    .map_err(|e| format!("重置剧本记忆失败: {:#}", e))
}

/// Stop a running script mid-way (user picked 自由对话 from the menu, cleared
/// the conversation, etc.). There is no shutdown channel: the script task is
/// typically blocked on a oneshot input/choice receiver, so dropping the
/// senders makes it error out and run its normal teardown (`on_script_end`
/// with completed=false → `script:end` → frontend cleanup + history rollback).
/// Waits briefly for `is_running` to flip so an immediate re-entry does not
/// race the old run's teardown; on timeout the old task still finishes its
/// teardown later (e.g. it may be mid-LLM-roundtrip), the frontend has
/// already cleaned up its own state by then.
#[tauri::command]
pub async fn stop_script(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let is_running = {
        let service = state.ai_service.lock().await;
        service.script_manager.is_running.clone()
    };
    if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(());
    }
    {
        let mut channels = state.script_channels.lock().await;
        // 发送端一掉，阻塞中的 input/choices/free_dialogue 事件立刻收 Err
        channels.input_tx = None;
        channels.choice_tx = None;
        channels.choice_allow_free = false;
    }
    // 等旧任务走完 on_script_end（含台词表截断），最多约 3 秒
    for _ in 0..30 {
        if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    if is_running.load(std::sync::atomic::Ordering::SeqCst) {
        tracing::warn!("[ScriptAPI] stop_script 等待超时，旧任务将在 IO 返回后自行收尾");
    }
    Ok(())
}

/// 把系统鼠标指针拖动到窗口内的指定 CSS 坐标。
///
/// 用于剧本的 `force_choice` 演出（DDLC 式强制拖动鼠标）。前端传视口 CSS 像素，
/// 这里只需换算成物理像素：**不能再叠加 `inner_position`**——tao 的
/// `set_cursor_position` 收的就是"窗口客户区相对坐标"，内部会自己做
/// ClientToScreen（Windows）/加窗口原点（macOS、Linux）。之前叠加了一次
/// inner_position，窗口非最大化时鼠标会被多拽出一段窗口偏移，方向看着就是歪的。
#[tauri::command]
pub async fn warp_cursor(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    // 诊断计数：前几次调用写 INFO 日志，便于排查"拖动没生效/方向不对"类反馈
    static LOG_COUNT: AtomicUsize = AtomicUsize::new(0);

    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let px = (x * scale).round() as i32;
    let py = (y * scale).round() as i32;
    let result = window
        .set_cursor_position(tauri::PhysicalPosition::new(px, py))
        .map_err(|e| e.to_string());
    let n = LOG_COUNT.fetch_add(1, Ordering::Relaxed);
    if n < 3 {
        tracing::info!(
            "[warp_cursor] logical=({x:.0},{y:.0}) scale={scale} client=({px},{py}) ok={}",
            result.is_ok()
        );
    }
    if let Err(ref e) = result {
        tracing::warn!("[warp_cursor] 设置光标位置失败: {e}");
    }
    result
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
