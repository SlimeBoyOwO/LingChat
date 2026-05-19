use tauri::{AppHandle, Manager};
use crate::AppState;
use crate::ai_service::proactive_system::types::UserScheduleSettings;

#[tauri::command]
pub async fn get_schedules() -> Result<UserScheduleSettings, String> {
    let schedules_path = crate::init::static_copy::resolve_data_dir()
        .join("game_data")
        .join("schedules.json");

    if !schedules_path.exists() {
        return Ok(UserScheduleSettings::default());
    }

    let content = std::fs::read_to_string(&schedules_path)
        .map_err(|e| format!("Failed to read schedules.json: {}", e))?;

    let parsed: UserScheduleSettings = serde_json::from_str(&content)
        .unwrap_or_default();

    Ok(parsed)
}

#[tauri::command]
pub async fn save_schedules(
    app: AppHandle,
    data: UserScheduleSettings,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let schedules_path = crate::init::static_copy::resolve_data_dir()
        .join("game_data")
        .join("schedules.json");

    let content = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize schedules data: {}", e))?;

    std::fs::write(&schedules_path, content)
        .map_err(|e| format!("Failed to write schedules.json: {}", e))?;

    log::info!("[ScheduleAPI] Schedules saved successfully at {:?}", schedules_path);

    // Reload settings in the proactive system
    if let Some(proactive) = &state.proactive_system {
        let mut sys = proactive.lock().await;
        sys.reload().await;
    }

    Ok("日程设置已保存！".to_string())
}

#[tauri::command]
pub async fn reload_proactive_system(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    if let Some(proactive) = &state.proactive_system {
        let mut sys = proactive.lock().await;
        sys.reload().await;
        Ok("主动对话系统配置已重载！".to_string())
    } else {
        Err("主动对话系统未运行！".to_string())
    }
}
