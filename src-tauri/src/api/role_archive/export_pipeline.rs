//! 角色压缩包导出的内部实现：压缩角色目录到临时文件，并清洗用户输入的文件名。
//!
//! `compress_role_to_temp` 由 `mod.rs` 中的 `export_role_to_path` 调用。

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};

use crate::utils::archive::{self, ArchiveFormat, EntryEvent};



use crate::db::entities::role::Entity as RoleEntity;

// ===== 导出命令 =====

/// 把角色压缩到缓存目录，并返回临时路径、建议文件名和文件大小。
pub(super) async fn compress_role_to_temp(
    app: &AppHandle,
    role_id: i32,
    format: ArchiveFormat,
) -> Result<(PathBuf, String, u64), String> {
    use sea_orm::EntityTrait;
    tracing::info!("[RoleArchive] compress_role_to_temp 开始: role_id={}, format={:?}", role_id, format);
    let db = app.state::<crate::AppState>().db.clone();

    let role = RoleEntity::find_by_id(role_id)
        .one(&db)
        .await
        .map_err(|e| format!("query role: {e}"))?
        .ok_or_else(|| format!("role #{role_id} not found"))?;

    let folder = role
        .resource_folder
        .clone()
        .ok_or_else(|| format!("role #{role_id} has no resource_folder"))?;

    let characters_root = crate::api::characters_dir();
    let src_dir = characters_root.join(&folder);
    if !src_dir.is_dir() {
        return Err(format!("role folder not found: {}", src_dir.display()));
    }

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cache dir: {e}"))?;
    let exports_root = cache_dir.join("exports");
    tokio::fs::create_dir_all(&exports_root)
        .await
        .map_err(|e| format!("create exports dir: {e}"))?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let safe_name = sanitize_file_name(&role.name);
    let suggested_name = format!("{safe_name}_{ts}.{}", format.as_str());
    let out_path = exports_root.join(&suggested_name);

    let arc_path = out_path.clone();
    let src_path = src_dir.clone();
    let fmt = format;
    let app_for_emit = app.clone();
    tokio::task::spawn_blocking(move || {
        let on_entry = |evt: EntryEvent| {
            // 发送导出进度事件，前端可复用现有进度条展示。
            let _ = app_for_emit.emit("role:export-progress", &evt);
        };
        archive::compress(&src_path, fmt, &arc_path, &on_entry)
    })
    .await
    .map_err(|e| format!("spawn_blocking join: {e}"))?
    .map_err(|e| e.to_string())?;

    let metadata = tokio::fs::metadata(&out_path)
        .await
        .map_err(|e| format!("stat output: {e}"))?;

    tracing::info!(
        "[RoleArchive] compress_role_to_temp 完成: temp_path={}, suggested_name={}, size={}B ({}MB)",
        out_path.display(),
        suggested_name,
        metadata.len(),
        metadata.len() / 1024 / 1024
    );

    Ok((out_path, suggested_name, metadata.len()))
}


fn sanitize_file_name(name: &str) -> String {
    let chars: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = chars.trim();
    if trimmed.is_empty() {
        "role".to_string()
    } else {
        trimmed.to_string()
    }
}
