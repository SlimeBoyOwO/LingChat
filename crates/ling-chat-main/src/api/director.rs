//! 导演 Agent 的 Tauri 命令层（`director_*`）。

use std::sync::Arc;

use tauri::State;
use tauri::ipc::Channel;

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::director::{
    DirectorAgent, DirectorAgentEvent, DirectorContext, DirectorState,
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
    if let Err(e) = core.director.sync_save(&state.db, save_id).await {
        tracing::warn!("导演会话同步存档失败: {e}");
    }
    let (save_id, running, messages) = core.director.state_snapshot().await;
    Ok(DirectorState {
        available: slot_snapshot(&core.llm).await.is_some(),
        save_bound: save_id.is_some(),
        save_id,
        running,
        messages,
    })
}

/// 开始一轮对话。返回本次用户消息的 DB id（0 = 自由对话未落库）。
#[tauri::command]
pub async fn director_start_chat(
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
    let turn = core
        .director
        .begin_turn(&state.db, save_id, message)
        .await?;
    let user_id = turn.user_id;

    let window = core.config_snapshot().recent_window;
    let assembled = {
        let game_status = game_status_handle(&state).await;
        let guard = game_status.lock().await;
        DirectorAgent::assemble(&guard, window)
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
            .run_chat(db, channel, request, turn)
            .await;
    });
    core.director.set_task(handle).await;

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
        // 句柄尚未交给会话（开始命令刚返回前的一瞬）：等运行位清掉即可
        None if had_token => {
            for _ in 0..60 {
                if !core.director.is_running().await {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
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
        DirectorAgent::assemble(&guard, window)
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
