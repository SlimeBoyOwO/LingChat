//! 好感度查询命令。
//!
//! 好感度的变更由上帝 Agent 评估任务主动广播 `affection:changed` 事件
//! （见 `message_system::generator::MessageGenerator::maybe_evaluate_affection`），
//! 本命令只负责初始化/兜底刷新时的全量查询。

use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::affection::AffectionState;

/// 返回所有已加载角色的当前好感度状态（role_id 字符串键 → 六维数值 + 情绪标签）。
#[tauri::command]
pub async fn get_affection(app: AppHandle) -> Result<HashMap<String, AffectionState>, String> {
    let state = app.state::<AppState>();
    let game_status = {
        let service = state.ai_service.lock().await;
        service.game_status.clone()
    };
    let gs = game_status.lock().await;
    Ok(gs.role_manager.loaded_affections())
}
