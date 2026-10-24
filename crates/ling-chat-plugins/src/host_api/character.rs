//! 角色与界面控制：切换角色、当前名字、自动模式、回复长度。

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine};
use tauri::{AppHandle, Emitter, Manager};

use ling_chat_main::AppState;
use ling_chat_main::ai_service::tools::game_status_handle;
use ling_chat_main::db::managers::role_repo::RoleRepo;

use super::{no_host, runtime, value_to_pyobject};

/// 取已加载角色的展示名（`settings.ai_name`，即对话里显示的名字）。
///
/// 锁顺序必须 `ai_service` → `game_status`（见 `ai_service::tools` 的约定）。
async fn loaded_display_name(app: &AppHandle, role_id: i32) -> Option<String> {
    let state = app.state::<AppState>();
    let gs = {
        let service = state.ai_service.lock().await;
        service.game_status.clone()
    };
    let gs = gs.lock().await;
    gs.role_manager
        .get_loaded(role_id)
        .and_then(|role| role.display_name.clone())
}

/// `switch_character` 的实现：完整切换当前角色。
///
/// 直接复用玩家侧的 `select_character`——它就是「正常切换」的全部语义：清空对话
/// 上下文、重置角色内存、递增会话边界代号丢弃旧流，最后产出整份 `WebInitData`。
/// 与 LLM 工具 `character_switch`（只换角色、保留历史）是两回事。
///
/// 插件调用没有前端调用方，所以额外把 `WebInitData` 广播给前端；不广播的话后端
/// 已经换了角色，前端还停在旧角色上。
async fn switch_character_impl(app: AppHandle, role_id: i32) -> serde_json::Value {
    // 先确认角色存在再动手：select_character 是先 clear_game_status() 再加载角色，
    // 传一个不存在的 id 会**先把对话清空**、然后才报错。玩家路径只会传合法 id，
    // 插件传的是脚本里的任意整数，所以这道校验必须前置。
    // 顺带取出角色标题（DB 的 role.name，即 settings.yml 的 title）。
    let title = {
        let state = app.state::<AppState>();
        match RoleRepo::get_role_by_id(&state.db, role_id).await {
            Ok(Some(role)) => role.name,
            Ok(None) => {
                return serde_json::json!({ "ok": false, "error": format!("角色 id {role_id} 不存在") });
            },
            Err(e) => {
                return serde_json::json!({ "ok": false, "error": format!("查询角色失败: {e}") });
            },
        }
    };

    match ling_chat_main::api::game::select_character(app.clone(), role_id).await {
        Ok(init) => {
            // name 回对话里显示的 AI 名称（settings.yml 的 ai_name），与 LLM 工具
            // `character_switch` 一致；title 是角色标题（角色卡列表页那行大字）。
            // 界面上两处分别用它们，所以都给出来，插件不用再去读 settings.yml。
            let name = loaded_display_name(&app, role_id)
                .await
                .unwrap_or_else(|| title.clone());
            if let Err(e) = app.emit("character:full-switch", &init) {
                tracing::warn!("emit character:full-switch 失败: {e}");
            }
            serde_json::json!({ "ok": true, "role_id": role_id, "name": name, "title": title })
        },
        Err(e) => serde_json::json!({ "ok": false, "error": format!("切换角色失败: {e}") }),
    }
}

/// 当前角色与玩家的显示名。插件写旁白时用这两个名字做第三人称称呼，
/// 不再用「你 / 玩家」这类泛指。
async fn current_names_impl(app: &AppHandle) -> serde_json::Value {
    let gs = game_status_handle(app).await;
    let gs = gs.lock().await;
    let role_id = gs.current_role_id;
    let ai_name = role_id
        .and_then(|id| gs.role_manager.get_loaded(id))
        .map(|role| {
            role.display_name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| role.settings.ai_name.clone())
        })
        .unwrap_or_default();
    serde_json::json!({
        "ok": true,
        "role_id": role_id,
        "ai_name": ai_name,
        "user_name": gs.player.user_name,
    })
}

/// 把主界面的「自动模式」切到指定状态。
///
/// 模式的持有者是前端——Pinia 里那个 flag 与推进定时器都在那儿，宿主改不了，所以这里
/// 只负责广播，实际切换由前端完成（与 `switch_character` 同款）。
fn set_auto_mode_impl(app: &AppHandle, enabled: bool) -> serde_json::Value {
    match app.emit(
        "plugin:auto-mode",
        serde_json::json!({ "enabled": enabled }),
    ) {
        Ok(()) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "ok": false, "error": format!("广播自动模式失败: {e}") }),
    }
}

pub(super) fn switch_character(role_id: i32, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(value_to_pyobject(
            vm,
            &serde_json::json!({ "ok": false, "error": "宿主句柄未初始化" }),
        ));
    };
    let result = runtime().block_on(switch_character_impl(app, role_id));
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn current_names(vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = runtime().block_on(current_names_impl(&app));
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn set_auto_mode(enabled: bool, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = set_auto_mode_impl(&app, enabled);
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn set_reply_length_mode(mode: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = runtime().block_on(async move {
        match ling_chat_main::api::chat::set_reply_length_mode(app, mode).await {
            Ok(mode) => serde_json::json!({ "ok": true, "mode": mode }),
            Err(error) => serde_json::json!({ "ok": false, "error": error }),
        }
    });
    Ok(value_to_pyobject(vm, &result))
}
