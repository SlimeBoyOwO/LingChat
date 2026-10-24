//! 插件资源变更后的宿主派生状态收敛。
//!
//! 这是「运行时直读 + 软删除」模型下唯一的收敛点：任何影响可见资源集合的操作
//! （启停 / 重载 / 删除 / 隐藏 / 恢复 / 保留）之后调用一次即可。
//!
//! 放在 crate 根而不是 `commands` 下：它不是 IPC 命令，导入流程与后台启动也会调用，
//! 这样 `importer` 不必反过来依赖命令层。

use tauri::{AppHandle, Emitter, Manager};

use ling_chat_main::AppState;
use ling_chat_main::ai_service::game_system::scene_store::{Scene, SceneStore};
use ling_chat_main::api::encode_plugin_folder;
use ling_chat_main::db::role_sync::PluginRoleInput;

use crate::ResourceKind;

/// 重建三类派生状态：插件角色入库、插件剧本合并、插件背景场景化。
pub async fn refresh_plugin_content(app: &AppHandle) {
    if let Err(e) = sync_plugin_roles_cmd(app).await {
        tracing::warn!("[PluginResources] 同步插件角色失败: {e}");
    }
    sync_plugin_scripts_cmd(app).await;
    sync_plugin_scenes_cmd(app).await;
    let _ = app.emit("role:list-updated", ());
}

/// 收集当前所有可见插件角色并同步进 DB。
async fn sync_plugin_roles_cmd(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let inputs = collect_plugin_role_inputs(&state).await;
    ling_chat_main::db::role_sync::sync_plugin_roles(&state.db, &inputs)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn collect_plugin_role_inputs(state: &AppState) -> Vec<PluginRoleInput> {
    let entries = state
        .data()
        .plugin_resources
        .visible_file_entries(ResourceKind::Characters)
        .await;
    entries
        .into_iter()
        .map(|e| PluginRoleInput {
            encoded_folder: encode_plugin_folder(&e.plugin_id, &e.key),
            title: e.name,
        })
        .collect()
}

/// 重建 script_manager 中的插件剧本部分。
async fn sync_plugin_scripts_cmd(app: &AppHandle) {
    let state = app.state::<AppState>();
    let entries = state
        .data()
        .plugin_resources
        .visible_file_entries(ResourceKind::Scripts)
        .await;
    let plugin_scripts: Vec<(String, std::path::PathBuf)> =
        entries.into_iter().map(|e| (e.plugin_id, e.path)).collect();
    let mut service = state.ai_service.lock().await;
    service.script_manager.apply_plugin_scripts(&plugin_scripts);
}

/// 把可见插件背景图同步为 scenes.json 中的插件场景（增删对齐）。
async fn sync_plugin_scenes_cmd(app: &AppHandle) {
    let state = app.state::<AppState>();
    let entries = state
        .data()
        .plugin_resources
        .visible_file_entries(ResourceKind::Backgrounds)
        .await;
    let data_dir = ling_chat_main::api::data_dir();
    let store = SceneStore::new(&data_dir);
    let Ok(mut scenes) = store.load_all() else {
        return;
    };
    // 先移除所有旧的插件场景，再按当前可见集合重建（幂等）
    scenes.retain(|s| s.plugin_id.is_none());
    for e in entries {
        scenes.push(Scene {
            id: format!("plugin:{}/{}", e.plugin_id, e.key),
            name: e.name.clone(),
            description: String::new(),
            background: e.path.to_string_lossy().into_owned(),
            lighting: None,
            category: None,
            created_at: String::new(),
            updated_at: String::new(),
            plugin_id: Some(e.plugin_id.clone()),
            // 插件自动注册的场景无室内外标注，缺省 Unknown（None）
            location_type: None,
        });
    }
    if let Err(err) = store.save_all(&scenes) {
        tracing::warn!("[PluginResources] 写入插件场景失败: {err}");
    }
}
