//! Tauri IPC commands for script/story mode.
//!
//! Replaces Python's WebSocket-based script communication.
//! Frontend calls these via `invoke()` instead of `/v1/chat/script/*` HTTP endpoints.

use crate::AppState;
use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::ai_service::game_system::script_engine::events::ScriptContext;
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
    /// 剧本是否声明了 persistent_vars（跨局记忆变量）。前端据此显示
    /// 「重置记忆」按钮——按能力声明而不是按剧本名/警告类型硬编码。
    pub has_persistent_vars: bool,
    /// 来源："game" 或提供该剧本的插件 id。
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

/// 从剧本 settings（story_config 的 script_settings 段）读 persistent_vars 声明。
fn has_persistent_vars(s: &crate::ai_service::types::ScriptStatus) -> bool {
    s.settings
        .get("persistent_vars")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty())
}

fn summary_of(s: &crate::ai_service::types::ScriptStatus) -> ScriptSummary {
    ScriptSummary {
        script_name: s.name.clone(),
        description: s.description.clone(),
        folder_key: s.folder_key.clone(),
        intro_chapter: s.intro_chapter.clone(),
        content_warning: s.content_warning.clone(),
        has_persistent_vars: has_persistent_vars(s),
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

/// 在后台任务中执行剧本（含羁绊完成处理），并把任务句柄登记到 AppState。
/// 新引擎起跑前先中止上一个登记的引擎任务并清理其输入通道——
/// 阻塞中的旧引擎靠 oneshot 挂起，不中止就会往新会话的共享 GameStatus 里写台词。
pub(crate) async fn spawn_script_execution(
    app: AppHandle,
    script: crate::ai_service::types::ScriptStatus,
) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Clone shared handles for the background task
    let ai_service = state.ai_service.clone();
    let channels = state.script_channels.clone();
    let db = state.db.clone();
    let data_dir = state.ai_service.lock().await.data_dir.clone();
    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;
    let achievement_manager = state.achievement_manager.clone();

    // Lock AIService briefly to extract needed data and reserve the run.
    // DLC 导入/删除与编辑器试玩对同一个 flag 做 compare_exchange 互斥，
    // 必须先成功预留再动旧任务——否则会把正在运行的剧本杀掉却又拒绝起跑。
    let (game_status, config, is_running) = {
        let service = ai_service.lock().await;
        let game_status = service.game_status.clone();
        let config = service.config.clone();
        let is_running = service.script_manager.is_running.clone();
        is_running
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .map_err(|_| "已有剧本正在运行或 DLC 管理操作尚未完成".to_string())?;
        (game_status, config, is_running)
    };

    // 中止上一个登记的引擎任务并清理其输入通道——阻塞中的旧引擎靠 oneshot
    // 挂起，不中止就会往新会话的共享 GameStatus 里写台词。CAS 已保证此刻
    // 没有真正运行中的剧本，这里的 abort 只命中已收尾但句柄未摘除的旧任务。
    if let Some(handle) = state.script_task.lock().await.take() {
        handle.abort();
    }
    {
        let mut ch = state.script_channels.lock().await;
        let _ = ch.input_tx.take();
        let _ = ch.choice_tx.take();
        ch.extension_state.clear();
        ch.choice_allow_free = false;
    }

    // A previous frontend may have been closed before consuming script:end.
    // Bind every auxiliary/system-window ticket to this exact formal run.
    let glitch_window_generation =
        crate::ai_service::game_system::script_engine::extension::begin(&app);

    // 切入正式剧本即推进生成代号：自由对话/入场问候若仍在后台流式生成，
    // publisher 与最终写入守卫都会丢弃旧代号，禁止迟到寒暄混进剧本队列。
    {
        let mut status = game_status.lock().await;
        status.preview_generation = status.preview_generation.wrapping_add(1);
    }

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
            glitch_window_generation,
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
    Ok(())
}

#[tauri::command]
pub async fn start_script(app: AppHandle, script_name: String) -> Result<(), String> {
    let script = {
        let state = app.state::<AppState>();
        let service = state.ai_service.lock().await;
        let script = service
            .script_manager
            .all_scripts
            .get(&script_name)
            .ok_or_else(|| format!("剧本不存在: '{}'", script_name))?
            .clone();
        // Enforce again for direct IPC callers and state changes after preflight.
        // Reject before reserving a run, switching characters or opening windows.
        if let Some(error) = crate::ai_service::game_system::script_engine::extension::entry_error(
            &service.data_dir,
            &script.path_key(),
            &script.settings,
        )
        .map_err(|error| format!("无法读取剧本运行状态: {error}"))?
        {
            return Err(error);
        }
        script
    };

    spawn_script_execution(app, script).await
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
    // Invalidate the run-owned native-window epoch before waiting for the task.
    // This is authoritative even if the frontend's separate close invoke races/fails.
    close_script_glitch_windows(app.clone());
    // 乱码窗口标题同样是运行期演出，停止剧本时立刻还原
    crate::ai_service::game_system::script_engine::extension::restore_title(&app);
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
        channels.extension_state.clear();
        channels.choice_allow_free = false;

        // 文件监视器一并停掉并丢弃未消费的跳转
        if let Some(task) = channels.watch_task.take() {
            task.abort();
        }
        channels.watch_jump = None;
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
pub async fn script_submit_choice(
    app: AppHandle,
    choice: String,
    request_id: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    crate::ai_service::game_system::script_engine::extension::validate_choice(
        &mut channels,
        &choice,
        request_id.as_deref(),
    )?;
    if let Some(tx) = channels.choice_tx.take() {
        channels.choice_allow_free = false;

        channels.extension_state.clear();
        let _ = tx.send(choice);
        Ok(())
    } else {
        Err("当前没有等待选择的脚本事件".to_string())
    }
}

#[tauri::command]
pub fn close_script_glitch_windows(app: AppHandle) {
    crate::ai_service::game_system::script_engine::extension::close(&app);
}
