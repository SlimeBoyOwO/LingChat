//! 设置页「记忆系统」浏览面板的 Tauri 命令：列出有记忆的角色、列出 / 删除单条记忆。
//!
//! 长期记忆存放在每「剧本 × 角色」一张的共用归档表 `memory_archive_{role_id}`
//! （目录见 `memory_archive_profiles`），本模块经 `MemoryService` / `store` 读写
//! 该套表，不碰 `game_status`，也不依赖 `memory_enabled` 开关——功能关闭时用户
//! 仍能查看和清理已存的记忆。

use std::collections::HashMap;

use sea_orm::EntityTrait;
use serde::Serialize;
use tauri::Manager;

use crate::AppState;
use crate::ai_service::memory::{
    MemoryItem,
    store::{count_role_entries, role_ids_with_memories},
};
use crate::db::entities::role;

/// 角色下拉选项：存在长期记忆的角色及其条数。
#[derive(Clone, Serialize)]
pub struct MemoryRoleSummary {
    pub role_id: i32,
    /// 角色名；`role` 表缺失该行时为空串，由前端回退成「ID N」展示。
    pub name: String,
    /// 可浏览条目数：长期记忆（按 `memory_id` 去重） + 旧版整合条目。共用表统一视图。
    pub count: usize,
}

/// 列出存在长期记忆的角色（按 `role_id` 升序），供浏览面板的角色下拉使用。
#[tauri::command]
pub async fn memory_list_roles(app: tauri::AppHandle) -> Result<Vec<MemoryRoleSummary>, String> {
    let state = app.state::<AppState>();

    let mut role_ids = role_ids_with_memories(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    role_ids.sort_unstable();

    let names: HashMap<i32, String> = role::Entity::find()
        .all(&state.db)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();

    let mut out = Vec::with_capacity(role_ids.len());
    for role_id in role_ids {
        // 可浏览条目数 = 长期记忆条数 + 旧版整合条目数（面板统一视图的计数）。
        let count = count_role_entries(&state.db, role_id)
            .await
            .map_err(|e| e.to_string())?;
        out.push(MemoryRoleSummary {
            role_id,
            name: names.get(&role_id).cloned().unwrap_or_default(),
            count,
        });
    }
    Ok(out)
}

/// 列出某角色的全部长期记忆（分块已聚合、标签已从正文剥离，最新在前）。
#[tauri::command]
pub async fn memory_list(app: tauri::AppHandle, role_id: i32) -> Result<Vec<MemoryItem>, String> {
    let state = app.state::<AppState>();
    state
        .memory
        .list_memories(role_id)
        .await
        .map_err(|e| e.to_string())
}

/// 删除某角色的一条长期记忆（按 `memory_id`，作用域限定在该角色名下），
/// 返回被删除的行数（分块多行一并删除）。
#[tauri::command]
pub async fn memory_delete(
    app: tauri::AppHandle,
    role_id: i32,
    memory_id: String,
) -> Result<usize, String> {
    let state = app.state::<AppState>();
    state
        .memory
        .delete_by_id(role_id, &memory_id)
        .await
        .map_err(|e| e.to_string())
}
