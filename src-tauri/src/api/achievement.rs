use std::collections::HashMap;

use tauri::AppHandle;

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
    // 与剧本事件的成就解锁共用 unlock_and_emit，行为一致（落盘 + 推送）
    crate::achievements::unlock_and_emit(&app, state.achievement_manager.as_ref(), &achievement_id)
        .await?;
    Ok(())
}
