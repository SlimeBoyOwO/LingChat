//! 角色压缩包导入/导出 Tauri 命令。
//!
//! `import_role_from_path` 同时支持桌面文件路径和 Android SAF 内容 URI。
//! Android 压缩包由后端复制到应用缓存后再解压，避免通过前端 IPC
//! 传递整包字节，并且不设置压缩包的绝对大小限制。

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;


mod export_pipeline;
mod import_pipeline;
mod state;

pub use state::{ImportTaskEntry, RoleArchiveState};

use export_pipeline::compress_role_to_temp;
use import_pipeline::{do_import, parse_format, parse_policy, prepare_import_source, write_temp_archive};
use state::{ImportingGuard, TaskRemoveGuard};

#[derive(Debug, Serialize, Clone)]
pub struct ImportResult {
    pub role_id: Option<i32>,
    pub role_name: String,
    pub conflict_action: String,
    pub warnings: Vec<String>,
    pub bytes_extracted: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExportResult {
    pub temp_path: String,
    pub suggested_name: String,
    pub size_bytes: u64,
}

// ===== 导入命令 =====

#[tauri::command]
pub async fn import_role(
    app: AppHandle,
    state: State<'_, RoleArchiveState>,
    bytes: Vec<u8>,
    format: String,
    conflict: String,
    file_name: Option<String>,
) -> Result<ImportResult, String> {
    // 并发保护：同一时间只允许一个导入任务。
    if state.importing.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("已有导入任务在进行中".into());
    }
    let _import_guard = ImportingGuard { flag: &state.importing };

    if bytes.is_empty() {
        tracing::warn!("[RoleArchive] import_role 收到空文件");
        return Err("空文件".into());
    }
    let format = parse_format(&format)?;
    let policy = parse_policy(&conflict)?;
    tracing::info!(
        "[RoleArchive] import_role 开始: format={:?}, conflict={:?}, size={}B ({}MB)",
        format,
        policy,
        bytes.len(),
        bytes.len() / 1024 / 1024
    );

    // 为每个导入任务分配独立的取消令牌。
    let task_id = uuid::Uuid::new_v4().to_string();
    let cancel_token = Arc::new(CancellationToken::new());
    state.tasks.lock().unwrap().insert(
        task_id.clone(),
        ImportTaskEntry {
            cancel_token: cancel_token.clone(),
            saf_cache_path: std::sync::Mutex::new(None),
        },
    );
    let _remove_guard = TaskRemoveGuard { state: &state, task_id: &task_id };
    // 把 task_id 通过事件发给前端，让前端的取消按钮能找到正确的令牌。
    let _ = app.emit("role:import-started", serde_json::json!({ "task_id": &task_id }));
    // 写入临时文件，供文件头校验和 ZIP/7z 解压库读取。
    let tmp_path = write_temp_archive(&app, &bytes).await?;
    let cleanup_path = tmp_path.clone();

    let result = do_import(&app, &tmp_path, format, policy, cancel_token, file_name.as_deref()).await;

    // 兜底清理临时文件
    let _ = tokio::fs::remove_file(&cleanup_path).await;

    match &result {
        Ok(r) => tracing::info!(
            "[RoleArchive] import_role 完成: role_name={}, role_id={:?}, action={}, bytes_extracted={}",
            r.role_name, r.role_id, r.conflict_action, r.bytes_extracted
        ),
        Err(e) => tracing::error!("[RoleArchive] import_role 失败: {e}"),
    }
    if result.is_ok() {
        let _ = app.emit("role:list-updated", ());
    }
    result
}

/// 取消正在进行的导入。

/// 取消正在进行的导入。
#[tauri::command]
pub async fn cancel_role_import(
    task_id: String,
    state: State<'_, RoleArchiveState>,
) -> Result<(), String> {
    tracing::info!(
        "[RoleArchive] cancel_role_import 收到取消: task_id={}",
        task_id
    );
    let entry = state.tasks.lock().unwrap().remove(&task_id);
    if let Some(entry) = entry {
        entry.cancel_token.cancel();
        // 取消时立即清理 SAF 缓存，不等待 `do_import` 执行结束。
        let cached_path = entry.saf_cache_path.lock().unwrap().take();
        if let Some(path) = cached_path {
            tracing::info!("[RoleArchive] cancel 清理 SAF 缓存: {}", path.display());
            let _ = tokio::fs::remove_file(&path).await;
        }
    } else {
        tracing::warn!(
            "[RoleArchive] cancel_role_import 未找到 task_id={}",
            task_id
        );
    }
    Ok(())
}

/// 从桌面文件路径或 Android SAF 内容 URI 导入角色。
#[tauri::command]
pub async fn import_role_from_path(
    app: AppHandle,
    state: State<'_, RoleArchiveState>,
    path: String,
    format: String,
    conflict: String,
    file_name: Option<String>,
) -> Result<ImportResult, String> {
    // 并发保护：同一时间只允许一个导入任务。
    if state.importing.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("已有导入任务在进行中".into());
    }
    let _import_guard = ImportingGuard { flag: &state.importing };

    if path.is_empty() {
        tracing::warn!("[RoleArchive] import_role_from_path 收到空 path");
        return Err("path 为空".into());
    }
    let format = parse_format(&format)?;
    let policy = parse_policy(&conflict)?;
    tracing::info!(
        "[RoleArchive] import_role_from_path 开始: path={}, format={:?}, conflict={:?}",
        path, format, policy
    );

    // 为每个导入任务分配独立的取消令牌。
    let task_id = uuid::Uuid::new_v4().to_string();
    let cancel_token = Arc::new(CancellationToken::new());
    let entry = ImportTaskEntry {
        cancel_token: cancel_token.clone(),
        saf_cache_path: std::sync::Mutex::new(None),
    };
    state.tasks.lock().unwrap().insert(task_id.clone(), entry);
    let _remove_guard = TaskRemoveGuard { state: &state, task_id: &task_id };

    // 把 task_id 通过事件发给前端，让前端的取消按钮能找到正确的令牌。
    let _ = app.emit("role:import-started", serde_json::json!({ "task_id": &task_id }));

    let (path_buf, cleanup_after_import) = prepare_import_source(&app, &path).await?;

    // SAF 源文件复制完成后记录缓存路径，便于取消任务时立即清理。
    if cleanup_after_import {
        if let Some(entry) = state.tasks.lock().unwrap().get_mut(&task_id) {
            *entry.saf_cache_path.lock().unwrap() = Some(path_buf.clone());
        }
    }

    let result = async {
        if !path_buf.exists() {
            return Err(format!("文件不存在: {}", path_buf.display()));
        }
        let meta = tokio::fs::metadata(&path_buf)
            .await
            .map_err(|e| format!("stat path: {e}"))?;
        tracing::info!(
            "[RoleArchive] import_role_from_path 文件大小: {}B ({}MB)",
            meta.len(),
            meta.len() / 1024 / 1024
        );
        do_import(
            &app,
            &path_buf,
            format,
            policy,
            cancel_token,
            file_name.as_deref(),
        )
        .await
    }
    .await;

    if cleanup_after_import {
        if let Err(error) = tokio::fs::remove_file(&path_buf).await {
            tracing::warn!(
                "[RoleArchive] import_role_from_path 清理 SAF 缓存失败: path={}, err={}",
                path_buf.display(),
                error
            );
        }
    }
    match &result {
        Ok(r) => tracing::info!(
            "[RoleArchive] import_role_from_path 完成: role_name={}, role_id={:?}, action={}",
            r.role_name, r.role_id, r.conflict_action
        ),
        Err(e) => tracing::error!("[RoleArchive] import_role_from_path 失败: {e}"),
    }
    if result.is_ok() {
        let _ = app.emit("role:list-updated", ());
    }
    result
}

// ===== 导出命令 =====

#[tauri::command]
pub async fn export_role(
    app: AppHandle,
    role_id: i32,
    format: String,
) -> Result<ExportResult, String> {
    let format = parse_format(&format)?;
    let (out_path, suggested_name, size) = compress_role_to_temp(&app, role_id, format).await?;
    Ok(ExportResult {
        temp_path: out_path.to_string_lossy().into_owned(),
        suggested_name,
        size_bytes: size,
    })
}

#[tauri::command]
pub async fn export_role_to_path(
    app: AppHandle,
    role_id: i32,
    format: String,
    dest_path: String,
) -> Result<ExportResult, String> {
    let format = parse_format(&format)?;
    if dest_path.is_empty() {
        return Err("dest_path 为空".into());
    }
    tracing::info!(
        "[RoleArchive] export_role_to_path 开始: role_id={}, format={:?}, dest={}",
        role_id, format, dest_path
    );

    let (temp_path, suggested_name, size) =
        compress_role_to_temp(&app, role_id, format).await?;

    if dest_path.starts_with("content://") {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};

        let source_uri = FsUri::from_path(&temp_path);
        let destination_uri = FsUri::from_uri(dest_path.clone());
        tracing::info!(
            "[RoleArchive] export_role_to_path SAF copy: temp={} -> dest={}",
            temp_path.display(),
            dest_path
        );

        let copy_result = app
            .android_fs_async()
            .copy(&source_uri, &destination_uri)
            .await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        copy_result.map_err(|e| format!("copy to SAF destination: {e}"))?;

        tracing::info!(
            "[RoleArchive] export_role_to_path SAF completed: dest={}, size={}B ({}MB)",
            dest_path,
            size,
            size / 1024 / 1024
        );
        return Ok(ExportResult {
            temp_path: dest_path,
            suggested_name,
            size_bytes: size,
        });
    }

    // 桌面端使用后端原生复制，不受 Tauri 文件系统权限范围约束。
    let dest = PathBuf::from(&dest_path);
    // 确保目标父目录存在
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create dest parent dir: {e}"))?;
    }

    let temp_clone = temp_path.clone();
    let dest_clone = dest.clone();
    tokio::task::spawn_blocking(move || std::fs::copy(&temp_clone, &dest_clone))
        .await
        .map_err(|e| format!("spawn_blocking join: {e}"))?
        .map_err(|e| format!("copy to dest: {e}"))?;

    // 删除临时文件
    let _ = tokio::fs::remove_file(&temp_path).await;

    tracing::info!(
        "[RoleArchive] export_role_to_path 完成: dest={}, size={}B ({}MB)",
        dest.display(),
        size,
        size / 1024 / 1024
    );

    Ok(ExportResult {
        temp_path: dest.to_string_lossy().into_owned(),
        suggested_name,
        size_bytes: size,
    })
}

#[tauri::command]
pub async fn rescan_roles(app: AppHandle) -> Result<Vec<i32>, String> {
    tracing::info!("[RoleArchive] rescan_roles 开始");
    let data_dir = crate::init::static_copy::get_data_dir().clone();
    let db = app.state::<crate::AppState>().db.clone();
    let ids = crate::init::role_sync::sync_roles_from_folder(&db, &data_dir)
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!("[RoleArchive] rescan_roles 完成: 同步 {} 个角色", ids.len());
    let _ = app.emit("role:list-updated", ());
    Ok(ids)
}
