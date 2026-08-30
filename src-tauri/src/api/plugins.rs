//! Plugin management Tauri commands.
//!
//! 插件的列表、启停、配置保存，以及插件携带资源（人物/剧本/音乐/背景图/环境音）的
//! 查询、隐藏（软删除）、恢复、保留（复制到游戏目录）。业务逻辑在 `plugins::PluginManager`。

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::ai_service::game_system::scene_store::{Scene, SceneStore};
use crate::api::encode_plugin_folder;
use crate::init::role_sync::PluginRoleInput;
use crate::plugins::{PluginInfo, PluginManager, PluginResourceEntry, ResourceKind};
use crate::utils::archive::{self, ArchiveImportState, ConflictPolicy};
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
pub async fn plugin_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    manager(&app).set_enabled(&id, enabled).await?;
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

/// 重新扫描插件目录（异步，避免阻塞调用线程）。
#[tauri::command]
pub async fn plugin_reload(app: AppHandle) -> Result<(), String> {
    let manager = manager(&app);
    tokio::task::spawn_blocking(move || manager.reload())
        .await
        .map_err(|e| format!("插件重载线程异常: {e}"))?;
    refresh_plugin_content(&app).await;
    Ok(())
}

/// 删除插件（含插件目录与状态记录）。
#[tauri::command]
pub async fn plugin_delete(app: AppHandle, id: String) -> Result<(), String> {
    manager(&app).delete_plugin(&id).await?;
    refresh_plugin_content(&app).await;
    Ok(())
}

// ========== 插件压缩包导入 ==========

/// 把 `conflict` 解析为插件允许的冲突策略。
///
/// 插件目录名必须等于 `manifest.id`，改名会让 `PluginManager` 直接拒绝加载，
/// 因此这里只接受「覆盖」与「放弃」，`rename` 视为参数错误。
fn parse_plugin_policy(s: &str) -> Result<ConflictPolicy, String> {
    match s {
        "overwrite" => Ok(ConflictPolicy::Overwrite),
        "abort" | "skip" => Ok(ConflictPolicy::Skip),
        other => Err(format!("插件不支持的 conflict: {other}")),
    }
}

/// 从桌面文件路径或 Android SAF 内容 URI 导入插件压缩包（zip / 7z）。
#[tauri::command]
pub async fn import_plugin_from_path(
    app: AppHandle,
    state: State<'_, ArchiveImportState>,
    path: String,
    format: Option<String>,
    conflict: String,
) -> Result<crate::plugins::importer::PluginImportResult, String> {
    // 与角色导入共用同一把全局并发锁：同一时刻只允许一个解压任务。
    if state
        .importing
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return Err("已有导入任务在进行中".into());
    }
    let _import_guard = archive::ImportingGuard {
        flag: &state.importing,
    };

    if path.is_empty() {
        return Err("path 为空".into());
    }
    let format = match format.as_deref() {
        None | Some("") | Some("auto") => None,
        Some("zip") => Some(archive::ArchiveFormat::Zip),
        Some("7z") => Some(archive::ArchiveFormat::SevenZ),
        Some(other) => return Err(format!("不支持的 format: {other}")),
    };
    let policy = parse_plugin_policy(&conflict)?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let cancel_token = state.register_task(&task_id);
    let _remove_guard = archive::TaskRemoveGuard {
        state: &state,
        task_id: &task_id,
    };
    // 前端取消按钮需要 task_id 才能定位到正确的取消令牌。
    let _ = app.emit(
        "plugin:import-started",
        serde_json::json!({ "task_id": &task_id }),
    );

    tracing::info!(
        "[PluginImport] import_plugin_from_path 开始: path={path}, format={format:?}, policy={policy:?}"
    );
    let src = crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &path)
        .await
        .map_err(|e| format!("准备导入源: {e}"))?;
    if src.cleanup_after_import {
        state.set_saf_cache(&task_id, src.path.clone());
    }

    let result = async {
        if !src.path.exists() {
            return Err(format!("文件不存在: {}", src.path.display()));
        }
        crate::plugins::importer::do_import_plugin(
            &app,
            &src.path,
            format,
            policy,
            cancel_token,
        )
        .await
    }
    .await;

    match &result {
        Ok(r) => tracing::info!(
            "[PluginImport] 完成: id={}, name={}, action={}",
            r.plugin_id,
            r.plugin_name,
            r.conflict_action
        ),
        Err(e) => tracing::error!("[PluginImport] 失败: {e}"),
    }
    result
}

/// 取消正在进行的插件导入。
#[tauri::command]
pub async fn cancel_plugin_import(
    task_id: String,
    state: State<'_, ArchiveImportState>,
) -> Result<(), String> {
    tracing::info!("[PluginImport] cancel_plugin_import: task_id={task_id}");
    state.cancel_task(&task_id);
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
    manager(&app).set_resource_hidden(&plugin_id, &key, true).await?;
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
    manager(&app).set_resource_hidden(&plugin_id, &key, false).await?;
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
            crate::init::role_sync::sync_roles_from_folder(&state.db, &crate::api::data_dir())
                .await
                .map_err(|e| e.to_string())?;
        }
        ResourceKind::Scripts => {
            let _ =
                crate::api::script_editor::editor_rescan_scripts(app.clone()).await;
        }
        _ => {}
    }
    Ok(())
}

/// 插件资源变更后重建三类派生状态：插件角色入库、插件剧本合并、插件背景场景化。
///
/// 这是「运行时直读 + 软删除」模型下唯一的收敛点：任何影响可见资源集合的操作
/// （启停 / 重载 / 删除 / 隐藏 / 恢复 / 保留）之后调用一次即可。
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
    crate::init::role_sync::sync_plugin_roles(&state.db, &inputs)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn collect_plugin_role_inputs(state: &AppState) -> Vec<PluginRoleInput> {
    let entries = state
        .data()
        .plugin_manager
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
        .plugin_manager
        .visible_file_entries(ResourceKind::Scripts)
        .await;
    let plugin_scripts: Vec<(String, std::path::PathBuf)> = entries
        .into_iter()
        .map(|e| (e.plugin_id, e.path))
        .collect();
    let mut service = state.ai_service.lock().await;
    service.script_manager.apply_plugin_scripts(&plugin_scripts);
}

/// 把可见插件背景图同步为 scenes.json 中的插件场景（增删对齐）。
async fn sync_plugin_scenes_cmd(app: &AppHandle) {
    let state = app.state::<AppState>();
    let entries = state
        .data()
        .plugin_manager
        .visible_file_entries(ResourceKind::Backgrounds)
        .await;
    let data_dir = crate::api::data_dir();
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
            created_at: String::new(),
            updated_at: String::new(),
            plugin_id: Some(e.plugin_id.clone()),
        });
    }
    if let Err(err) = store.save_all(&scenes) {
        tracing::warn!("[PluginResources] 写入插件场景失败: {err}");
    }
}
