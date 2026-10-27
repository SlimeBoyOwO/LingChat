//! Tauri IPC commands for the adventure/bond (羁绊) system.
//!
//! Replaces Python's `/v1/chat/adventure/*` HTTP endpoints.
//! Frontend calls these via `invoke()` instead of HTTP.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use chrono::NaiveDateTime;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::AppState;
use crate::achievements::manager::AchievementManager;
use crate::achievements::types::AchievementDef;
use crate::adventures::manager::{AdventureManager, AdventureStates};
use crate::adventures::trigger::{self, UnlockedAdventureInfo};
use crate::ai_service::service::SharedAIService;
use crate::ai_service::types::ScriptStatus;

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

/// 依据数据库解锁表和运行时状态拼出冒险列表。
/// 「已完成」以数据库 `completed_at` 为准：内存里的 `completed_scripts` 只在本次运行有效，
/// 重启后就没了，用它判定会让通关过的羁绊重新变成「可玩」。
fn build_adventure_infos(
    adventures: Vec<&ScriptStatus>,
    states: &AdventureStates,
    is_running: bool,
    current_script_folder: Option<&str>,
) -> Vec<AdventureInfo> {
    // 与存档、成就等其它时间字段保持一致，统一走 RFC3339
    let fmt = |t: Option<NaiveDateTime>| t.map(|v| v.and_utc().to_rfc3339());

    let mut result: Vec<AdventureInfo> = adventures
        .into_iter()
        .map(|adv| {
            let (unlocked_at, completed_at) =
                states.get(&adv.folder_key).copied().unwrap_or((None, None));
            let is_unlocked = unlocked_at.is_some() || completed_at.is_some();

            // 正在重玩已通关的羁绊时，运行态优先——否则面板会把它当成「已完成」
            let status = if is_running && current_script_folder == Some(adv.folder_key.as_str()) {
                "in_progress"
            } else if completed_at.is_some() {
                "completed"
            } else if is_unlocked {
                "unlocked"
            } else {
                "locked"
            };

            AdventureInfo {
                adventure_folder: adv.folder_key.clone(),
                name: adv.name.clone(),
                description: adv.description.clone(),
                recommand_start: adv.recommand_start.clone(),
                order: adv.adventure.order,
                status: status.to_string(),
                unlocked_at: fmt(unlocked_at),
                completed_at: fmt(completed_at),
                unlock_conditions: adv.adventure.unlock_conditions.clone(),
            }
        })
        .collect();

    result.sort_by_key(|a| a.order);
    result
}

/// 无解锁条件的羁绊默认开放，列表拉取时顺手补上解锁记录
async fn auto_unlock_plain(db: &DatabaseConnection, adventures: &[&ScriptStatus]) {
    for adv in adventures {
        if adv.adventure.unlock_conditions.is_empty() {
            if let Err(e) = AdventureManager::unlock_adventure(
                db,
                &adv.folder_key,
                &adv.adventure.bound_character_folder,
            )
            .await
            {
                tracing::warn!("[AdventureAPI] 自动解锁「{}」失败: {}", adv.folder_key, e);
            }
        }
    }
}

/// 获取指定角色的所有羁绊冒险列表（含解锁状态）
#[tauri::command]
pub async fn list_character_adventures(
    app: AppHandle,
    character_folder: String,
) -> Result<Vec<AdventureInfo>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let service = state.ai_service.lock().await;

    let adventures: Vec<&ScriptStatus> = service
        .script_manager
        .get_character_adventures(&character_folder)
        .into_iter()
        .collect();

    auto_unlock_plain(db, &adventures).await;

    let states = AdventureManager::load_all_states(db)
        .await
        .map_err(|e| format!("读取冒险解锁状态失败: {}", e))?;

    let is_running = service.script_manager.is_running.load(Ordering::Relaxed);
    let gs = service.game_status.lock().await;
    let current_script_folder = gs.script_status.as_ref().map(|ss| ss.folder_key.as_str());

    Ok(build_adventure_infos(
        adventures,
        &states,
        is_running,
        current_script_folder,
    ))
}

/// 获取所有羁绊冒险（含解锁状态）
#[tauri::command]
pub async fn list_all_adventures(app: AppHandle) -> Result<Vec<AdventureInfo>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let service = state.ai_service.lock().await;

    let adventures: Vec<&ScriptStatus> = service
        .script_manager
        .get_all_adventures()
        .into_iter()
        .collect();

    auto_unlock_plain(db, &adventures).await;

    let states = AdventureManager::load_all_states(db)
        .await
        .map_err(|e| format!("读取冒险解锁状态失败: {}", e))?;

    let is_running = service.script_manager.is_running.load(Ordering::Relaxed);
    let gs = service.game_status.lock().await;
    let current_script_folder = gs.script_status.as_ref().map(|ss| ss.folder_key.as_str());

    Ok(build_adventure_infos(
        adventures,
        &states,
        is_running,
        current_script_folder,
    ))
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

    // 取出剧本后即结束对 state 的借用，随后把 app 交给共用的后台执行入口
    let script = {
        let service = state.ai_service.lock().await;
        // 剧本互斥：另一段羁绊（或独立剧本）在跑时不再启动新的，
        // 否则会把前一段静默 abort 掉，让它永远停在「进行中」
        crate::api::script::ensure_no_script_running(&service)?;
        service
            .script_manager
            .all_scripts
            .values()
            .find(|s| s.folder_key == adventure_folder)
            .ok_or_else(|| format!("冒险不存在: '{}'", adventure_folder))?
            .clone()
    };

    crate::api::script::spawn_script_execution(app, script).await;

    Ok(())
}

/// 手动检测是否有新冒险可解锁，返回新解锁列表并推送事件
#[tauri::command]
pub async fn check_adventure_unlocks(app: AppHandle) -> Result<Vec<UnlockedAdventureInfo>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let newly_unlocked = {
        let service = state.ai_service.lock().await;
        let adventures: Vec<&ScriptStatus> = service
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
                        AchievementDef {
                            title: title.to_string(),
                            description: desc.to_string(),
                            ach_type: ach_type.to_string(),
                            target_progress: 1,
                            hidden: false,
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
/// Called after `ScriptManager::execute_script` when a script finishes.
pub(crate) async fn handle_adventure_completion(
    db: &DatabaseConnection,
    achievement_manager: &Arc<Mutex<AchievementManager>>,
    app: &AppHandle,
    ai_service: &SharedAIService,
    folder_key: &str,
    completion_achievements: &[serde_json::Value],
    name: &str,
) {
    // Mark global completion in DB
    if let Err(e) = AdventureManager::mark_global_completed(db, folder_key).await {
        tracing::error!("[AdventureAPI] 持久化冒险完成状态失败: {}", e);
        return;
    }

    // Unlock completion achievements
    if !completion_achievements.is_empty() {
        let mut ach_mgr = achievement_manager.lock().await;
        for ach_def in completion_achievements {
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
                AchievementDef {
                    title: title.to_string(),
                    description: desc.to_string(),
                    ach_type: ach_type.to_string(),
                    target_progress: 1,
                    hidden: false,
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
        let adventures: Vec<&ScriptStatus> = service
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::AdventureConfig;
    use chrono::DateTime;
    use std::path::PathBuf;

    fn adventure(folder: &str, order: i32) -> ScriptStatus {
        ScriptStatus {
            folder_key: folder.to_string(),
            name: folder.to_string(),
            description: String::new(),
            intro_chapter: "main".to_string(),
            settings: serde_json::Map::new(),
            script_path: PathBuf::from(format!("/scripts/character/角色/{folder}")),
            recommand_start: String::new(),
            adventure: AdventureConfig {
                is_adventure: true,
                order,
                ..Default::default()
            },
            running_client_id: None,
            current_chapter_key: String::new(),
            current_event_process: 0,
            vars: serde_json::Map::new(),
            plugin_id: None,
        }
    }

    fn ts(secs: i64) -> Option<NaiveDateTime> {
        Some(DateTime::from_timestamp(secs, 0).unwrap().naive_utc())
    }

    fn build(
        adventures: &[&ScriptStatus],
        states: &AdventureStates,
        running: Option<&str>,
    ) -> Vec<AdventureInfo> {
        build_adventure_infos(adventures.to_vec(), states, running.is_some(), running)
    }

    #[test]
    fn locked_without_record() {
        let a = adventure("a", 1);
        let infos = build(&[&a], &AdventureStates::new(), None);

        assert_eq!(infos[0].status, "locked");
        assert!(infos[0].unlocked_at.is_none());
        assert!(infos[0].completed_at.is_none());
    }

    #[test]
    fn unlocked_when_only_unlocked_at() {
        let a = adventure("a", 1);
        let mut states = AdventureStates::new();
        states.insert("a".to_string(), (ts(1), None));

        assert_eq!(build(&[&a], &states, None)[0].status, "unlocked");
    }

    #[test]
    fn running_takes_precedence_over_completed() {
        let a = adventure("a", 1);
        let mut states = AdventureStates::new();
        states.insert("a".to_string(), (ts(1), ts(2)));

        // 重玩已通关的羁绊时，面板要显示「进行中」而不是「已完成」
        assert_eq!(build(&[&a], &states, Some("a"))[0].status, "in_progress");
        assert_eq!(build(&[&a], &states, None)[0].status, "completed");
    }

    #[test]
    fn running_folder_mismatch_not_in_progress() {
        let a = adventure("a", 1);
        let b = adventure("b", 2);
        let mut states = AdventureStates::new();
        states.insert("a".to_string(), (ts(1), None));
        states.insert("b".to_string(), (ts(1), None));

        let infos = build(&[&a, &b], &states, Some("b"));
        assert_eq!(infos[0].status, "unlocked");
        assert_eq!(infos[1].status, "in_progress");
    }

    #[test]
    fn completed_without_unlocked_at() {
        let a = adventure("a", 1);
        let mut states = AdventureStates::new();
        states.insert("a".to_string(), (None, ts(2)));

        assert_eq!(build(&[&a], &states, None)[0].status, "completed");
    }

    #[test]
    fn sorted_by_order_and_rfc3339_time() {
        let a = adventure("a", 2);
        let b = adventure("b", 1);
        let mut states = AdventureStates::new();
        states.insert("a".to_string(), (ts(1), None));

        let infos = build(&[&a, &b], &states, None);
        assert_eq!(infos[0].adventure_folder, "b");
        assert_eq!(
            infos[1].unlocked_at.as_deref(),
            Some("1970-01-01T00:00:01+00:00")
        );
    }
}
