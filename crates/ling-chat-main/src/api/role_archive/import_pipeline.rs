//! 角色压缩包导入的内部实现：检测格式、解压、目录定位、冲突处理。
//!
//! 该模块不直接注册 Tauri 命令；上层 `mod.rs` 中的命令函数负责提取/校验
//! 用户参数并调用 `pub(super)` 暴露的实现。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use crate::ai_service::tts::local::saf_bridge::{ImportSource, prepare_file_import_source};
use crate::db::entities::role::{Column, Entity as RoleEntity};
use crate::utils::archive::{
    self, ArchiveError, ArchiveFormat, ConflictPolicy, EntryEvent, ExtractSummary,
};
use crate::utils::yaml_file::resolve_settings_file;

use super::ImportResult;

pub(super) async fn do_import(
    app: &AppHandle,
    tmp_path: &Path,
    format: Option<ArchiveFormat>,
    policy: ConflictPolicy,
    cancel_token: Arc<CancellationToken>,
    file_name: Option<&str>,
) -> Result<ImportResult, String> {
    // 1. 校验文件头魔数。前端 hint 可选；以 magic 为准，hint 仅用于 debug 日志。
    let detected = archive::detect_format(tmp_path).map_err(|e| e.to_string())?;
    let format = match format {
        Some(hint) if hint == detected => detected,
        Some(hint) => {
            tracing::warn!(
                "[RoleArchive] do_import 扩展名 hint={hint:?} 与 magic {detected:?} 不一致，采用 magic 结果"
            );
            detected
        },
        None => detected,
    };

    // 2. 使用去除扩展名后的压缩包文件名作为角色文件夹名。
    let final_name = sanitize_role_folder_name(file_name, None);
    tracing::info!(
        "[RoleArchive] do_import 文件夹名: final_name={} (file_name={:?})",
        final_name,
        file_name
    );

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
        // on_entry 不检查 cancel: archive.rs extract_zip/extract_sevenz 在每条 entry 前
        // 已经检查 cancel_token 并直接 return ArchiveError::Cancelled, 不会调到这里.
        let on_entry = |evt: EntryEvent| {
            let _ = app_emit.emit("role:import-progress", &evt);
        };
        match format {
            ArchiveFormat::Zip => {
                archive::extract_zip(&path_for_blocking, &target, &cancel_for_blocking, &on_entry)
            },
            ArchiveFormat::SevenZ => archive::extract_sevenz(
                &path_for_blocking,
                &target,
                &cancel_for_blocking,
                &on_entry,
            ),
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
    let extracted_dir = archive::locate_extracted_root(&staging_root).await;
    tracing::info!(
        "[RoleArchive] do_import extracted_dir={}",
        extracted_dir.display()
    );

    // 5.5 先在暂存目录里校验设定文件（settings_local.yml 优先，否则 settings.yml）。
    //     必须早于任何破坏旧目录的动作：否则一个缺设定文件的坏包会把原角色一并带走。
    if !resolve_settings_file(&extracted_dir).exists() {
        tracing::error!(
            "[RoleArchive] do_import 压缩包缺少设定文件: {}",
            extracted_dir.display()
        );
        cleanup_err(&staging_root_for_cleanup);
        // 返回 i18n 错误码，前端按 ui.archiveProgress.errors.<code> 查表翻译。
        return Err("ARCHIVE_MISSING_SETTINGS_YML".into());
    }

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
        },
        Err(ArchiveError::AlreadyExists(name)) => {
            tracing::info!("[RoleArchive] do_import Skip 跳过已存在: {}", name);
            cleanup_err(&staging_root_for_cleanup);
            return Ok(ImportResult {
                role_id: None,
                role_name: name,
                conflict_action: "skipped".into(),
                warnings: vec![],
                bytes_extracted: 0,
                format,
            });
        },
        Err(e) => {
            cleanup_err(&staging_root_for_cleanup);
            return Err(e.to_string());
        },
    };

    // 解析目标目录后再次检查取消状态，处理用户在解压期间发出的取消请求。
    if cancel_token.is_cancelled() {
        tracing::info!("[RoleArchive] do_import cancel hit after resolve: cleanup staging");
        cleanup_err(&staging_root_for_cleanup);
        return Err("导入已取消".into());
    }

    // 覆盖策略：把旧目录改名到同级的隐藏备份，而不是直接删除。
    // 任何后续步骤失败都靠它还原来保证原角色不丢；只有全部成功后才真正清理。
    let backup_dir = if resolution.action == "overwritten" {
        let backup = characters_root.join(format!(".import_backup_{staging_id}"));
        if let Err(e) = tokio::fs::rename(&resolution.target, &backup).await {
            tracing::error!(
                "[RoleArchive] do_import overwrite 备份旧目录失败: target={}, err={}",
                resolution.target.display(),
                e
            );
            cleanup_err(&staging_root_for_cleanup);
            return Err(format!(
                "无法覆盖已存在的角色目录 {} (可能正在被使用, 请关闭相关界面后重试): {e}",
                resolution.target.display()
            ));
        }
        Some(backup)
    } else {
        None
    };

    if let Some(parent) = resolution.target.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            cleanup_err(&staging_root_for_cleanup);
            restore_backup(&resolution.target, backup_dir.as_deref()).await;
            return Err(format!("创建目标父目录: {e}"));
        }
    }

    // 移动目录前再次检查取消状态；目标已确定，但尚未写入最终位置。
    if cancel_token.is_cancelled() {
        tracing::info!("[RoleArchive] do_import cancel hit before rename: cleanup staging");
        cleanup_err(&staging_root_for_cleanup);
        restore_backup(&resolution.target, backup_dir.as_deref()).await;
        return Err("导入已取消".into());
    }

    // 7. 把解压后的角色目录移动到最终目标位置。
    //    同一磁盘优先重命名；若因句柄占用或权限失败，则重试并回退到复制。
    if let Err(e) = archive::relocate_dir(&extracted_dir, &resolution.target).await {
        tracing::error!(
            "[RoleArchive] do_import 移动失败: src={}, target={}, err={}",
            extracted_dir.display(),
            resolution.target.display(),
            e
        );
        cleanup_err(&staging_root_for_cleanup);
        restore_backup(&resolution.target, backup_dir.as_deref()).await;
        return Err(e);
    }
    tracing::info!(
        "[RoleArchive] do_import 移动完成: {} -> {}",
        extracted_dir.display(),
        resolution.target.display()
    );

    // 8. 同步前删除暂存目录空壳，避免被误注册为角色。
    let _ = tokio::fs::remove_dir_all(&staging_root).await;
    tracing::info!("[RoleArchive] do_import staging 已清理");

    // 同步角色数据前进行最后一次取消检查。
    if cancel_token.is_cancelled() {
        tracing::info!("[RoleArchive] do_import cancel hit before sync: rollback target");
        restore_backup(&resolution.target, backup_dir.as_deref()).await;
        return Err("导入已取消".into());
    }

    // 9. 把角色目录同步到数据库。
    let data_dir = crate::data_dir::get_data_dir().clone();
    let db = app.state::<crate::AppState>().db.clone();
    if let Err(e) = crate::db::role_sync::sync_roles_from_folder(&db, &data_dir).await {
        // 同步失败时立即回滚已移入的角色目录（await 而不是 spawn），
        // 避免用户立刻重试同名导入时遇到尚未删除的旧目录。
        tracing::error!(
            "[RoleArchive] do_import sync failed, rolling back target={}",
            resolution.target.display()
        );
        restore_backup(&resolution.target, backup_dir.as_deref()).await;
        return Err(format!("sync roles: {e}"));
    }

    // 10. 全部成功，删除旧目录备份。
    if let Some(backup) = &backup_dir {
        if let Err(e) = tokio::fs::remove_dir_all(backup).await {
            // 备份残留不影响功能（隐藏目录不会被注册为角色），只记录日志。
            tracing::warn!(
                "[RoleArchive] 删除旧目录备份失败: backup={}, err={}",
                backup.display(),
                e
            );
        }
    }

    // 11. 查询新角色 ID。
    let role_id = find_role_id_by_folder(&db, &resolution.final_name).await?;

    Ok(ImportResult {
        role_id,
        role_name: resolution.final_name.clone(),
        conflict_action: resolution.action.into(),
        warnings: summary.warnings,
        bytes_extracted: summary.bytes_extracted,
        format,
    })
}

/// 导入失败时把目标位置恢复成导入前的样子。
///
/// 覆盖导入的旧目录此时躺在同级隐藏备份里：先删掉刚落位的新目录，再把备份改名
/// 回原位，保证原角色不会因为一个坏包而消失。还原失败时备份目录仍然保留，
/// 日志里带上它的路径，用户可以手工恢复。
/// 非覆盖场景（新建 / 重命名）没有旧目录，直接删掉刚移入的目录即可。
async fn restore_backup(target: &Path, backup: Option<&Path>) {
    let Some(backup) = backup else {
        let _ = tokio::fs::remove_dir_all(target).await;
        return;
    };
    let _ = tokio::fs::remove_dir_all(target).await;
    match tokio::fs::rename(backup, target).await {
        Ok(()) => tracing::info!("[RoleArchive] 已还原原角色目录: {}", target.display()),
        Err(e) => tracing::error!(
            "[RoleArchive] 还原原角色失败, 旧目录仍在备份中 ({}): {e}",
            backup.display()
        ),
    }
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
    tracing::info!(
        "[RoleArchive] write_temp_archive: {}B -> {}",
        bytes.len(),
        tmp_path.display()
    );
    Ok(tmp_path)
}

/// 准备导入源文件路径.
/// - 如果路径以 `content://` 开头，则把 Android SAF 文件复制到缓存目录。
/// - 否则按桌面端文件系统路径处理，不创建额外副本。
///
/// 返回 [`ImportSource`]：`path` 是本地可读路径，`cleanup_after_import` 指示调用方
/// 是否需要在导入完成后清理本地副本，`display_name` 是用户面向的文件名
/// （Android 上由 SAF 元数据提供；桌面端是路径的 basename）。
/// 后端用它做 fallback：当传入的 `file_name` 看起来是 percent-encoded hex blob
/// （content URI 的末段未经前端 decode 的痕迹）时优先采用它，保证角色文件夹名
/// 是真实可读的名字，而不是 `%E6%A9%98...` 之类的 hex 串。
pub(super) async fn prepare_import_source(
    app: &AppHandle,
    path: &str,
) -> Result<ImportSource, String> {
    prepare_file_import_source(app, path).await
}

/// 判断一个字符串是否像是 percent-encoded hex blob（Android content URI
/// 末段的典型形态）。仅当传入的 `file_name` 命中此模式时才考虑替换。
pub(super) fn looks_like_percent_encoded_blob(s: &str) -> bool {
    if s.is_empty() || !s.is_ascii() {
        return false;
    }
    // 整段几乎都是 hex 字符 + 极少其他字符（点/下划线/连字符）。
    let hex_count = s.chars().filter(|c| c.is_ascii_hexdigit()).count();
    let total = s.chars().count();
    if total < 8 || hex_count * 10 < total * 9 {
        return false;
    }
    s.split('.')
        .next()
        .is_some_and(|stem| stem.len() >= 6 && stem.chars().all(|c| c.is_ascii_hexdigit()))
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
        s.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '.'))
            .collect()
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

/// Optional format parser: `None`/空字符串代表前端没传 hint，让后端用 magic 决定。
pub(super) fn parse_format_opt(s: Option<&str>) -> Result<Option<ArchiveFormat>, String> {
    match s {
        None | Some("") | Some("auto") => Ok(None),
        Some(v) => parse_format(v).map(Some),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[tokio::test]
    async fn restore_backup_puts_old_role_back() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("角色A");
        let backup = dir.path().join(".import_backup_x");
        write(&backup.join("settings.yml"), "name: 旧角色");
        // 目标位置放的是刚落位的坏包内容
        write(&target.join("junk.txt"), "broken");

        restore_backup(&target, Some(backup.as_path())).await;

        assert!(target.join("settings.yml").exists());
        assert!(!target.join("junk.txt").exists());
        assert!(!backup.exists());
    }

    #[tokio::test]
    async fn restore_backup_without_backup_removes_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("新角色");
        write(&target.join("settings.yml"), "name: 新角色");

        restore_backup(&target, None).await;

        assert!(!target.exists());
    }
}
