//! 压缩包导入的「暂存 → 定位 → 落位」辅助。
//!
//! 角色导入（`api::role_archive`）与插件导入（`plugins::importer`）共用：两者都是
//! 解压到暂存目录、判定内容根目录、再把内容搬进最终位置。搬运失败的回退
//! （目标被占用 / 跨设备）与目录结构判定规则对两类资源完全一致，故收敛在此。

use std::path::{Path, PathBuf};

/// 归档内的 macOS 元数据，判定目录结构时一并忽略。
const JUNK_NAMES: &[&str] = &["__MACOSX", ".DS_Store"];

fn is_junk(name: &str) -> bool {
    JUNK_NAMES.contains(&name) || name.starts_with("._")
}

/// 忽略 macOS 元数据后列出目录内容，返回 `(子目录列表, 是否存在普通文件)`。
pub async fn list_content_entries(dir: &Path) -> (Vec<PathBuf>, bool) {
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut has_files = false;
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if is_junk(&name) {
                continue;
            }
            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => subdirs.push(entry.path()),
                Ok(ft) if ft.is_file() => has_files = true,
                _ => {}
            }
        }
    }
    (subdirs, has_files)
}

/// 定位解压后的内容根目录。
///
/// 暂存目录只含一个子目录且没有散落文件时，说明压缩包外层套了一个同名文件夹，
/// 返回该子目录；否则内容直接位于暂存目录根部，返回暂存目录本身。
pub async fn locate_extracted_root(staging: &Path) -> PathBuf {
    let (subdirs, has_files) = list_content_entries(staging).await;
    if subdirs.len() == 1 && !has_files {
        if let Some(dir) = subdirs.into_iter().next() {
            return dir;
        }
    }
    staging.to_path_buf()
}

/// 把目录搬到最终位置。
///
/// 同一磁盘优先 `rename`（零拷贝）；被句柄占用或权限拒绝时退避重试，三次仍失败
/// 退化为递归复制并在成功后删除源目录。调用方负责源目录位于暂存区内。
pub async fn relocate_dir(src: &Path, dst: &Path) -> Result<(), String> {
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 1..=3 {
        match tokio::fs::rename(src, dst).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(150 * attempt as u64)).await;
                }
            }
        }
    }
    let rerr = last_err.map(|e| e.to_string()).unwrap_or_default();
    tracing::warn!(
        "[Archive] relocate_dir rename 3 次均失败: src={}, dst={}, err={}",
        src.display(),
        dst.display(),
        rerr
    );
    let src_owned = src.to_path_buf();
    let dst_owned = dst.to_path_buf();
    let copy_res = tokio::task::spawn_blocking(move || copy_dir_recursive(&src_owned, &dst_owned))
        .await
        .map_err(|je| format!("移动目录失败 (rename: {rerr}; 复制回退线程异常: {je})"))?;
    copy_res.map_err(|cerr| {
        format!("移动目录失败 (rename: {rerr}; 复制回退: {cerr})，目标可能正被其他进程占用")
    })?;
    let _ = tokio::fs::remove_dir_all(src).await;
    Ok(())
}

/// 递归复制目录，作为重命名失败时的回退方案。
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
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
