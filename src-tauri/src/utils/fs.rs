//! 通用文件系统操作工具。

use std::path::{Path, PathBuf};

/// 确保目标父目录存在后复制文件，返回目标路径。
///
/// 等价于 `create_dir_all(parent) + copy`，消除项目中散落的重复模式。
pub fn copy_with_parent(src: &Path, dst: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create_dir_all {}: {e}", parent.display()))?;
    }
    std::fs::copy(src, dst).map_err(|e| format!("copy {} -> {}: {e}", src.display(), dst.display()))?;
    Ok(dst.to_path_buf())
}
