//! 永久记忆（MemoryBank）的只读检视命令。
//!
//! 这是**只读**入口：不触发压缩、不写回记忆库、不改存档、不惰性注册角色。
//! 特别地，它**不会**走 `check_and_trigger_auto_update` 里那段指针修正——
//! 那段会写回 bank 并置 `has_pending`，进而经 `sync_to_role()` →
//! `persist_memory_banks_to_db()` 落库；只读入口绝不能触发持久化写入。
//!
//! 后续若要加写操作（手动触发一次压缩 / 清空某角色记忆 / 重置指针），
//! 也放在本模块，并复用 [`RoleMemorySnapshot`] 作为返回结构，让前端
//! "读 → 写 → 再读"走同一条路径，而不是各写一套。

use serde::Serialize;
use tauri::Manager;

use crate::AppState;
use crate::ai_service::game_system::persistent_memory_system::MemorySystemSnapshot;
use crate::ai_service::types::LlmMessage;

/// 角色选择器的一项。刻意只带廉价字段，不含计数与快照
/// （那些要锁 bank 并扫描台词历史，属于详情接口）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDebugRole {
    pub role_id: i32,
    pub display_name: Option<String>,
    /// 该角色的 `PersistentMemorySystem` 是否已创建。
    pub runtime_present: bool,
    /// 运行时缺失或不可用的原因；`None` = 运行时存在且已启用。
    pub runtime_note: Option<&'static str>,
    /// 运行时开关（全局 `use_persistent_memory` 在**创建时**的取值）。
    /// 运行时缺失时为 `None`。
    pub enabled: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDebugOverview {
    pub active_save_id: Option<i32>,
    /// `false` 表示当前是自由对话：压缩照常跑，但记忆只在内存里、**重启即失**。
    pub active_save_persistent: bool,
    pub current_role_id: Option<i32>,
    /// 角色清单是否来自有序的 `onstage_role_ids`；
    /// `false` = 回退到无序的 `present_role_ids`（已排序后返回）。
    pub roles_from_onstage: bool,
    pub roles: Vec<MemoryDebugRole>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleMemorySnapshot {
    pub role_id: i32,
    pub display_name: Option<String>,
    /// 运行时快照：存储真源 + 注入视图 + 压缩状态。
    /// 注意其中嵌套的 `GameMemoryBank` 与 `LlmMessage` 没有 `rename_all`，
    /// 键名保持 snake_case（如 `last_processed_global_idx`）。
    pub runtime: MemorySystemSnapshot,
    /// 组装好、**下一轮请求将以此为基础**的上下文。
    /// 它是"上一次同步时构建的"，因此不含刚发出的那条 user 消息。
    pub context: Vec<LlmMessage>,
}

/// 列出可检视的角色与当前存档上下文。
#[tauri::command]
pub async fn get_memory_debug_overview(
    app: tauri::AppHandle,
) -> Result<MemoryDebugOverview, String> {
    let state = app.state::<AppState>();
    let llm_ready = crate::ai_service::llm::slot_snapshot(&state.chat.llm)
        .await
        .is_some();

    let game_status = {
        let service = state.ai_service.lock().await;
        service.game_status.clone()
    };
    let gs = game_status.lock().await;

    let (role_ids, roles_from_onstage) = if !gs.onstage_role_ids.is_empty() {
        (gs.onstage_role_ids.clone(), true)
    } else {
        // present_role_ids 是 HashSet：必须排序，否则手动刷新时顺序会跳。
        let mut ids: Vec<i32> = gs.present_role_ids.iter().copied().collect();
        ids.sort_unstable();
        (ids, false)
    };

    let roles = role_ids
        .into_iter()
        .map(|role_id| {
            let display_name = gs
                .role_manager
                .get_loaded(role_id)
                .and_then(|role| role.display_name.clone());
            let enabled = gs.role_manager.memory_runtime_enabled(role_id);
            let runtime_note = match enabled {
                Some(true) => None,
                Some(false) => Some("global_switch_off"),
                // 运行时缺失：`ensure_memory_bank_system` 在 LLM 槽位为空时直接早退；
                // 另一种是该角色还没进过 `sync_memories` 的 involved_ids。
                None if !llm_ready => Some("no_llm_configured"),
                None => Some("not_synced_yet"),
            };
            MemoryDebugRole {
                role_id,
                display_name,
                runtime_present: enabled.is_some(),
                runtime_note,
                enabled,
            }
        })
        .collect();

    Ok(MemoryDebugOverview {
        active_save_id: gs.active_save_id,
        active_save_persistent: gs.active_save_id.is_some(),
        current_role_id: gs.current_role_id,
        roles_from_onstage,
        roles,
    })
}

/// 取某角色永久记忆的只读快照。
#[tauri::command]
pub async fn get_role_memory_snapshot(
    app: tauri::AppHandle,
    role_id: i32,
) -> Result<RoleMemorySnapshot, String> {
    let state = app.state::<AppState>();
    let game_status = {
        let service = state.ai_service.lock().await;
        service.game_status.clone()
    };
    let gs = game_status.lock().await;

    // 用 get_loaded（&self）。不要用 get_role：它会惰性注册角色并写库。
    let display_name = gs
        .role_manager
        .get_loaded(role_id)
        .and_then(|role| role.display_name.clone());

    // `memory_as_json` 是既有的 pub 方法，取的就是真正发给 LLM 的那份上下文。
    let context = gs.role_manager.memory_as_json(role_id).unwrap_or_default();

    let runtime = gs
        .role_manager
        .memory_debug(role_id, &gs.line_list)
        .await
        .ok_or_else(|| {
            format!(
                "角色 {role_id} 的永久记忆运行时尚未创建：通常是未配置 LLM\
                 （槽位为空时不会创建运行时），或该角色还没参与过任何台词"
            )
        })?;

    Ok(RoleMemorySnapshot {
        role_id,
        display_name,
        runtime,
        context,
    })
}
