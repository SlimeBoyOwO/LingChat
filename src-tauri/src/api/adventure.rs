//! Tauri IPC commands for the adventure/bond (羁绊) system.
//!
//! Replaces Python's `/v1/chat/adventure/*` HTTP endpoints.
//! Frontend calls these via `invoke()` instead of HTTP.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::adventures::manager::AdventureManager;
use crate::adventures::trigger::{self, UnlockedAdventureInfo};
use crate::AppState;

// ============================================================
// Response types
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AdventureInfo {
    pub adventure_folder: String,
    pub name: String,
    pub description: String,
    pub recommand_start: String,
    pub order: i32,
    pub status: String, // "locked" | "unlocked" | "in_progress" | "completed"
    pub unlocked_at: Option<String>,
    pub completed_at: Option<String>,
    pub unlock_conditions: Vec<serde_json::Value>,
}

// ============================================================
// Tauri commands
// ============================================================

/// 获取指定角色的所有羁绊冒险列表（含解锁状态）
#[tauri::command]
pub async fn list_character_adventures(
    app: AppHandle,
    character_folder: String,
) -> Result<Vec<AdventureInfo>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let service = state.ai_service.lock().await;

    let adventures: Vec<&crate::ai_service::types::ScriptStatus> = service
        .script_manager
        .get_character_adventures(&character_folder)
        .into_iter()
        .collect();

    // Auto-unlock adventures with no conditions (empty conditions = default unlocked)
    for adv in &adventures {
        if adv.adventure.unlock_conditions.is_empty() {
            let _ = AdventureManager::unlock_adventure(
                db,
                &adv.folder_key,
                &adv.adventure.bound_character_folder,
            )
            .await;
        }
    }

    let is_running = service.script_manager.is_running;
    let gs = service.game_status.lock().await;
    let current_script_folder = gs.script_status.as_ref().map(|ss| ss.folder_key.clone());
    let completed = &gs.completed_scripts;

    let mut result = Vec::new();
    for adv in adventures {
        let is_unlocked = AdventureManager::is_unlocked(db, &adv.folder_key)
            .await
            .unwrap_or(false);

        let status = if completed.contains(&adv.path_key()) {
            "completed"
        } else if is_running && current_script_folder.as_deref() == Some(&adv.folder_key) {
            "in_progress"
        } else if is_unlocked {
            "unlocked"
        } else {
            "locked"
        };

        result.push(AdventureInfo {
            adventure_folder: adv.folder_key.clone(),
            name: adv.name.clone(),
            description: adv.description.clone(),
            recommand_start: adv.recommand_start.clone(),
            order: adv.adventure.order,
            status: status.to_string(),
            unlocked_at: None,
            completed_at: None,
            unlock_conditions: adv.adventure.unlock_conditions.clone(),
        });
    }

    // Sort by order
    result.sort_by_key(|a| a.order);
    Ok(result)
}

/// 获取所有羁绊冒险（含解锁状态）
#[tauri::command]
pub async fn list_all_adventures(app: AppHandle) -> Result<Vec<AdventureInfo>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let service = state.ai_service.lock().await;

    let adventures: Vec<&crate::ai_service::types::ScriptStatus> = service
        .script_manager
        .get_all_adventures()
        .into_iter()
        .collect();

    // Auto-unlock adventures with no conditions (empty conditions = default unlocked)
    for adv in &adventures {
        if adv.adventure.unlock_conditions.is_empty() {
            let _ = AdventureManager::unlock_adventure(
                db,
                &adv.folder_key,
                &adv.adventure.bound_character_folder,
            )
            .await;
        }
    }

    let is_running = service.script_manager.is_running;
    let gs = service.game_status.lock().await;
    let current_script_folder = gs.script_status.as_ref().map(|ss| ss.folder_key.clone());
    let completed = &gs.completed_scripts;

    let mut result = Vec::new();
    for adv in adventures {
        let is_unlocked = AdventureManager::is_unlocked(db, &adv.folder_key)
            .await
            .unwrap_or(false);

        let status = if completed.contains(&adv.path_key()) {
            "completed"
        } else if is_running && current_script_folder.as_deref() == Some(&adv.folder_key) {
            "in_progress"
        } else if is_unlocked {
            "unlocked"
        } else {
            "locked"
        };

        result.push(AdventureInfo {
            adventure_folder: adv.folder_key.clone(),
            name: adv.name.clone(),
            description: adv.description.clone(),
            recommand_start: adv.recommand_start.clone(),
            order: adv.adventure.order,
            status: status.to_string(),
            unlocked_at: None,
            completed_at: None,
            unlock_conditions: adv.adventure.unlock_conditions.clone(),
        });
    }

    Ok(result)
}

/// 启动指定羁绊冒险
#[tauri::command]
pub async fn start_adventure(app: AppHandle, adventure_folder: String) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Validate: adventure must be unlocked
    let is_unlocked = AdventureManager::is_unlocked(&state.db, &adventure_folder)
        .await
        .map_err(|e| format!("查询冒险状态失败: {}", e))?;

    if !is_unlocked {
        return Err("冒险尚未解锁，无法启动".to_string());
    }

    // Find the script_name matching this adventure folder_key
    let script_name = {
        let service = state.ai_service.lock().await;
        service
            .script_manager
            .all_scripts
            .values()
            .find(|s| s.folder_key == adventure_folder)
            .map(|s| s.name.clone())
            .ok_or_else(|| format!("冒险不存在: '{}'", adventure_folder))?
    };

    // Delegate to the same background execution as start_script
    let ai_service = state.ai_service.clone();
    let channels = state.script_channels.clone();
    let db = state.db.clone();
    let data_dir = state.ai_service.lock().await.data_dir.clone();
    let llm = state.chat.llm.clone();
    let achievement_manager = state.achievement_manager.clone();

    tokio::spawn(async move {
        match super::script::run_script_background(
            ai_service,
            channels,
            db,
            data_dir,
            app,
            llm,
            script_name,
            achievement_manager,
        )
        .await
        {
            Ok(()) => tracing::info!("[AdventureAPI] 冒险执行完成"),
            Err(e) => tracing::error!("[AdventureAPI] 冒险执行错误: {}", e),
        }
    });

    Ok(())
}

/// 手动检测是否有新冒险可解锁，返回新解锁列表并推送事件
#[tauri::command]
pub async fn check_adventure_unlocks(app: AppHandle) -> Result<Vec<UnlockedAdventureInfo>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let newly_unlocked = {
        let service = state.ai_service.lock().await;
        let adventures: Vec<&crate::ai_service::types::ScriptStatus> = service
            .script_manager
            .get_all_adventures()
            .into_iter()
            .collect();
        let game_status = service.game_status.lock().await;

        let mut ach_mgr = state.achievement_manager.lock().await;

        // Pre-register completion achievements before checking conditions
        for adv in &adventures {
            for ach_def in &adv.adventure.completion_achievements {
                if let (Some(id), Some(title), Some(desc), Some(ach_type)) = (
                    ach_def.get("id").and_then(|v| v.as_str()),
                    ach_def.get("title").and_then(|v| v.as_str()),
                    ach_def.get("description").and_then(|v| v.as_str()),
                    ach_def.get("type").and_then(|v| v.as_str()),
                ) {
                    ach_mgr.register_achievement(
                        id.to_string(),
                        crate::achievements::types::AchievementDef {
                            title: title.to_string(),
                            description: desc.to_string(),
                            ach_type: ach_type.to_string(),
                            target_progress: 1,
                            img_url: None,
                            audio_url: None,
                            duration: None,
                        },
                    );
                }
            }
        }

        let result = trigger::check_all_adventures(db, &ach_mgr, &game_status, &adventures)
            .map_err(|e| format!("检测冒险解锁失败: {}", e))?;
        result
    };

    // Emit events for newly unlocked adventures
    for info in &newly_unlocked {
        let _ = app.emit("adventure:unlocked", info);
    }

    Ok(newly_unlocked)
}

/// 重置冒险进度以供重玩
#[tauri::command]
pub async fn reset_adventure(app: AppHandle, adventure_folder: String) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Remove from in-memory completed set
    {
        let service = state.ai_service.lock().await;
        let path_key = service
            .script_manager
            .all_scripts
            .values()
            .find(|s| s.folder_key == adventure_folder)
            .map(|s| s.path_key());
        if let Some(key) = path_key {
            service
                .game_status
                .lock()
                .await
                .completed_scripts
                .remove(&key);
        }
    }

    // Delete from DB
    AdventureManager::reset_adventure(&state.db, &adventure_folder)
        .await
        .map_err(|e| format!("重置冒险失败: {}", e))?;

    tracing::info!("[AdventureAPI] 冒险已重置: {}", adventure_folder);
    Ok(())
}

// ============================================================
// Shared helpers (used by script.rs for completion handling)
// ============================================================

/// Handle adventure completion: persist to DB, unlock achievements, check chained unlocks.
/// Called from `run_script_background` when a script finishes.
pub(crate) async fn handle_adventure_completion(
    db: &sea_orm::DatabaseConnection,
    achievement_manager: &std::sync::Arc<
        tokio::sync::Mutex<crate::achievements::manager::AchievementManager>,
    >,
    app: &AppHandle,
    ai_service: &crate::ai_service::service::SharedAIService,
) {
    let (folder_key, completion_achievements, name) = {
        let service = ai_service.lock().await;
        let gs = service.game_status.lock().await;
        match gs.script_status.as_ref() {
            Some(ss) if ss.adventure.is_adventure => (
                ss.folder_key.clone(),
                ss.adventure.completion_achievements.clone(),
                ss.name.clone(),
            ),
            _ => return,
        }
    };

    // Mark global completion in DB
    if let Err(e) = AdventureManager::mark_global_completed(db, &folder_key).await {
        tracing::error!("[AdventureAPI] 持久化冒险完成状态失败: {}", e);
        return;
    }

    // Unlock completion achievements
    if !completion_achievements.is_empty() {
        let mut ach_mgr = achievement_manager.lock().await;
        for ach_def in &completion_achievements {
            let (ach_id, title, desc, ach_type) = match (
                ach_def.get("id").and_then(|v| v.as_str()),
                ach_def.get("title").and_then(|v| v.as_str()),
                ach_def.get("description").and_then(|v| v.as_str()),
                ach_def.get("type").and_then(|v| v.as_str()),
            ) {
                (Some(id), Some(t), Some(d), Some(ty)) => (id, t, d, ty),
                _ => continue,
            };

            ach_mgr.register_achievement(
                ach_id.to_string(),
                crate::achievements::types::AchievementDef {
                    title: title.to_string(),
                    description: desc.to_string(),
                    ach_type: ach_type.to_string(),
                    target_progress: 1,
                    img_url: None,
                    audio_url: None,
                    duration: None,
                },
            );
            if let Some(achievement) = ach_mgr.unlock(ach_id) {
                let _ = app.emit("achievement:unlocked", &achievement);
            }
        }
    }

    // Emit adventure completed event
    let _ = app.emit(
        "adventure:completed",
        &serde_json::json!({
            "adventure_folder": folder_key,
            "name": name,
        }),
    );

    // Check for chained adventure unlocks
    let newly_unlocked = {
        let service = ai_service.lock().await;
        let adventures: Vec<&crate::ai_service::types::ScriptStatus> = service
            .script_manager
            .get_all_adventures()
            .into_iter()
            .collect();
        let gs = service.game_status.lock().await;
        let ach_mgr = achievement_manager.lock().await;
        trigger::check_all_adventures(db, &ach_mgr, &gs, &adventures).unwrap_or_default()
    };

    for info in &newly_unlocked {
        let _ = app.emit("adventure:unlocked", info);
    }
}
