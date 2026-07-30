use std::collections::HashMap;

use tauri::{AppHandle, Emitter};

use crate::achievements::types::Achievement;
use crate::AppState;

#[tauri::command]
pub async fn get_achievement_list(
    state: tauri::State<'_, AppState>,
) -> Result<HashMap<String, Achievement>, String> {
    let mgr = state.achievement_manager.lock().await;
    Ok(mgr.get_all_achievements())
}

#[tauri::command]
pub async fn unlock_achievement(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    achievement_id: String,
) -> Result<(), String> {
    let mut mgr = state.achievement_manager.lock().await;
    if let Some(achievement) = mgr.unlock(&achievement_id) {
        app.emit("achievement:unlocked", &achievement)
            .map_err(|e| format!("发送成就事件失败: {}", e))?;
    }
    Ok(())
}
