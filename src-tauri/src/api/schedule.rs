use crate::ai_service::proactive_system::types::UserScheduleSettings;
use crate::AppState;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn get_schedules() -> Result<UserScheduleSettings, String> {
    let schedules_path = crate::api::data_dir()
        .join("game_data")
        .join("schedules.json");

    if !schedules_path.exists() {
        return Ok(UserScheduleSettings::default());
    }

    let content = std::fs::read_to_string(&schedules_path)
        .map_err(|e| format!("Failed to read schedules.json: {}", e))?;

    let parsed: UserScheduleSettings = serde_json::from_str(&content).unwrap_or_default();

    Ok(parsed)
}

#[tauri::command]
pub async fn save_schedules(app: AppHandle, data: UserScheduleSettings) -> Result<String, String> {
    let state = app.state::<AppState>();
    let schedules_path = crate::api::data_dir()
        .join("game_data")
        .join("schedules.json");

    // Read-merge-write: only overwrite fields the frontend actually sent.
    // None means "not provided" — preserve the existing value on disk.
    let merged = if schedules_path.exists() {
        let existing_content = std::fs::read_to_string(&schedules_path)
            .map_err(|e| format!("Failed to read schedules.json: {}", e))?;
        let mut existing: UserScheduleSettings =
            serde_json::from_str(&existing_content).unwrap_or_default();
        if data.schedule_groups.is_some() {
            existing.schedule_groups = data.schedule_groups;
        }
        if data.todo_groups.is_some() {
            existing.todo_groups = data.todo_groups;
        }
        if data.important_days.is_some() {
            existing.important_days = data.important_days;
        }
        existing
    } else {
        data
    };

    let content = serde_json::to_string_pretty(&merged)
        .map_err(|e| format!("Failed to serialize schedules data: {}", e))?;

    std::fs::write(&schedules_path, content)
        .map_err(|e| format!("Failed to write schedules.json: {}", e))?;

    tracing::info!(
        "[ScheduleAPI] Schedules saved successfully at {:?}",
        schedules_path
    );

    // Reload settings in the proactive system
    if let Some(proactive) = &state.proactive_system {
        crate::ai_service::proactive_system::ProactiveSystem::reload(proactive.clone()).await;
    }

    Ok("日程设置已保存！".to_string())
}

#[tauri::command]
pub async fn test_proactive_message(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();

    if let Some(ps) = &state.proactive_system {
        match crate::ai_service::proactive_system::ProactiveSystem::test_screen_proactive(
            ps.clone(),
        )
        .await
        {
            Ok(Some(prompt)) => Ok(format!("已触发主动搭话: {}", prompt)),
            Ok(None) => Ok("屏幕分析返回 [PASS]，未触发搭话".to_string()),
            Err(e) => Err(e),
        }
    } else {
        Err("主动对话系统未初始化".to_string())
    }
}

#[tauri::command]
pub async fn reload_proactive_system(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let proactive_running = state.proactive_system.is_some();

    // 先应用主动系统的开关/闸门；即便视觉请求仍在收尾，在途结果也会按新配置复核。
    if let Some(proactive) = &state.proactive_system {
        crate::ai_service::proactive_system::ProactiveSystem::reload(proactive.clone()).await;
    }

    // 无论主动系统是否已启动，都刷新共享屏幕分析器，让“看桌面”等路径使用最新设置。
    let pconfig = crate::config::proactive::ProactiveConfig::load(&app);
    let sa_config =
        crate::ai_service::screen_analyzer::build_screen_analyzer_config(&app, &pconfig);
    let mut sa = state.screen_analyzer.lock().await;
    sa.update_config(sa_config);
    drop(sa);

    if proactive_running {
        Ok("主动对话系统配置已重载！".to_string())
    } else {
        Ok("设置已保存，主动对话系统当前未运行，将在启动后自动生效。".to_string())
    }
}
