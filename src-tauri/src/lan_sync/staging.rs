//! 锁定文件暂存机制。
//!
//! 同步时若目标文件被其他进程锁定（如 SQLite 数据库），
//! 无法直接重命名覆盖。此时将文件暂存到 `data/.lan_sync_staging/`，
//! 下次启动时在数据库初始化之前应用。

use std::path::{Path, PathBuf};

use tracing::{info, warn};

const STAGING_DIR: &str = ".lan_sync_staging";

/// 获取暂存目录路径。
pub fn staging_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(STAGING_DIR)
}

/// 将文件暂存到 `.lan_sync_staging/`，保持相对路径结构。
///
/// 下次启动时由 [`apply_staged_files`] 自动应用。
pub fn stage_file(data_dir: &Path, relative_path: &str, tmp_path: &Path) -> Result<(), String> {
    let staging = staging_dir(data_dir);
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("创建暂存目录失败: {e}"))?;

    // 在暂存目录中保持相对路径（如 game_database.db）
    let dest = staging.join(relative_path);
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::copy(tmp_path, &dest)
        .map_err(|e| format!("暂存文件失败 [{}]: {}", relative_path, e))?;

    info!("文件已暂存（重启后生效）: {}", relative_path);
    Ok(())
}

/// 启动时应用所有暂存文件。
///
/// **必须在数据库初始化之前调用**，否则 `.db` 等文件仍会被锁定。
pub fn apply_staged_files(data_dir: &Path) -> u64 {
    let staging = staging_dir(data_dir);
    if !staging.exists() {
        return 0;
    }

    info!("检测到暂存文件，开始应用...");

    let count = match apply_dir(&staging, data_dir) {
        Ok(c) => c,
        Err(e) => {
            warn!("应用暂存文件出错: {}", e);
            0
        }
    };

    // 清理暂存目录
    if let Err(e) = std::fs::remove_dir_all(&staging) {
        warn!("清理暂存目录失败: {}", e);
    } else if count > 0 {
        info!("已应用 {} 个暂存文件，暂存目录已清理", count);
    }

    count
}

/// 递归应用暂存目录中的文件到目标基准目录。
fn apply_dir(staging: &Path, target_base: &Path) -> Result<u64, String> {
    let mut count = 0;
    for entry in
        std::fs::read_dir(staging).map_err(|e| format!("读取暂存目录失败: {}", e))?
    {
        let entry = entry.map_err(|e| format!("读取暂存条目失败: {}", e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            let sub_target = target_base.join(&name);
            count += apply_dir(&path, &sub_target)?;
        } else {
            let target = target_base.join(&name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("创建目标目录失败 [{}]: {}", target.display(), e)
                })?;
            }
            // 直接重命名（此时应用刚启动，文件未被锁定）
            std::fs::rename(&path, &target)
                .map_err(|e| format!("应用暂存文件失败 [{}]: {}", name, e))?;
            info!("已应用暂存文件: {}", name);
            count += 1;
        }
    }
    Ok(count)
}
