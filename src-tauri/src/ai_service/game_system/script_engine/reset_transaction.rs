//! Durable, forward-recoverable script reset intent.
//!
//! Ordinary command failures still use in-memory byte snapshots for rollback.
//! This tiny journal covers process/power interruption: once progress detaches,
//! startup can generically restore declared markers and clear the owner's menu.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::ai_service::types::ScriptStatus;

const RESET_ROOT: &str = ".script-reset";
const MAX_RECORD_BYTES: u64 = 8 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct PendingReset {
    version: u32,
    owner: String,
}

#[cfg(windows)]
fn is_link_like(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_like(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn validate_owner(owner: &str) -> Result<()> {
    if owner.is_empty() || owner.chars().count() > 512 || owner.chars().any(char::is_control) {
        return Err(anyhow!("剧本重置 owner 非法"));
    }
    Ok(())
}

fn ensure_root(data_dir: &Path) -> Result<PathBuf> {
    let root = data_dir.join(RESET_ROOT);
    match fs::symlink_metadata(&root) {
        Ok(metadata) => {
            if is_link_like(&metadata) || !metadata.is_dir() {
                return Err(anyhow!("剧本重置事务根目录不能是链接或普通文件"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&root)
                .with_context(|| format!("创建剧本重置事务目录失败: {}", root.display()))?;
            super::dlc_transaction::sync_directory(data_dir)
                .context("提交剧本重置事务根目录失败")?;
        }
        Err(error) => return Err(error).context("读取剧本重置事务目录失败"),
    }
    Ok(root)
}

fn read_record(path: &Path, root: &Path) -> Result<PendingReset> {
    if path.parent() != Some(root) {
        return Err(anyhow!("剧本重置事务不在受控根目录"));
    }
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("读取剧本重置事务失败: {}", path.display()))?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > MAX_RECORD_BYTES {
        return Err(anyhow!("剧本重置事务记录类型或大小非法"));
    }
    let record: PendingReset =
        serde_json::from_slice(&fs::read(path)?).context("解析剧本重置事务失败")?;
    if record.version != 1 {
        return Err(anyhow!("不支持的剧本重置事务版本"));
    }
    validate_owner(&record.owner)?;
    Ok(record)
}

pub(crate) fn begin_reset(data_dir: &Path, owner: &str) -> Result<PathBuf> {
    validate_owner(owner)?;
    let root = ensure_root(data_dir)?;
    let path = root.join(format!("reset-{}.json", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(&PendingReset {
        version: 1,
        owner: owner.to_string(),
    })
    .context("序列化剧本重置事务失败")?;
    crate::ai_service::tools::atomic_replace(&path, &bytes)
        .map_err(|error| anyhow!("保存剧本重置事务失败: {error}"))?;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .and_then(|file| file.sync_all())
        .context("把剧本重置事务刷入磁盘失败")?;
    super::dlc_transaction::sync_directory(&root).context("提交剧本重置事务目录项失败")?;
    Ok(path)
}

pub(crate) fn finish_reset(data_dir: &Path, record_path: &Path) -> Result<()> {
    let root = ensure_root(data_dir)?;
    if record_path.parent() != Some(root.as_path()) {
        return Err(anyhow!("拒绝完成受控目录外的剧本重置事务"));
    }
    match fs::remove_file(record_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("删除剧本重置事务记录失败"),
    }
    super::dlc_transaction::sync_directory(&root).context("提交剧本重置事务完成状态失败")?;
    if fs::read_dir(&root)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
        && fs::remove_dir(&root).is_ok()
    {
        let _ = super::dlc_transaction::sync_directory(data_dir);
    }
    Ok(())
}

pub(crate) fn recover_pending_resets(data_dir: &Path, scripts: &HashMap<String, ScriptStatus>) {
    let root = match ensure_root(data_dir) {
        Ok(root) => root,
        Err(error) => {
            tracing::warn!("[ScriptReset] 无法检查重置事务: {error:#}");
            return;
        }
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let record = match read_record(&path, &root) {
            Ok(record) => record,
            Err(error) => {
                tracing::warn!("[ScriptReset] 重置事务无效并保留: {error:#}");
                continue;
            }
        };
        let Some(script) = scripts
            .values()
            .find(|script| script.path_key() == record.owner)
        else {
            tracing::warn!(
                "[ScriptReset] 待恢复剧本当前不可用，保留事务: {}",
                record.owner
            );
            continue;
        };
        let result = (|| -> Result<()> {
            super::persistent_state::reset_playthrough(data_dir, &record.owner)
                .context("恢复重置时清理周目状态失败")?;
            super::events::character_file_event::restore_declared_character_files(script, data_dir)
                .context("恢复重置时还原角色标记失败")?;
            super::events::menu_effect_event::clear_menu_effect_for_owner(data_dir, &record.owner)
                .context("恢复重置时清理菜单特效失败")?;
            finish_reset(data_dir, &path)
        })();
        match result {
            Ok(()) => tracing::info!("[ScriptReset] 已恢复重置事务: {}", record.owner),
            Err(error) => {
                tracing::warn!("[ScriptReset] 重置事务仍待重试 {}: {error:#}", record.owner)
            }
        }
    }
}
