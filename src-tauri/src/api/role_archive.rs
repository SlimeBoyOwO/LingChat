//! 角色压缩包导入/导出 Tauri 命令。
//!
//! # P0 #3 修复: 合并 prepare_import_slot + import_role_from_file 为单 invoke
//!
//! 旧设计 (有 Android 边界问题):
//!   1. prepare_import_slot           → 返回 temp_path
//!   2. 前端 plugin-fs.writeFile(...) → **Android 上不可靠**
//!   3. import_role_from_file(...)    → 解压
//!
//! 新设计 (跨端一致):
//!   1. import_role(bytes, format, conflict) — 小文件 (< 50MB) 直接传 Vec<u8>
//!   2. import_role_from_path(path, ...)    — 大文件备用 (TODO 集成)
//!
//! 收益:
//!   - 前端不依赖 plugin-fs 写 cache,绕开 Android plugin-fs scope 边界坑
//!   - 单次 invoke,事务边界清晰
//!   - 临时文件由 Rust 自己管理,失败也保证清理

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::db::entities::role::{Column, Entity as RoleEntity};
use crate::utils::archive::{
    self, ArchiveError, ArchiveFormat, ConflictPolicy, EntryEvent, ExtractSummary,
};


// ===== ???? (cancel token ? invoke ??) =====

pub struct RoleArchiveState {
    pub cancel_token: Arc<std::sync::Mutex<Arc<CancellationToken>>>,
}

impl Default for RoleArchiveState {
    fn default() -> Self {
        Self {
            cancel_token: Arc::new(std::sync::Mutex::new(Arc::new(CancellationToken::new()))),
        }
    }
}

// ===== 响应结构 =====

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

// ===== Tauri 命令 =====

const MAX_INVOKE_BYTES: usize = 50 * 1024 * 1024;

/// P0 #3 修复: 单 invoke 导入角色 (小文件 < 50MB)。
#[tauri::command]
pub async fn import_role(
    app: AppHandle,
    state: State<'_, RoleArchiveState>,
    bytes: Vec<u8>,
    format: String,
    conflict: String,
    file_name: Option<String>,
) -> Result<ImportResult, String> {
    if bytes.is_empty() {
        tracing::warn!("[RoleArchive] import_role 收到空文件");
        return Err("空文件".into());
    }
    if bytes.len() > MAX_INVOKE_BYTES {
        tracing::warn!(
            "[RoleArchive] import_role 超过单 invoke 上限: {}MB > {}MB",
            bytes.len() / 1024 / 1024,
            MAX_INVOKE_BYTES / 1024 / 1024
        );
        return Err(format!(
            "单 invoke 上限 {}MB, 实际 {}MB, 请改用 import_role_from_path",
            MAX_INVOKE_BYTES / 1024 / 1024,
            bytes.len() / 1024 / 1024
        ));
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
    let cancel_token = fresh_cancel_token(&state);

    // 写临时文件 (用于 magic bytes 校验 + zip/sevenz crate 接受 path/reader)
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
#[tauri::command]
pub async fn cancel_role_import(state: State<'_, RoleArchiveState>) -> Result<(), String> {
    tracing::info!("[RoleArchive] cancel_role_import 收到取消请求");
    let guard = state.cancel_token.lock().unwrap();
    guard.cancel();
    Ok(())
}

// ===== 内部 helper =====

/// ??? (>50MB) ????????? SAF / ?????? `$APPCACHE/imports/` ?????????,
/// ??? `path` (file:// URI ?????) ?????
#[tauri::command]
pub async fn import_role_from_path(
    app: AppHandle,
    state: State<'_, RoleArchiveState>,
    path: String,
    format: String,
    conflict: String,
    file_name: Option<String>,
) -> Result<ImportResult, String> {
    if path.is_empty() {
        tracing::warn!("[RoleArchive] import_role_from_path 收到空 path");
        return Err("path ??".into());
    }
    let format = parse_format(&format)?;
    let policy = parse_policy(&conflict)?;
    tracing::info!(
        "[RoleArchive] import_role_from_path 开始: path={}, format={:?}, conflict={:?}",
        path, format, policy
    );
    let cancel_token = fresh_cancel_token(&state);

    let path_buf = if let Some(stripped) = path.strip_prefix("file://") {
        PathBuf::from(stripped.trim_start_matches('/'))
    } else {
        PathBuf::from(&path)
    };
    if !path_buf.exists() {
        return Err(format!("?????: {}", path_buf.display()));
    }
    let meta = tokio::fs::metadata(&path_buf)
        .await
        .map_err(|e| format!("stat path: {e}"))?;
    tracing::info!(
        "[RoleArchive] import_role_from_path 文件大小: {}B ({}MB)",
        meta.len(),
        meta.len() / 1024 / 1024
    );
    if meta.len() > crate::utils::archive::MAX_EXTRACTED_BYTES {
        tracing::warn!(
            "[RoleArchive] import_role_from_path 超过解压上限: {}MB > {}MB",
            meta.len() / 1024 / 1024,
            crate::utils::archive::MAX_EXTRACTED_BYTES / 1024 / 1024
        );
        return Err(format!(
            "????? {}MB ?????? {}MB, ???? (??????)",
            meta.len() / 1024 / 1024,
            crate::utils::archive::MAX_EXTRACTED_BYTES / 1024 / 1024
        ));
    }

    let result = do_import(&app, &path_buf, format, policy, cancel_token, file_name.as_deref()).await;
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

fn fresh_cancel_token(state: &State<'_, RoleArchiveState>) -> Arc<CancellationToken> {
    let mut guard = state.cancel_token.lock().unwrap();
    *guard = Arc::new(CancellationToken::new());
    guard.clone()
}

async fn write_temp_archive(app: &AppHandle, bytes: &[u8]) -> Result<PathBuf, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cache dir 不可用: {e}"))?;
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .map_err(|e| format!("创建 cache dir: {e}"))?;
    let tmp_path = cache_dir.join(format!("role_import_{}.tmp", uuid::Uuid::new_v4()));
    tokio::fs::write(&tmp_path, bytes)
        .await
        .map_err(|e| format!("写临时文件: {e}"))?;
    Ok(tmp_path)
}

async fn do_import(
    app: &AppHandle,
    tmp_path: &Path,
    format: ArchiveFormat,
    policy: ConflictPolicy,
    cancel_token: Arc<CancellationToken>,
    file_name: Option<&str>,
) -> Result<ImportResult, String> {
    // 1. magic bytes 校验
    let detected = archive::detect_format(tmp_path).map_err(|e| e.to_string())?;
    if detected != format {
        tracing::warn!("[RoleArchive] do_import 格式不匹配: 前端 {format:?}, 实际 {detected:?}");
        return Err(format!(
            "格式不匹配: 前端传 {format:?}, 实际 {detected:?}"
        ));
    }

    // 2. 角色文件夹名 = 压缩包文件名 (去扩展名). 不再从 settings.yml 读角色名.
    let final_name = sanitize_role_folder_name(file_name, None);
    tracing::info!("[RoleArchive] do_import 文件夹名: final_name={} (file_name={:?})", final_name, file_name);

    // 3. 创建 staging 目录: characters/.import_staging_{uuid}/
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

    // 4. 解压到 staging
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
            ArchiveFormat::Zip => archive::extract_zip(&path_for_blocking, &target, &on_entry),
            ArchiveFormat::SevenZ => {
                archive::extract_sevenz(&path_for_blocking, &target, &on_entry)
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

    // 5. 定位 extracted_dir:
    //    - 解压后 staging 只含单一子目录 (有外层角色名目录) -> extracted_dir = staging/{that}
    //    - 否则 (无外层, 直接是 settings.yml + avatar/ 平级) -> extracted_dir = staging 本身
    let extracted_dir = locate_extracted_dir(&staging_root).await;
    tracing::info!("[RoleArchive] do_import extracted_dir={}", extracted_dir.display());

    // 6. resolve_target 处理冲突
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

    // Overwrite 策略: 先清空旧目录. 失败时报明确错误
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

    // 7. 移动 extracted_dir -> resolution.target
    //    同盘 rename 应成功; 失败时 (句柄占用/权限) 重试 + 复制回退
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
                // 复制成功, 删除源 (如果源是 staging 本身, 不删, 后面统一清 staging)
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

    // 8. 同步删除 staging 空壳 (在 sync 之前确保清除, 避免被误注册为角色)
    let _ = tokio::fs::remove_dir_all(&staging_root).await;
    tracing::info!("[RoleArchive] do_import staging 已清理");

    // 8.5 校验 settings.yml 存在 (不可缺少). 缺失则回滚 (删除刚移动的目录) 并报错.
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

    // 9. 同步角色到 DB
    let data_dir = crate::init::static_copy::get_data_dir().clone();
    let db = app.state::<crate::AppState>().db.clone();
    crate::init::role_sync::sync_roles_from_folder(&db, &data_dir)
        .await
        .map_err(|e| format!("sync roles: {e}"))?;

    // 10. 查新角色 id
    let role_id = find_role_id_by_folder(&db, &resolution.final_name).await?;

    Ok(ImportResult {
        role_id,
        role_name: resolution.final_name.clone(),
        conflict_action: resolution.action.into(),
        warnings: summary.warnings,
        bytes_extracted: summary.bytes_extracted,
    })
}

/// 定位解压后的角色内容根目录:
/// - staging 只含单一子目录 (有外层角色名目录, 如 "??/settings.yml") -> 返回 staging/{that}
/// - 否则 (无外层, settings.yml + avatar/ 平级) -> 返回 staging 本身
/// 跳过 __MACOSX / ._ 开头的 macOS 元数据.
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
        // 无外层, 内容直接在 staging 根 -> 返回 staging 本身
        staging.to_path_buf()
    }
}

/// 规范化角色文件夹名:
/// - 替换非法字符
/// - 拒绝保留名 (avatar / __MACOSX / 以 ._ 开头 / 以 . 开头的隐藏名)
/// - 空或非法时回退到 fallback; fallback 本身也会被规范化
fn sanitize_role_folder_name(name: Option<&str>, fallback: Option<&str>) -> String {
    const RESERVED: &[&str] = &["avatar", "__macosx"];
    /// 去掉常见压缩包扩展名 (.zip/.7z), 保留其余部分作为名字.
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
    // 优先: name (去扩展名) -> fallback (去扩展名) -> role_{ts}
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

/// 递归复制目录 (rename 失败时回退). 返回 io::Result.
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

fn parse_format(s: &str) -> Result<ArchiveFormat, String> {
    match s {
        "zip" => Ok(ArchiveFormat::Zip),
        "7z" => Ok(ArchiveFormat::SevenZ),
        _ => Err(format!("不支持的 format: {s}")),
    }
}

fn parse_policy(s: &str) -> Result<ConflictPolicy, String> {
    match s {
        "rename" => Ok(ConflictPolicy::Rename),
        "skip" => Ok(ConflictPolicy::Skip),
        "overwrite" => Ok(ConflictPolicy::Overwrite),
        _ => Err(format!("不支持的 conflict: {s}")),
    }
}

// ===== 导出命令 (导出流程其他 P 修复点的实现留待后续) =====

#[tauri::command]
/// 内部: 压缩角色到 cache/exports 下的临时文件, 返回 (path, suggested_name, size)
async fn compress_role_to_temp(
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
    tokio::task::spawn_blocking(move || {
        let on_entry = |_evt: EntryEvent| {};
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

/// 导出角色到临时文件 (仅返回 temp_path, 不复制).
/// 使用场景: 需要前端自行处理临时文件. 一般推荐用 export_role_to_path.
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

/// 导出角色并复制到用户指定的目标路径, 然后删除临时文件.
/// 使用 std::fs::copy (后端原生 IO, 不受 Tauri fs scope 约束), 避免 plugin-fs.copyFile 权限问题.
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

    let dest = PathBuf::from(&dest_path);
    // 确保目标父目录存在
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create dest parent dir: {e}"))?;
    }

    // 后端原生复制 (不受 fs scope 约束)
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
