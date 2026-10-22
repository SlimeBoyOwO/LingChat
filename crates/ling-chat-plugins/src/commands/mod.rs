//! 插件管理 Tauri 命令：列表、启停、配置、重载、删除与资源操作。
//!
//! 导入相关命令在 `import`；派生状态收敛在 crate 根的 `content_sync`。

pub mod import;

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager};

use ling_chat_main::AppState;

use super::{PluginInfo, PluginManager, PluginResourceEntry, ResourceKind};
use crate::content_sync::refresh_plugin_content;
use crate::signal::SIGNAL_LINE_SHOWN;

fn manager(app: &AppHandle) -> Arc<PluginManager> {
    app.state::<Arc<PluginManager>>().inner().clone()
}

/// 列出所有插件（含启停状态与配置 schema）。
#[tauri::command]
pub async fn plugin_list(app: AppHandle) -> Result<Vec<PluginInfo>, String> {
    Ok(manager(&app).list().await)
}

/// 启用/禁用插件。
///
/// 启用后会在后台跑该插件的启动入口：失败时插件会自行禁用并推 `plugin:auto-disabled`
/// 事件，所以这里不等它，直接返回。禁用时级联停掉依赖它的插件（前置没了它们跑不通）。
#[tauri::command]
pub async fn plugin_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    let plugin_manager = manager(&app);
    plugin_manager.set_enabled(&id, enabled).await?;
    if enabled {
        let app_handle = app.clone();
        let startup_manager = plugin_manager.clone();
        let startup_id = id.clone();
        tauri::async_runtime::spawn(async move {
            startup_manager
                .run_startup_hook_for(&app_handle, &startup_id)
                .await;
        });
    } else {
        plugin_manager.cascade_disable_dependents(&app, &id).await;
    }
    refresh_plugin_content(&app).await;
    Ok(())
}

/// 保存插件配置（表单填写的字段）。
#[tauri::command]
pub async fn plugin_save_config(
    app: AppHandle,
    id: String,
    config: HashMap<String, JsonValue>,
) -> Result<(), String> {
    manager(&app).save_config(&id, config).await
}

/// 当前是否有插件接管了台词语音的播放（前端据此把本机语音静音）。
///
/// 断线、禁用插件或关掉设置项后自动变回 false，下一次播放就回到本机。
#[tauri::command]
pub async fn plugin_voice_playback_active(app: AppHandle) -> Result<bool, String> {
    Ok(manager(&app).voice_playback_owner().await.is_some())
}

/// 主界面开始展示某句台词时由前端调用：派发给订阅了 `line_shown` 的插件。
#[tauri::command]
pub async fn plugin_line_shown(app: AppHandle, line: JsonValue) -> Result<(), String> {
    let plugin_manager = manager(&app);
    if !plugin_manager.has_signal_subscribers(SIGNAL_LINE_SHOWN) {
        return Ok(());
    }
    plugin_manager
        .dispatch_signal(&app, SIGNAL_LINE_SHOWN, &line, None)
        .await;
    Ok(())
}

/// 重新扫描插件目录（异步，避免阻塞调用线程）。
#[tauri::command]
pub async fn plugin_reload(app: AppHandle) -> Result<(), String> {
    let plugin_manager = manager(&app);
    let reload_manager = plugin_manager.clone();
    tokio::task::spawn_blocking(move || reload_manager.reload())
        .await
        .map_err(|e| format!("插件重载线程异常: {e}"))?;
    refresh_plugin_content(&app).await;
    // 重载等于按 state.json 把插件重新起一遍：旧令牌作废、记录重建，所以启动入口
    // 也得跟着重跑——否则插件的初始化状态丢了，而用户没有别的入口补跑。与启动路径
    // 一致，不等它跑完（见 plugin_set_enabled 的说明）。
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        plugin_manager.run_startup_hooks(&app_handle).await;
    });
    Ok(())
}

/// 删除插件（含插件目录与状态记录）。
#[tauri::command]
pub async fn plugin_delete(app: AppHandle, id: String) -> Result<(), String> {
    let plugin_manager = manager(&app);
    plugin_manager.delete_plugin(&id).await?;
    // 前置被卸载，依赖它的插件已跑不通，一并停掉。
    plugin_manager.cascade_disable_dependents(&app, &id).await;
    refresh_plugin_content(&app).await;
    Ok(())
}

// ========== 插件携带资源 ==========

/// 某插件的全部资源条目（供插件管理页资源区），带 conflict / hidden 标记。
#[tauri::command]
pub async fn plugin_resources(
    app: AppHandle,
    plugin_id: String,
) -> Result<Vec<PluginResourceEntry>, String> {
    manager(&app).plugin_resources(&plugin_id).await
}

/// 软删除：隐藏某插件资源（列表不再显示，文件保留）。
#[tauri::command]
pub async fn plugin_resource_hide(
    app: AppHandle,
    plugin_id: String,
    key: String,
) -> Result<(), String> {
    manager(&app)
        .set_resource_hidden(&plugin_id, &key, true)
        .await?;
    refresh_plugin_content(&app).await;
    Ok(())
}

/// 恢复被隐藏的插件资源。
#[tauri::command]
pub async fn plugin_resource_restore(
    app: AppHandle,
    plugin_id: String,
    key: String,
) -> Result<(), String> {
    manager(&app)
        .set_resource_hidden(&plugin_id, &key, false)
        .await?;
    refresh_plugin_content(&app).await;
    Ok(())
}

/// 保留：把插件资源复制到游戏对应目录，成功后自动隐藏插件版。
#[tauri::command]
pub async fn plugin_resource_keep(
    app: AppHandle,
    plugin_id: String,
    key: String,
) -> Result<(), String> {
    let kind = manager(&app).keep_resource(&plugin_id, &key).await?;
    refresh_plugin_content(&app).await;
    // 保留 = 复制进游戏目录成为游戏自有资源，游戏侧需要即时重扫才能看到：
    // 角色入 role 表、剧本进 ScriptManager（图/音列表实时读目录，无需重扫）。
    match kind {
        ResourceKind::Characters => {
            let state = app.state::<AppState>();
            ling_chat_main::db::role_sync::sync_roles_from_folder(
                &state.db,
                &ling_chat_main::api::data_dir(),
            )
            .await
            .map_err(|e| e.to_string())?;
        },
        ResourceKind::Scripts => {
            let _ = ling_chat_main::api::script_editor::editor_rescan_scripts(app.clone()).await;
        },
        _ => {},
    }
    Ok(())
}
