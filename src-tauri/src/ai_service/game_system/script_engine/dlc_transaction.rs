//! Crash-recoverable DLC uninstall quarantine.
//!
//! Package removal is committed with one same-volume directory rename. A small
//! transaction record lives outside `game_data/scripts`, so a hard kill or a
//! partial `remove_dir_all` can never leave a half-deleted package looking
//! installed. Startup and later DLC operations retry forward cleanup.

use std::fs;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const TRANSACTION_ROOT: &str = ".dlc-uninstall";
const REJECTED_ROOT: &str = ".dlc-rejected";
const IMPORT_ROOT: &str = ".dlc-import";
const TRANSACTION_FILE: &str = "transaction.json";
const INSTALL_FILE: &str = "install.json";
const PACKAGE_DIR: &str = "package";
const MAX_TRANSACTION_BYTES: u64 = 8 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct PendingUninstall {
    version: u32,
    folder_key: String,
    owner: String,
    #[serde(default)]
    rename_durable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingInstall {
    version: u32,
    folder_key: String,
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FlushFileBuffers, FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_WRITE,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    }
    .with_context(|| format!("打开目录刷盘句柄失败: {}", path.display()))?;
    let flush_result = unsafe { FlushFileBuffers(handle) }
        .with_context(|| format!("刷新目录元数据失败: {}", path.display()));
    let close_result = unsafe { CloseHandle(handle) }
        .with_context(|| format!("关闭目录刷盘句柄失败: {}", path.display()));
    flush_result.and(close_result)
}

#[cfg(not(windows))]
pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .with_context(|| format!("打开目录刷盘句柄失败: {}", path.display()))?
        .sync_all()
        .with_context(|| format!("刷新目录元数据失败: {}", path.display()))
}

fn sync_directory_tree(root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(root)
        .with_context(|| format!("读取待刷盘目录失败: {}", root.display()))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(anyhow!("待刷盘目录树包含不安全根目录"));
    }
    for entry in
        fs::read_dir(root).with_context(|| format!("读取待刷盘目录失败: {}", root.display()))?
    {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("读取暂存条目失败: {}", path.display()))?;
        if is_link_like(&metadata) {
            return Err(anyhow!("DLC 暂存目录树不允许链接: {}", path.display()));
        }
        if metadata.is_dir() {
            sync_directory_tree(&path)?;
        } else if !metadata.is_file() {
            return Err(anyhow!("DLC 暂存目录树包含非普通文件"));
        }
    }
    sync_directory(root)
}

fn transaction_root(data_dir: &Path) -> PathBuf {
    data_dir.join(TRANSACTION_ROOT)
}

fn rejected_root(data_dir: &Path) -> PathBuf {
    data_dir.join(REJECTED_ROOT)
}

fn import_root(data_dir: &Path) -> PathBuf {
    data_dir.join(IMPORT_ROOT)
}

fn standalone_root(data_dir: &Path) -> PathBuf {
    data_dir
        .join("game_data")
        .join("scripts")
        .join("standalone")
}

fn validate_folder_key(folder_key: &str) -> Result<()> {
    let mut components = Path::new(folder_key).components();
    if folder_key.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(anyhow!("DLC 卸载事务含不安全目录名"));
    }
    Ok(())
}

fn ensure_transaction_root(data_dir: &Path) -> Result<PathBuf> {
    let root = transaction_root(data_dir);
    match fs::symlink_metadata(&root) {
        Ok(metadata) => {
            if is_link_like(&metadata) || !metadata.is_dir() {
                return Err(anyhow!("DLC 卸载事务根目录不能是链接或普通文件"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&root)
                .with_context(|| format!("创建 DLC 卸载事务目录失败: {}", root.display()))?;
            sync_directory(data_dir).context("提交 DLC 卸载事务根目录失败")?;
        }
        Err(error) => return Err(error).context("读取 DLC 卸载事务目录失败"),
    }
    Ok(root)
}

fn ensure_rejected_root(data_dir: &Path) -> Result<PathBuf> {
    let root = rejected_root(data_dir);
    match fs::symlink_metadata(&root) {
        Ok(metadata) => {
            if is_link_like(&metadata) || !metadata.is_dir() {
                return Err(anyhow!("DLC 拒绝包隔离目录不能是链接或普通文件"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&root)
                .with_context(|| format!("创建 DLC 拒绝包隔离目录失败: {}", root.display()))?;
            sync_directory(data_dir).context("提交 DLC 拒绝包隔离根目录失败")?;
        }
        Err(error) => return Err(error).context("读取 DLC 拒绝包隔离目录失败"),
    }
    Ok(root)
}

fn ensure_import_root(data_dir: &Path) -> Result<PathBuf> {
    let root = import_root(data_dir);
    match fs::symlink_metadata(&root) {
        Ok(metadata) => {
            if is_link_like(&metadata) || !metadata.is_dir() {
                return Err(anyhow!("DLC 导入暂存根目录不能是链接或普通文件"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&root)
                .with_context(|| format!("创建 DLC 导入暂存目录失败: {}", root.display()))?;
            sync_directory(data_dir).context("提交 DLC 导入暂存根目录失败")?;
        }
        Err(error) => return Err(error).context("读取 DLC 导入暂存目录失败"),
    }
    Ok(root)
}

fn read_transaction(transaction_dir: &Path) -> Result<PendingUninstall> {
    let metadata = fs::symlink_metadata(transaction_dir)
        .with_context(|| format!("读取卸载事务目录失败: {}", transaction_dir.display()))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(anyhow!("卸载事务目录不能是链接或普通文件"));
    }
    let path = transaction_dir.join(TRANSACTION_FILE);
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("读取卸载事务记录失败: {}", path.display()))?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > MAX_TRANSACTION_BYTES {
        return Err(anyhow!("卸载事务记录类型或大小非法"));
    }
    let transaction: PendingUninstall = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("读取卸载事务失败: {}", path.display()))?,
    )
    .context("解析 DLC 卸载事务失败")?;
    if transaction.version != 1 {
        return Err(anyhow!("不支持的 DLC 卸载事务版本"));
    }
    validate_folder_key(&transaction.folder_key)?;
    Ok(transaction)
}

fn direct_transaction_dir(data_dir: &Path, transaction_dir: &Path) -> Result<PathBuf> {
    let root = ensure_transaction_root(data_dir)?;
    if transaction_dir.parent() != Some(root.as_path()) {
        return Err(anyhow!("卸载事务不在受控根目录内"));
    }
    Ok(root)
}

fn save_uninstall_transaction(
    root: &Path,
    transaction_dir: &Path,
    transaction: &PendingUninstall,
) -> Result<()> {
    let content = serde_json::to_vec_pretty(transaction).context("序列化 DLC 卸载事务失败")?;
    let record_path = transaction_dir.join(TRANSACTION_FILE);
    crate::ai_service::tools::atomic_replace(&record_path, &content)
        .map_err(|error| anyhow!(error))
        .context("保存 DLC 卸载事务失败")?;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&record_path)
        .and_then(|file| file.sync_all())
        .context("把 DLC 卸载事务记录刷入磁盘失败")?;
    sync_directory(transaction_dir)?;
    sync_directory(root)
}

/// Atomically move an installed DLC out of the scan tree. No persistent state
/// is detached before this succeeds.
pub(crate) fn stage_uninstall(data_dir: &Path, target: &Path, owner: &str) -> Result<PathBuf> {
    let expected_root = standalone_root(data_dir);
    if target.parent() != Some(expected_root.as_path()) {
        return Err(anyhow!("只允许隔离 standalone 根目录中的 DLC"));
    }
    let target_metadata = fs::symlink_metadata(target)
        .with_context(|| format!("读取待卸载 DLC 目录失败: {}", target.display()))?;
    if is_link_like(&target_metadata) || !target_metadata.is_dir() {
        return Err(anyhow!("待卸载 DLC 根目录不能是链接或普通文件"));
    }
    let folder_key = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("无法确定 DLC 目录名"))?;
    validate_folder_key(folder_key)?;

    let root = ensure_transaction_root(data_dir)?;
    let transaction_dir = root.join(format!("txn-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&transaction_dir)
        .with_context(|| format!("创建 DLC 卸载事务失败: {}", transaction_dir.display()))?;

    let mut transaction = PendingUninstall {
        version: 1,
        folder_key: folder_key.to_string(),
        owner: owner.to_string(),
        rename_durable: false,
    };
    if let Err(error) = save_uninstall_transaction(&root, &transaction_dir, &transaction) {
        let _ = fs::remove_dir_all(&transaction_dir);
        let _ = sync_directory(&root);
        return Err(error).context("把 DLC 卸载事务刷入磁盘失败");
    }

    let quarantine = transaction_dir.join(PACKAGE_DIR);
    if let Err(error) = fs::rename(target, &quarantine) {
        let _ = fs::remove_dir_all(&transaction_dir);
        let _ = sync_directory(&root);
        return Err(error).with_context(|| {
            format!(
                "把 DLC 原子移入隔离区失败: {} -> {}",
                target.display(),
                quarantine.display()
            )
        });
    }
    // The rename is now committed. Mark it durable only after every affected
    // parent directory flushed; otherwise finalize/startup must retry and keep
    // the transaction record instead of retiring uncertain evidence.
    let durability = sync_directory(&expected_root)
        .and_then(|_| sync_directory(&transaction_dir))
        .and_then(|_| sync_directory(&root));
    match durability {
        Ok(()) => {
            transaction.rename_durable = true;
            if let Err(error) = save_uninstall_transaction(&root, &transaction_dir, &transaction) {
                tracing::warn!("[DLC] 卸载已刷盘，但持久确认记录失败: {error:#}");
            }
        }
        Err(error) => {
            tracing::warn!("[DLC] 卸载重命名尚未持久确认；本次/启动恢复会重试: {error:#}")
        }
    }
    Ok(transaction_dir)
}

/// Finish one committed quarantine transaction. Every step is idempotent; the
/// transaction record is deleted last so a crash simply causes another retry.
pub(crate) fn finalize_uninstall(data_dir: &Path, transaction_dir: &Path) -> Result<()> {
    let root = direct_transaction_dir(data_dir, transaction_dir)?;
    let mut transaction = read_transaction(transaction_dir)?;
    let installed_root = standalone_root(data_dir);
    let original = installed_root.join(&transaction.folder_key);
    let quarantine = transaction_dir.join(PACKAGE_DIR);

    if original.exists() && quarantine.exists() {
        return Err(anyhow!(
            "DLC 原目录与隔离副本同时存在，拒绝猜测应删除哪一个"
        ));
    }
    if original.exists() && !quarantine.exists() {
        // Crash happened before the atomic rename; no uninstall was committed.
        fs::remove_dir_all(transaction_dir).context("清理未提交的 DLC 卸载事务失败")?;
        sync_directory(&root).context("提交未完成卸载事务清理失败")?;
        return Ok(());
    }
    if !transaction.rename_durable {
        if !quarantine.exists() {
            return Err(anyhow!("卸载重命名尚未持久确认且隔离包缺失"));
        }
        sync_directory(&installed_root).context("重试提交卸载源目录失败")?;
        sync_directory(transaction_dir).context("重试提交卸载隔离目录失败")?;
        sync_directory(&root).context("重试提交卸载事务根目录失败")?;
        transaction.rename_durable = true;
        save_uninstall_transaction(&root, transaction_dir, &transaction)
            .context("记录卸载重命名持久确认失败")?;
    }

    crate::ai_service::game_system::script_engine::persistent_state::reset_playthrough(
        data_dir,
        &transaction.owner,
    )
    .context("清理已隔离 DLC 的周目状态失败")?;
    crate::ai_service::game_system::script_engine::events::menu_effect_event::clear_menu_effect_for_owner(
        data_dir,
        &transaction.owner,
    )
    .context("清理已隔离 DLC 的菜单效果失败")?;
    crate::ai_service::game_system::script_engine::events::character_file_event::remove_character_dir_for_owner(
        &transaction.owner,
        data_dir,
    )
    .context("清理已隔离 DLC 的角色标记失败")?;

    if quarantine.exists() {
        fs::remove_dir_all(&quarantine)
            .with_context(|| format!("删除 DLC 隔离副本失败: {}", quarantine.display()))?;
        sync_directory(transaction_dir).context("提交 DLC 隔离副本删除失败")?;
    }
    fs::remove_dir_all(transaction_dir)
        .with_context(|| format!("删除 DLC 卸载事务失败: {}", transaction_dir.display()))?;
    sync_directory(&root).context("提交 DLC 卸载事务退休失败")?;
    if fs::read_dir(&root)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
    {
        if fs::remove_dir(&root).is_ok() {
            let _ = sync_directory(data_dir);
        }
    }
    Ok(())
}

fn direct_import_transaction(data_dir: &Path, transaction_dir: &Path) -> Result<PathBuf> {
    let root = ensure_import_root(data_dir)?;
    if transaction_dir.parent() != Some(root.as_path()) {
        return Err(anyhow!("DLC 导入事务不在受控根目录内"));
    }
    let metadata = fs::symlink_metadata(transaction_dir)
        .with_context(|| format!("读取 DLC 导入事务失败: {}", transaction_dir.display()))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(anyhow!("DLC 导入事务不能是链接或普通文件"));
    }
    Ok(root)
}

fn read_install_transaction(data_dir: &Path, transaction_dir: &Path) -> Result<PendingInstall> {
    direct_import_transaction(data_dir, transaction_dir)?;
    let path = transaction_dir.join(INSTALL_FILE);
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("读取 DLC 安装提交记录失败: {}", path.display()))?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > MAX_TRANSACTION_BYTES {
        return Err(anyhow!("DLC 安装提交记录类型或大小非法"));
    }
    let record: PendingInstall =
        serde_json::from_slice(&fs::read(&path)?).context("解析 DLC 安装提交记录失败")?;
    if record.version != 1 {
        return Err(anyhow!("不支持的 DLC 安装提交记录版本"));
    }
    validate_folder_key(&record.folder_key)?;
    Ok(record)
}

fn save_install_transaction(
    root: &Path,
    transaction_dir: &Path,
    record: &PendingInstall,
) -> Result<()> {
    let path = transaction_dir.join(INSTALL_FILE);
    let bytes = serde_json::to_vec_pretty(record).context("序列化 DLC 安装提交记录失败")?;
    crate::ai_service::tools::atomic_replace(&path, &bytes)
        .map_err(|error| anyhow!("保存 DLC 安装提交记录失败: {error}"))?;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .and_then(|file| file.sync_all())
        .context("把 DLC 安装提交记录刷入磁盘失败")?;
    sync_directory(transaction_dir)?;
    sync_directory(root)
}

/// Create one extraction destination outside every script scan root.
pub(crate) fn begin_install_staging(data_dir: &Path) -> Result<PathBuf> {
    let root = ensure_import_root(data_dir)?;
    let transaction_dir = root.join(format!("import-{}", uuid::Uuid::new_v4()));
    let package = transaction_dir.join(PACKAGE_DIR);
    fs::create_dir(&transaction_dir)
        .with_context(|| format!("创建 DLC 导入事务失败: {}", transaction_dir.display()))?;
    if let Err(error) = fs::create_dir(&package) {
        let _ = fs::remove_dir(&transaction_dir);
        return Err(error).context("创建 DLC 导入包暂存目录失败");
    }
    if let Err(error) = sync_directory(&transaction_dir).and_then(|_| sync_directory(&root)) {
        let _ = fs::remove_dir_all(&transaction_dir);
        let _ = sync_directory(&root);
        return Err(error).context("把 DLC 导入暂存事务刷入磁盘失败");
    }
    Ok(package)
}

/// Commit a fully validated staged package with one same-volume rename.
pub(crate) fn commit_staged_install(
    data_dir: &Path,
    staged_package: &Path,
    target: &Path,
) -> Result<PathBuf> {
    let imports = ensure_import_root(data_dir)?;
    let transaction_dir = staged_package
        .parent()
        .ok_or_else(|| anyhow!("DLC 暂存包没有事务父目录"))?;
    if staged_package.file_name() != Some(std::ffi::OsStr::new(PACKAGE_DIR))
        || transaction_dir.parent() != Some(imports.as_path())
    {
        return Err(anyhow!("DLC 暂存包不在受控导入事务中"));
    }
    let metadata = fs::symlink_metadata(staged_package)
        .with_context(|| format!("读取 DLC 暂存包失败: {}", staged_package.display()))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(anyhow!("DLC 暂存包不能是链接或普通文件"));
    }

    let standalone = standalone_root(data_dir);
    if target.parent() != Some(standalone.as_path()) || target.exists() {
        return Err(anyhow!("DLC 最终安装路径不安全或已存在"));
    }
    let folder_key = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("无法确定 DLC 最终目录名"))?;
    validate_folder_key(folder_key)?;
    let root_metadata = fs::symlink_metadata(&standalone)
        .with_context(|| format!("读取 standalone 根目录失败: {}", standalone.display()))?;
    if is_link_like(&root_metadata) || !root_metadata.is_dir() {
        return Err(anyhow!("standalone 根目录不能是链接或普通文件"));
    }
    sync_directory_tree(staged_package).context("提交前刷新 DLC 暂存目录树失败")?;
    sync_directory(transaction_dir).context("提交前刷新 DLC 导入事务失败")?;
    save_install_transaction(
        &imports,
        transaction_dir,
        &PendingInstall {
            version: 1,
            folder_key: folder_key.to_string(),
        },
    )?;

    if let Err(error) = fs::rename(staged_package, target) {
        let _ = fs::remove_file(transaction_dir.join(INSTALL_FILE));
        let _ = sync_directory(transaction_dir);
        let _ = sync_directory(&imports);
        return Err(error).with_context(|| {
            format!(
                "提交 DLC 原子安装失败: {} -> {}",
                staged_package.display(),
                target.display()
            )
        });
    }
    for directory in [&standalone, transaction_dir, &imports] {
        if let Err(error) = sync_directory(directory) {
            tracing::warn!("[DLC] 导入提交尚未持久确认；安装事务会保留并重试: {error:#}");
        }
    }
    Ok(transaction_dir.to_path_buf())
}

/// Finish or recover a committed installation. The record remains until both
/// sides of the directory rename have been flushed.
pub(crate) fn finish_install_commit(data_dir: &Path, transaction_dir: &Path) -> Result<()> {
    let imports = direct_import_transaction(data_dir, transaction_dir)?;
    let record = read_install_transaction(data_dir, transaction_dir)?;
    let staged = transaction_dir.join(PACKAGE_DIR);
    let standalone = standalone_root(data_dir);
    let target = standalone.join(&record.folder_key);
    let staged_exists = staged.exists();
    let target_exists = target.exists();
    if staged_exists && target_exists {
        return Err(anyhow!("DLC 暂存包与最终安装目录同时存在，拒绝猜测"));
    }
    if !staged_exists && !target_exists {
        return Err(anyhow!("DLC 安装提交两侧均缺失"));
    }
    if staged_exists {
        let metadata = fs::symlink_metadata(&staged)?;
        if is_link_like(&metadata) || !metadata.is_dir() {
            return Err(anyhow!("DLC 恢复暂存包不是安全目录"));
        }
        fs::rename(&staged, &target).with_context(|| {
            format!(
                "恢复 DLC 原子安装失败: {} -> {}",
                staged.display(),
                target.display()
            )
        })?;
    }
    let target_metadata = fs::symlink_metadata(&target)?;
    if is_link_like(&target_metadata) || !target_metadata.is_dir() {
        return Err(anyhow!("DLC 最终安装目录不是安全目录"));
    }
    sync_directory(&standalone).context("提交 DLC 最终安装目录失败")?;
    sync_directory(transaction_dir).context("提交 DLC 安装事务目录失败")?;
    sync_directory(&imports).context("提交 DLC 安装事务根目录失败")?;
    fs::remove_dir_all(transaction_dir).context("退休 DLC 安装提交事务失败")?;
    sync_directory(&imports).context("提交 DLC 安装事务退休失败")?;
    Ok(())
}

/// Remove commit evidence only after a rejected live package was successfully
/// moved away. If either side still exists, keep the record for startup.
pub(crate) fn abort_install_commit(data_dir: &Path, transaction_dir: &Path) -> Result<()> {
    let imports = direct_import_transaction(data_dir, transaction_dir)?;
    let record = read_install_transaction(data_dir, transaction_dir)?;
    let staged = transaction_dir.join(PACKAGE_DIR);
    let target = standalone_root(data_dir).join(record.folder_key);
    if staged.exists() || target.exists() {
        return Err(anyhow!("DLC 安装任一侧仍存在，拒绝删除恢复证据"));
    }
    fs::remove_dir_all(transaction_dir).context("取消 DLC 安装提交事务失败")?;
    sync_directory(&imports).context("提交 DLC 安装事务取消失败")
}

/// Move a rejected staged/live package into an out-of-scan quarantine. Never
/// fall back to partial in-place deletion: if rename is blocked, startup can
/// still safely retry staged cleanup or the complete live package remains
/// visible to DLC management.
pub(crate) fn discard_rejected_install(data_dir: &Path, target: &Path) -> Result<()> {
    if !target.exists() {
        return Ok(());
    }
    let standalone = standalone_root(data_dir);
    let imports = ensure_import_root(data_dir)?;
    let source_parent = target
        .parent()
        .ok_or_else(|| anyhow!("失败安装路径没有父目录"))?;
    let staged = target.file_name() == Some(std::ffi::OsStr::new(PACKAGE_DIR))
        && source_parent.parent() == Some(imports.as_path());
    if source_parent != standalone && !staged {
        return Err(anyhow!("拒绝清理受控导入/standalone 根目录之外的失败安装"));
    }
    let metadata = fs::symlink_metadata(target)
        .with_context(|| format!("读取失败安装目录失败: {}", target.display()))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(anyhow!("失败安装根目录不能是链接或普通文件"));
    }

    let root = ensure_rejected_root(data_dir)?;
    let quarantine = root.join(format!("rejected-{}", uuid::Uuid::new_v4()));
    fs::rename(target, &quarantine).with_context(|| {
        format!(
            "把失败安装移入隔离区失败（不会在扫描区内部分删除）: {} -> {}",
            target.display(),
            quarantine.display()
        )
    })?;
    sync_directory(source_parent).context("提交失败安装源目录隔离失败")?;
    sync_directory(&root).context("提交失败安装隔离目录失败")?;

    if staged
        && fs::read_dir(source_parent)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
        && fs::remove_dir(source_parent).is_ok()
    {
        let _ = sync_directory(&imports);
    }
    if let Err(error) = fs::remove_dir_all(&quarantine) {
        tracing::warn!(
            "[DLC] 失败安装已隔离，删除将在下次启动重试 {}: {}",
            quarantine.display(),
            error
        );
    } else {
        let _ = sync_directory(&root);
    }
    Ok(())
}

fn recover_abandoned_imports(data_dir: &Path) {
    let root = match ensure_import_root(data_dir) {
        Ok(root) => root,
        Err(error) => {
            tracing::warn!("[DLC] 无法检查遗留导入暂存区: {error:#}");
            return;
        }
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if is_link_like(&metadata) || !metadata.is_dir() {
            tracing::warn!("[DLC] 拒绝遍历异常导入暂存项: {}", path.display());
            continue;
        }
        if path.join(INSTALL_FILE).exists() {
            match finish_install_commit(data_dir, &path) {
                Ok(()) => tracing::info!("[DLC] 已恢复安装提交事务: {}", path.display()),
                Err(error) => {
                    tracing::warn!("[DLC] 安装提交事务仍待重试 {}: {error:#}", path.display())
                }
            }
            continue;
        }
        if let Err(error) = fs::remove_dir_all(&path) {
            tracing::warn!("[DLC] 遗留导入暂存项仍待清理 {}: {}", path.display(), error);
        }
    }
    let _ = sync_directory(&root);
    if fs::read_dir(&root)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
        && fs::remove_dir(&root).is_ok()
    {
        let _ = sync_directory(data_dir);
    }
}

fn recover_rejected_installs(data_dir: &Path) {
    let root = match ensure_rejected_root(data_dir) {
        Ok(root) => root,
        Err(error) => {
            tracing::warn!("[DLC] 无法检查失败安装隔离区: {error:#}");
            return;
        }
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if is_link_like(&metadata) || !metadata.is_dir() {
            tracing::warn!("[DLC] 拒绝遍历异常失败安装隔离项: {}", path.display());
            continue;
        }
        if let Err(error) = fs::remove_dir_all(&path) {
            tracing::warn!("[DLC] 失败安装隔离项仍待清理 {}: {}", path.display(), error);
        }
    }
    if fs::read_dir(&root)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
    {
        let _ = fs::remove_dir(&root);
    }
}

/// Retry transactions left by a hard kill or a partial Windows directory
/// deletion. Invalid records remain quarantined and are never scanned as DLCs.
pub(crate) fn recover_pending_uninstalls(data_dir: &Path) {
    recover_abandoned_imports(data_dir);
    recover_rejected_installs(data_dir);
    let root = match ensure_transaction_root(data_dir) {
        Ok(root) => root,
        Err(error) => {
            tracing::warn!("[DLC] 无法检查待恢复卸载事务: {error:#}");
            return;
        }
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if is_link_like(&metadata) || !metadata.is_dir() {
            tracing::warn!("[DLC] 拒绝遍历异常卸载事务项: {}", path.display());
            continue;
        }
        let is_empty = fs::read_dir(&path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if is_empty {
            if fs::remove_dir(&path).is_ok() {
                let _ = sync_directory(&root);
            }
            continue;
        }
        match finalize_uninstall(data_dir, &path) {
            Ok(()) => tracing::info!("[DLC] 已恢复卸载事务: {}", path.display()),
            Err(error) => tracing::warn!(
                "[DLC] 卸载事务仍待重试（隔离包不会被加载） {}: {error:#}",
                path.display()
            ),
        }
    }
}

pub(crate) fn has_pending_for_folder(data_dir: &Path, folder_key: &str) -> Result<bool> {
    validate_folder_key(folder_key)?;
    let root = ensure_transaction_root(data_dir)?;
    for entry in fs::read_dir(&root).context("读取 DLC 卸载事务目录失败")? {
        let path = entry?.path();
        if path.is_dir() {
            match read_transaction(&path) {
                Ok(transaction) if transaction.folder_key == folder_key => return Ok(true),
                Ok(_) => {}
                Err(error) => tracing::warn!(
                    "[DLC] 跳过无法验证的隔离卸载事务（不会全局阻断导入） {}: {error:#}",
                    path.display()
                ),
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantined_package_is_not_left_in_script_tree() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "lingchat-dlc-transaction-{}-{unique}",
            std::process::id()
        ));
        let folder = format!("sample-{}-{unique}", std::process::id());
        let target = standalone_root(&data_dir).join(&folder);
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("story_config.yaml"), b"script_name: sample").unwrap();
        let owner = Path::new("standalone")
            .join(&folder)
            .to_string_lossy()
            .to_string();

        let transaction = stage_uninstall(&data_dir, &target, &owner).unwrap();
        assert!(!target.exists());
        assert!(transaction.join(PACKAGE_DIR).is_dir());
        finalize_uninstall(&data_dir, &transaction).unwrap();
        assert!(!transaction.exists());

        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn staged_install_never_extracts_inside_script_scan_tree() {
        let data_dir = std::env::temp_dir().join(format!(
            "lingchat-dlc-import-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let standalone = standalone_root(&data_dir);
        fs::create_dir_all(&standalone).unwrap();
        let staged = begin_install_staging(&data_dir).unwrap();
        assert!(!staged.starts_with(data_dir.join("game_data").join("scripts")));
        fs::write(staged.join("story_config.yaml"), b"script_name: staged").unwrap();
        let target = standalone.join("staged");
        let transaction = commit_staged_install(&data_dir, &staged, &target).unwrap();
        assert!(target.join("story_config.yaml").is_file());
        assert!(!staged.exists());
        assert!(transaction.join(INSTALL_FILE).is_file());
        finish_install_commit(&data_dir, &transaction).unwrap();
        assert!(!transaction.exists());

        let _ = fs::remove_dir_all(data_dir);
    }
}
