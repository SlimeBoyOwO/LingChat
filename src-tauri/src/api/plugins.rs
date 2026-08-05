//! Plugin management Tauri commands.
//!
//! 插件的列表、启停、配置保存。业务逻辑在 `plugins::PluginManager`。

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager};

use crate::plugins::{PluginInfo, PluginManager};
use crate::AppState;

fn manager(app: &AppHandle) -> Arc<PluginManager> {
    app.state::<AppState>().data().plugin_manager.clone()
}

/// 列出所有插件（含启停状态与配置 schema）。
#[tauri::command]
pub async fn plugin_list(app: AppHandle) -> Result<Vec<PluginInfo>, String> {
    Ok(manager(&app).list().await)
}

/// 启用/禁用插件。
#[tauri::command]
pub async fn plugin_set_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    manager(&app).set_enabled(&id, enabled).await
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

/// 重新扫描插件目录（异步，避免阻塞调用线程）。
#[tauri::command]
pub async fn plugin_reload(app: AppHandle) -> Result<(), String> {
    let manager = manager(&app);
    tokio::task::spawn_blocking(move || manager.reload())
        .await
        .map_err(|e| format!("插件重载线程异常: {e}"))?;
    Ok(())
}

/// 删除插件（含插件目录与状态记录）。
#[tauri::command]
pub async fn plugin_delete(app: AppHandle, id: String) -> Result<(), String> {
    manager(&app).delete_plugin(&id).await
}
