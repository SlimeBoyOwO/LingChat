//! 导演 Agent 的命令层。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::State;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::director::context::assemble;
use crate::ai_service::god_agent::director::draft::default_mode_view;
use crate::ai_service::god_agent::director::{
    DirectorAgentEvent, DirectorConfirmResult, DirectorContext, DirectorDraftView,
    DirectorModeView, DirectorState,
};
use crate::ai_service::llm::slot_snapshot;

/// 当前存档 ID。先锁 AI 服务取句柄，再短锁 GameStatus，两把锁都不跨 await 持有。
async fn current_save_id(state: &AppState) -> Option<i32> {
    let game_status = game_status_handle(state).await;
    let save_id = game_status.lock().await.active_save_id;
    save_id
}

async fn game_status_handle(state: &AppState) -> Arc<tokio::sync::Mutex<GameStatus>> {
    let service = state.ai_service.lock().await;
    service.game_status.clone()
}

/// 固定剧本或编辑器试玩期间禁用导演工具。
pub(crate) async fn director_tools_enabled(state: &AppState) -> bool {
    let (script_running, game_status) = {
        let service = state.ai_service.lock().await;
        (
            service.script_manager.is_running.load(Ordering::SeqCst),
            service.game_status.clone(),
        )
    };
    let in_script_mode = game_status.lock().await.script_status.is_some();
    !script_running && !in_script_mode
}

#[tauri::command]
pub async fn director_get_state(state: State<'_, AppState>) -> Result<DirectorState, String> {
    let Some(core) = state.god_agent.clone() else {
        return Ok(DirectorState {
            available: false,
            save_bound: false,
            save_id: None,
            running: false,
            messages: Vec::new(),
        });
    };

    let save_id = current_save_id(&state).await;
    core.director.sync_save(&state.db, save_id).await?;
    let (save_id, running, messages) = core.director.state_snapshot().await;
    Ok(DirectorState {
        available: slot_snapshot(&core.llm).await.is_some(),
        save_bound: save_id.is_some(),
        save_id,
        running,
        messages,
    })
}

#[tauri::command]
pub async fn director_start_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    message: String,
    channel: Channel<DirectorAgentEvent>,
) -> Result<i32, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("消息不能为空".to_string());
    }
    let core = state
        .god_agent
        .clone()
        .ok_or_else(|| "导演服务不可用：未配置可用的对话模型".to_string())?;
    let llm = slot_snapshot(&core.llm)
        .await
        .ok_or_else(|| "导演服务不可用：未配置可用的对话模型".to_string())?;
    let provider = Some(llm.config().provider.clone());
    let model = Some(llm.config().model.clone());

    let save_id = current_save_id(&state).await;
    let tools_enabled = director_tools_enabled(&state).await;
    let turn = core
        .director
        .begin_turn(&state.db, save_id, message)
        .await?;
    let user_id = turn.user_id;
    let turn_id = turn.id();

    let window = core.config_snapshot().recent_window;
    let assembled = {
        let game_status = game_status_handle(&state).await;
        let guard = game_status.lock().await;
        assemble(&guard, window)
    };
    let (request, _context) = core
        .director
        .build_request(assembled, "last_run", provider, model)
        .await;

    let db = state.db.clone();
    let core_for_task = core.clone();
    let handle = tauri::async_runtime::spawn(async move {
        core_for_task
            .director
            .run_chat(db, channel, request, turn, app, tools_enabled)
            .await;
    });
    core.director.set_task(turn_id, handle).await;

    Ok(user_id)
}

/// 请求停止：取消令牌在 chunk 之间中断，等已产出的正文落库后返回。
#[tauri::command]
pub async fn director_stop_chat(state: State<'_, AppState>) -> Result<(), String> {
    let Some(core) = state.god_agent.clone() else {
        return Ok(());
    };
    let (cancel, task) = core.director.take_control().await;
    let had_token = cancel.is_some();
    if let Some(token) = cancel {
        token.cancel();
    }

    match task {
        Some(handle) => {
            let _ = handle.await;
        },
        None if had_token => {
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while core.director.is_running().await {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            })
            .await
            .map_err(|_| "导演停止请求超时，请稍后重试".to_string())?;
        },
        None => {},
    }
    Ok(())
}

#[tauri::command]
pub async fn director_clear(state: State<'_, AppState>) -> Result<(), String> {
    let core = state
        .god_agent
        .clone()
        .ok_or_else(|| "导演服务不可用".to_string())?;
    let save_id = current_save_id(&state).await;
    core.director.clear(&state.db, save_id).await
}

/// 上下文快照：默认返回上次实际请求的那份，rebuild=true 时按当前现场重算。
#[tauri::command]
pub async fn director_get_context(
    state: State<'_, AppState>,
    rebuild: Option<bool>,
) -> Result<DirectorContext, String> {
    let core = state
        .god_agent
        .clone()
        .ok_or_else(|| "导演服务不可用：未配置可用的对话模型".to_string())?;
    let save_id = current_save_id(&state).await;
    core.director.sync_save(&state.db, save_id).await?;

    if !rebuild.unwrap_or(false) {
        if let Some(context) = core.director.last_context().await {
            return Ok(context);
        }
    }

    let window = core.config_snapshot().recent_window;
    let assembled = {
        let game_status = game_status_handle(&state).await;
        let guard = game_status.lock().await;
        assemble(&guard, window)
    };
    let (provider, model) = match slot_snapshot(&core.llm).await {
        Some(llm) => (
            Some(llm.config().provider.clone()),
            Some(llm.config().model.clone()),
        ),
        None => (None, None),
    };
    Ok(core
        .director
        .rebuild_context(assembled, provider, model)
        .await)
}

#[tauri::command]
pub async fn director_get_tool_history(
    state: State<'_, AppState>,
) -> Result<Vec<crate::ai_service::god_agent::director::DirectorMessage>, String> {
    let core = state
        .god_agent
        .clone()
        .ok_or_else(|| "导演服务不可用：未配置可用的对话模型".to_string())?;
    let save_id = current_save_id(&state).await;
    core.director.sync_save(&state.db, save_id).await?;
    Ok(core.director.tool_history().await)
}

#[tauri::command]
pub async fn director_get_mode(state: State<'_, AppState>) -> Result<DirectorModeView, String> {
    let Some(core) = state.god_agent.clone() else {
        return Ok(default_mode_view());
    };
    let save_id = current_save_id(&state).await;
    core.director.sync_save(&state.db, save_id).await?;
    Ok(core.director.mode_view().await)
}

/// 设置模式开关与间隔轮数；interval 省略时保留原值，关闭时作废当前草案。
#[tauri::command]
pub async fn director_set_mode(
    state: State<'_, AppState>,
    enabled: bool,
    interval_rounds: Option<usize>,
) -> Result<DirectorModeView, String> {
    let core = state
        .god_agent
        .clone()
        .ok_or_else(|| "导演服务不可用：未配置可用的对话模型".to_string())?;
    core.director.set_mode(enabled, interval_rounds).await
}

#[tauri::command]
pub async fn director_get_draft(
    state: State<'_, AppState>,
) -> Result<Option<DirectorDraftView>, String> {
    let Some(core) = state.god_agent.clone() else {
        return Ok(None);
    };
    let save_id = current_save_id(&state).await;
    core.director.sync_save(&state.db, save_id).await?;
    let view = core.director.draft_view().await;
    Ok((view.status != "idle" || !view.events.is_empty()).then_some(view))
}

#[tauri::command]
pub async fn director_confirm_draft(app: AppHandle) -> Result<DirectorConfirmResult, String> {
    let state = app.state::<AppState>();
    let core = state
        .god_agent
        .clone()
        .ok_or_else(|| "导演服务不可用：未配置可用的对话模型".to_string())?;
    // 固定剧本运行期间拒绝草案入队
    if !director_tools_enabled(&state).await {
        return Ok(DirectorConfirmResult {
            ok: false,
            applied: 0,
            error: Some("当前正在运行固定剧本，导演草案不可用；请结束剧本后再确认".to_string()),
        });
    }
    core.director.confirm_draft(app.clone()).await
}

#[tauri::command]
pub async fn director_discard_draft(
    state: State<'_, AppState>,
) -> Result<Option<DirectorDraftView>, String> {
    let Some(core) = state.god_agent.clone() else {
        return Ok(None);
    };
    let view = core.director.discard_draft().await?;
    Ok((view.status != "idle" || !view.events.is_empty()).then_some(view))
}
