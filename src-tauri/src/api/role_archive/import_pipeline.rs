//! 角色压缩包导入的内部实现：检测格式、解压、目录定位、冲突处理。
//!
//! 该模块不直接注册 Tauri 命令；上层 `mod.rs` 中的命令函数负责提取/校验
//! 用户参数并调用 `pub(super)` 暴露的实现。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use crate::db::entities::role::{Column, Entity as RoleEntity};
use crate::utils::archive::{
    self, ArchiveError, ArchiveFormat, ConflictPolicy, EntryEvent, ExtractSummary,
};

use super::ImportResult;

pub(super) async fn do_import(
    app: &AppHandle,
    tmp_path: &Path,
    format: ArchiveFormat,
    policy: ConflictPolicy,
    cancel_token: Arc<CancellationToken>,
    file_name: Option<&str>,
) -> Result<ImportResult, String> {
    // 1. 校验文件头魔数。
    let detected = archive::detect_format(tmp_path).map_err(|e| e.to_string())?;
    if detected != format {
        tracing::warn!("[RoleArchive] do_import 格式不匹配: 前端 {format:?}, 实际 {detected:?}");
        return Err(format!(
            "格式不匹配: 前端传 {format:?}, 实际 {detected:?}"
        ));
    }

    // 2. 使用去除扩展名后的压缩包文件名作为角色文件夹名。
    let final_name = sanitize_role_folder_name(file_name, None);
    tracing::info!("[RoleArchive] do_import 文件夹名: final_name={} (file_name={:?})", final_name, file_name);

    // 3. 在角色目录下创建本次导入使用的临时暂存目录。
    let characters_root = crate::api::characters_dir();
    tokio::fs::create_dir_all(&characters_root)
        .await
        .map_err(|e| format!("创建 characters dir: {e}"))?;
    let staging_id = uuid::Uuid::new_v4().to_string();
    let staging_root = characters_root.join(format!(".import_staging_{staging_id}"));
    tokio::fs::create_dir_all(&staging_root)
        .await
        .map_err(|e| format!("创建 staging dir: {e}"))?;

    let staging_root_for_cleanup = staging_root.clone();
    let cleanup_err = |p: &Path| {
        let _ = std::fs::remove_dir_all(p);
    };

    // 4. 解压到临时暂存目录。
    let app_emit = app.clone();
    let target = staging_root.clone();
    let path_for_blocking = tmp_path.to_path_buf();
    let cancel_for_blocking = cancel_token.clone();
    let summary: ExtractSummary = tokio::task::spawn_blocking(move || {
        let on_entry = |evt: EntryEvent| {
            if cancel_for_blocking.is_cancelled() {
                let _ = app_emit.emit("role:import-error", "cancelled by user");
                return;
            }
            let _ = app_emit.emit("role:import-progress", &evt);
        };
        match format {
            ArchiveFormat::Zip => archive::extract_zip(&path_for_blocking, &target, &cancel_for_blocking, &on_entry),
            ArchiveFormat::SevenZ => {
                archive::extract_sevenz(&path_for_blocking, &target, &cancel_for_blocking, &on_entry)
            }
        }
    })
    .await
    .map_err(|e| {
        cleanup_err(&staging_root_for_cleanup);
        format!("spawn_blocking join: {e}")
    })?
    .map_err(|e| {
        tracing::error!("[RoleArchive] do_import 解压失败: {e}");
        cleanup_err(&staging_root_for_cleanup);
        e.to_string()
    })?;
    tracing::info!(
        "[RoleArchive] do_import 解压完成: files={}, bytes={}, skipped_macos={}, warnings={}",
        summary.files_extracted,
        summary.bytes_extracted,
        summary.skipped_macos_metadata,
        summary.warnings.len()
    );

    // 解压完成后再次检查取消状态，命中时立即清理暂存目录并退出。
    if cancel_token.is_cancelled() {
        tracing::info!("[RoleArchive] do_import cancel hit after extract: cleanup staging");
        cleanup_err(&staging_root_for_cleanup);
        return Err("导入已取消".into());
    }

    // 5. 定位解压后的角色内容根目录。
    //    如果只有一个外层角色目录则进入该目录，否则直接使用暂存目录。
    let extracted_dir = locate_extracted_dir(&staging_root).await;
    tracing::info!("[RoleArchive] do_import extracted_dir={}", extracted_dir.display());

    // 6. 根据同名冲突策略解析最终目标目录。
    let resolution = match archive::resolve_target(&characters_root, &final_name, policy) {
        Ok(r) => {
            tracing::info!(
                "[RoleArchive] do_import resolve: target={}, action={}, final_name={}",
                r.target.display(),
                r.action,
                r.final_name
            );
            r
        }
        Err(ArchiveError::AlreadyExists(name)) => {
            tracing::info!("[RoleArchive] do_import Skip 跳过已存在: {}", name);
            cleanup_err(&staging_root_for_cleanup);
            return Ok(ImportResult {
                role_id: None,
                role_name: name,
                conflict_action: "skipped".into(),
                warnings: vec![],
                bytes_extracted: 0,
            });
        }
        Err(e) => {
            cleanup_err(&staging_root_for_cleanup);
            return Err(e.to_string());
        }
    };

    // 解析目标目录后再次检查取消状态，处理用户在解压期间发出的取消请求。
    if cancel_token.is_cancelled() {
        tracing::info!("[RoleArchive] do_import cancel hit after resolve: cleanup staging");
        cleanup_err(&staging_root_for_cleanup);
        return Err("导入已取消".into());
    }

    // 覆盖策略：先清空旧目录，失败时返回明确错误。
    if resolution.action == "overwritten" {
        if let Err(e) = tokio::fs::remove_dir_all(&resolution.target).await {
            tracing::error!(
                "[RoleArchive] do_import overwrite 清空旧目录失败: target={}, err={}",
                resolution.target.display(), e
            );
            cleanup_err(&staging_root_for_cleanup);
            return Err(format!(
                "无法覆盖已存在的角色目录 {} (可能正在被使用, 请关闭相关界面后重试): {e}",
                resolution.target.display()
            ));
        }
    }
    if let Some(parent) = resolution.target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| {
                cleanup_err(&staging_root_for_cleanup);
                format!("创建目标父目录: {e}")
            })?;
    }

    // 移动目录前再次检查取消状态；目标已确定，但尚未写入最终位置。
    if cancel_token.is_cancelled() {
        tracing::info!("[RoleArchive] do_import cancel hit before rename: cleanup staging");
        cleanup_err(&staging_root_for_cleanup);
        return Err("导入已取消".into());
    }

    // 7. 把解压后的角色目录移动到最终目标位置。
    //    同一磁盘优先重命名；若因句柄占用或权限失败，则重试并回退到复制。
    let target_exists_before = resolution.target.exists();
    let mut rename_err: Option<std::io::Error> = None;
    for attempt in 1..=3 {
        match tokio::fs::rename(&extracted_dir, &resolution.target).await {
            Ok(()) => {
                rename_err = None;
                break;
            }
            Err(e) => {
                rename_err = Some(e);
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(150 * attempt as u64)).await;
                }
            }
        }
    }
    if let Some(rerr) = rename_err {
        tracing::warn!(
            "[RoleArchive] do_import rename 3次均失败: src={}, target={}, target_exists={}, err={}",
            extracted_dir.display(), resolution.target.display(), target_exists_before, rerr
        );
        let src_c = extracted_dir.clone();
        let dst_c = resolution.target.clone();
        let copy_res = tokio::task::spawn_blocking(move || copy_dir_recursive(&src_c, &dst_c))
            .await
            .map_err(|je| {
                cleanup_err(&staging_root_for_cleanup);
                format!("移动角色目录失败: rename={rerr}, spawn={je}")
            })?;
        match copy_res {
            Ok(()) => {
                // 复制成功后删除源目录；暂存目录本身由后续统一清理。
                if extracted_dir != staging_root {
                    let _ = tokio::fs::remove_dir_all(&extracted_dir).await;
                }
                tracing::info!("[RoleArchive] do_import rename 失败后复制成功");
            }
            Err(cerr) => {
                cleanup_err(&staging_root_for_cleanup);
                return Err(format!(
                    "移动角色目录失败 (rename: {rerr}; 复制回退: {cerr}). 可能目标正被其他进程占用."
                ));
            }
        }
    }
    tracing::info!(
        "[RoleArchive] do_import 移动完成: {} -> {}",
        extracted_dir.display(),
        resolution.target.display()
    );

    // 8. 同步前删除暂存目录空壳，避免被误注册为角色。
    let _ = tokio::fs::remove_dir_all(&staging_root).await;
    tracing::info!("[RoleArchive] do_import staging 已清理");

    // 8.5 校验 `settings.yml`；缺失时删除刚移动的目录并返回错误。
    let settings_yml = resolution.target.join("settings.yml");
    if !settings_yml.exists() {
        tracing::error!(
            "[RoleArchive] do_import 缺少 settings.yml: {}",
            settings_yml.display()
        );
        let _ = tokio::fs::remove_dir_all(&resolution.target).await;
        return Err(format!(
            "压缩包缺少 settings.yml (角色配置文件不可缺少). 请确保压缩包内含 settings.yml 后重试."
        ));
    }

    // 同步角色数据前进行最后一次取消检查。
    if cancel_token.is_cancelled() {
        tracing::info!("[RoleArchive] do_import cancel hit before sync: rollback target");
        let _ = tokio::fs::remove_dir_all(&resolution.target).await;
        return Err("导入已取消".into());
    }

    // 9. 把角色目录同步到数据库。
    let data_dir = crate::init::static_copy::get_data_dir().clone();
    let db = app.state::<crate::AppState>().db.clone();
    crate::init::role_sync::sync_roles_from_folder(&db, &data_dir)
        .await
        .map_err(|e| {
            // 同步失败时回滚已移入的角色目录，避免产生数据库无记录的孤立目录。
            let target = resolution.target.clone();
            tracing::error!("[RoleArchive] do_import sync failed, rolling back target={}", target.display());
            tokio::spawn(async move {
                let _ = tokio::fs::remove_dir_all(&target).await;
            });
            format!("sync roles: {e}")
        })?;

    // 10. 查询新角色 ID。
    let role_id = find_role_id_by_folder(&db, &resolution.final_name).await?;

    Ok(ImportResult {
        role_id,
        role_name: resolution.final_name.clone(),
        conflict_action: resolution.action.into(),
        warnings: summary.warnings,
        bytes_extracted: summary.bytes_extracted,
    })
}

/// 把 Tauri 调用传来的字节写入应用缓存目录，并返回临时文件路径。
/// 调用方负责删除文件，除非清理守卫已经接管。
pub(super) async fn write_temp_archive(app: &AppHandle, bytes: &[u8]) -> Result<PathBuf, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cache dir: {e}"))?;
    let imports_root = cache_dir.join("imports");
    tokio::fs::create_dir_all(&imports_root)
        .await
        .map_err(|e| format!("create imports dir: {e}"))?;
    let tmp_id = uuid::Uuid::new_v4().to_string();
    let tmp_path = imports_root.join(format!("import_{tmp_id}.bin"));
    tokio::fs::write(&tmp_path, bytes)
        .await
        .map_err(|e| format!("write temp archive: {e}"))?;
    tracing::info!("[RoleArchive] write_temp_archive: {}B -> {}", bytes.len(), tmp_path.display());
    Ok(tmp_path)
}

/// 准备导入源文件路径.
/// - 如果路径以 `content://` 开头，则把 Android SAF 文件复制到缓存目录。
/// - 否则按桌面端文件系统路径处理，不创建额外副本。
///
/// 返回值中的布尔值表示导入完成后是否需要清理本地副本。
pub(super) async fn prepare_import_source(app: &AppHandle, path: &str) -> Result<(PathBuf, bool), String> {
    if path.starts_with("content://") {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        let cache_dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| format!("cache dir: {e}"))?;
        let imports_root = cache_dir.join("imports");
        tokio::fs::create_dir_all(&imports_root)
            .await
            .map_err(|e| format!("create imports dir: {e}"))?;

        let tmp_id = uuid::Uuid::new_v4().to_string();
        let local_path = imports_root.join(format!("import_saf_{tmp_id}.bin"));
        let local_uri = FsUri::from_path(&local_path);
        let src_uri = FsUri::from_uri(path.to_string());
        tracing::info!(
            "[RoleArchive] prepare_import_source SAF: src={}, local={}",
            path,
            local_path.display()
        );

        app.android_fs_async()
            .copy(&src_uri, &local_uri)
            .await
            .map_err(|e| format!("SAF copy to local cache: {e}"))?;

        Ok((local_path, true))
    } else {
        Ok((PathBuf::from(path), false))
    }
}

/// 定位解压后的角色内容根目录:
/// - 暂存目录只含一个子目录时，返回该角色目录，例如 `角色名/settings.yml`。
/// - 否则表示内容直接位于压缩包根目录，返回暂存目录本身。
/// 检查目录结构时忽略 `__MACOSX`、`._*` 和 `.DS_Store`。
async fn locate_extracted_dir(staging: &Path) -> PathBuf {
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut has_files = false;
    if let Ok(mut entries) = tokio::fs::read_dir(staging).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "__MACOSX" || name.starts_with("._") || name == ".DS_Store" {
                continue;
            }
            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => subdirs.push(entry.path()),
                Ok(ft) if ft.is_file() => has_files = true,
                _ => {}
            }
        }
    }
    // 只有 1 个子目录且无文件 -> 返回该子目录 (有外层包裹)
    if subdirs.len() == 1 && !has_files {
        subdirs.into_iter().next().unwrap_or_else(|| staging.to_path_buf())
    } else {
        // 没有外层角色目录时，内容直接位于暂存目录根部。
        staging.to_path_buf()
    }
}

/// 规范化角色文件夹名:
/// - 替换非法字符
/// - 拒绝保留名称，例如 `avatar`、`__MACOSX` 和隐藏名称。
/// - 名称为空或非法时使用备用名称，备用名称也会经过规范化。
fn sanitize_role_folder_name(name: Option<&str>, fallback: Option<&str>) -> String {
    const RESERVED: &[&str] = &["avatar", "__macosx"];
    /// 去掉 `.zip` 或 `.7z` 扩展名，保留其余部分作为名称。
    fn strip_archive_ext(s: &str) -> String {
        let lower = s.to_lowercase();
        for ext in [".zip", ".7z"] {
            if lower.ends_with(ext) {
                return s[..s.len() - ext.len()].to_string();
            }
        }
        s.to_string()
    }
    fn sanitize_once(s: &str) -> String {
        let chars: String = s
            .chars()
            .map(|c| match c {
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect();
        chars.trim().to_string()
    }
    fn is_reserved(s: &str) -> bool {
        let lower = s.to_lowercase();
        RESERVED.contains(&lower.as_str()) || lower.starts_with("._") || lower.starts_with('.')
    }
    // 优先使用指定名称，其次使用备用名称，最后生成带时间戳的名称。
    for candidate in [name, fallback].into_iter().flatten() {
        let stripped = strip_archive_ext(candidate);
        let s = sanitize_once(&stripped);
        if !s.is_empty() && !is_reserved(&s) {
            return s;
        }
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("role_{ts}")
}

/// 递归复制目录，作为重命名失败时的回退方案。
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ft.is_file() {
            std::fs::copy(&from, &to)?;
        } else if ft.is_symlink() {
            if let Ok(meta) = std::fs::metadata(&from) {
                if meta.is_dir() {
                    copy_dir_recursive(&from, &to)?;
                } else {
                    std::fs::copy(&from, &to)?;
                }
            }
        }
    }
    Ok(())
}

async fn find_role_id_by_folder(
    db: &DatabaseConnection,
    folder: &str,
) -> Result<Option<i32>, String> {
    let role = RoleEntity::find()
        .filter(Column::ResourceFolder.eq(folder))
        .one(db)
        .await
        .map_err(|e| format!("查角色: {e}"))?;
    Ok(role.map(|r| r.id))
}

pub(super) fn parse_format(s: &str) -> Result<ArchiveFormat, String> {
    match s {
        "zip" => Ok(ArchiveFormat::Zip),
        "7z" => Ok(ArchiveFormat::SevenZ),
        _ => Err(format!("不支持的 format: {s}")),
    }
}

pub(super) fn parse_policy(s: &str) -> Result<ConflictPolicy, String> {
    match s {
        "rename" => Ok(ConflictPolicy::Rename),
        "skip" => Ok(ConflictPolicy::Skip),
        "overwrite" => Ok(ConflictPolicy::Overwrite),
        _ => Err(format!("不支持的 conflict: {s}")),
    }
}
