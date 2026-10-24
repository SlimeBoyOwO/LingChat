//! 插件压缩包导入与取消。

use tauri::{AppHandle, Emitter, State};

use ling_chat_main::utils::archive::{self, ArchiveImportState, ConflictPolicy};

use crate::importer;

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
) -> Result<importer::PluginImportResult, String> {
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
    let src =
        ling_chat_main::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &path)
            .await
            .map_err(|e| format!("准备导入源: {e}"))?;
    if src.cleanup_after_import {
        state.set_saf_cache(&task_id, src.path.clone());
    }

    let result = async {
        if !src.path.exists() {
            return Err(format!("文件不存在: {}", src.path.display()));
        }
        importer::do_import_plugin(&app, &src.path, format, policy, cancel_token).await
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
