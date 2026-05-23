use anyhow::Result;
use chrono::Timelike;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::Value;

use crate::achievements::manager::AchievementManager;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::ScriptStatus;

use super::manager::AdventureManager;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UnlockedAdventureInfo {
    pub adventure_folder: String,
    pub name: String,
    pub description: String,
    pub character_folder: String,
    pub order: i32,
}

/// Evaluate all unlock conditions for all adventures.
/// Returns list of newly unlocked adventures.
pub fn check_all_adventures(
    db: &DatabaseConnection,
    achievement_mgr: &AchievementManager,
    game_status: &GameStatus,
    adventures: &[&ScriptStatus],
) -> Result<Vec<UnlockedAdventureInfo>> {
    let rt = tokio::runtime::Handle::current();

    let mut newly_unlocked = Vec::new();

    for adv in adventures {
        let folder = &adv.folder_key;

        // Skip if already unlocked
        let already_unlocked = tokio::task::block_in_place(|| {
            rt.block_on(AdventureManager::is_unlocked(db, folder))
        })
        .unwrap_or(false);
        if already_unlocked {
            continue;
        }

        // Evaluate all conditions (AND logic)
        let conditions = &adv.adventure.unlock_conditions;
        if conditions.is_empty() {
            // No conditions: default-unlock
            tokio::task::block_in_place(|| {
                rt.block_on(AdventureManager::unlock_adventure(
                    db,
                    folder,
                    &adv.adventure.bound_character_folder,
                ))
            })?;
            newly_unlocked.push(UnlockedAdventureInfo {
                adventure_folder: folder.clone(),
                name: adv.name.clone(),
                description: adv.description.clone(),
                character_folder: adv.adventure.bound_character_folder.clone(),
                order: adv.adventure.order,
            });
            continue;
        }

        let all_passed = conditions.iter().all(|cond| {
            evaluate_condition(db, achievement_mgr, game_status, cond)
        });

        if all_passed {
            tokio::task::block_in_place(|| {
                rt.block_on(AdventureManager::unlock_adventure(
                    db,
                    folder,
                    &adv.adventure.bound_character_folder,
                ))
            })?;
            newly_unlocked.push(UnlockedAdventureInfo {
                adventure_folder: folder.clone(),
                name: adv.name.clone(),
                description: adv.description.clone(),
                character_folder: adv.adventure.bound_character_folder.clone(),
                order: adv.adventure.order,
            });
        }
    }

    Ok(newly_unlocked)
}

/// Evaluate a single unlock condition.
fn evaluate_condition(
    db: &DatabaseConnection,
    achievement_mgr: &AchievementManager,
    game_status: &GameStatus,
    condition: &Value,
) -> bool {
    let cond_type = condition
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match cond_type {
        "chat_count" => {
            let threshold = condition
                .get("threshold")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as usize;
            game_status.chat_message_count() >= threshold
        }

        "time_range" => {
            let start = condition
                .get("start_hour")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;
            let end = condition
                .get("end_hour")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;
            let now = chrono::Local::now();
            let hour = now.hour();
            // Support cross-midnight ranges (e.g., 23-6 → 23,0,1,2,3,4,5)
            if start <= end {
                hour >= start && hour < end
            } else {
                hour >= start || hour < end
            }
        }

        "adventure_completed" => {
            let prereq = condition
                .get("adventure_folder")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if prereq.is_empty() {
                return false;
            }
            let rt = tokio::runtime::Handle::current();
            tokio::task::block_in_place(|| {
                rt.block_on(AdventureManager::is_globally_completed(db, prereq))
            })
            .unwrap_or(false)
        }

        "achievement_unlocked" => {
            let ach_id = condition
                .get("achievement_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if ach_id.is_empty() {
                return false;
            }
            achievement_mgr.is_unlocked(ach_id)
        }

        _ => {
            log::warn!("未知的冒险解锁条件类型: {}", cond_type);
            false
        }
    }
}
