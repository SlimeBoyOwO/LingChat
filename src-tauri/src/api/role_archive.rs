//! 角色压缩包导入/导出 Tauri 命令。
//!
//! `import_role_from_path` 同时支持桌面文件路径和 Android SAF 内容 URI。
//! Android 压缩包由后端复制到应用缓存后再解压，避免通过前端 IPC
//! 传递整包字节，并且不设置压缩包的绝对大小限制。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::db::entities::role::{Column, Entity as RoleEntity};
use crate::utils::archive::{
    self, ArchiveError, ArchiveFormat, ConflictPolicy, EntryEvent, ExtractSummary,
};


// ===== 状态（取消令牌与调用锁） =====

/// 单个导入任务的运行时状态。
/// `saf_cache_path` 用于在取消任务时立即清理 SAF 缓存副本。
pub struct ImportTaskEntry {
    pub cancel_token: Arc<CancellationToken>,
    pub saf_cache_path: std::sync::Mutex<Option<PathBuf>>,
}

/// 角色压缩包导入/导出的全局状态。
/// - `tasks`：当前正在运行的导入任务，键为任务 ID。
/// - `importing`：全局导入并发锁，为 `true` 时拒绝新任务。
pub struct RoleArchiveState {
    pub tasks: std::sync::Mutex<std::collections::HashMap<String, ImportTaskEntry>>,
    pub importing: std::sync::atomic::AtomicBool,
}

impl Default for RoleArchiveState {
    fn default() -> Self {
        Self {
            tasks: std::sync::Mutex::new(std::collections::HashMap::new()),
            importing: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

/// 基于 RAII 的守卫，函数返回时自动释放 `importing` 标志。
struct ImportingGuard<'a> {
    flag: &'a std::sync::atomic::AtomicBool,
}

impl Drop for ImportingGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// 基于 RAII 的守卫，函数返回时自动移除任务并清理 SAF 缓存副本。
struct TaskRemoveGuard<'a> {
    state: &'a RoleArchiveState,
    task_id: &'a str,
}

impl Drop for TaskRemoveGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut tasks) = self.state.tasks.lock() {
            if let Some(entry) = tasks.remove(self.task_id) {
                if let Ok(mut guard) = entry.saf_cache_path.lock() {
                    if let Some(path) = guard.take() {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
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

/// 通过单次 Tauri 调用传入压缩包字节并导入角色。
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
// ===== 内部辅助函数 =====

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
async fn do_import(
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
async fn write_temp_archive(app: &AppHandle, bytes: &[u8]) -> Result<PathBuf, String> {
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
async fn prepare_import_source(app: &AppHandle, path: &str) -> Result<(PathBuf, bool), String> {
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

// ===== 导出命令 =====

#[tauri::command]
/// 把角色压缩到缓存目录，并返回临时路径、建议文件名和文件大小。
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

/// 导出角色到临时文件，仅返回 `temp_path`，不复制到用户目录。
/// 仅供前端需要自行处理临时文件时使用；通常应调用 `export_role_to_path`。
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

/// 导出角色并复制到用户指定的位置，然后删除临时文件。
/// 桌面端使用 `std::fs::copy`，Android 内容 URI 使用 `android-fs` SAF 接口。
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
