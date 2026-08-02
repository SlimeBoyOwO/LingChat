use std::path::{Path, PathBuf};

/// 将角色资源路径解析为绝对路径。
///
/// 相对路径统一放在 `data/game_data/characters` 下，绝对路径保持不变。
pub fn resolve_character_path(data_dir: &Path, resource_path: &str) -> PathBuf {
    let path = PathBuf::from(resource_path);
    if path.is_absolute() {
        path
    } else {
        data_dir.join("game_data").join("characters").join(path)
    }
}

/// 批量创建目录（幂等）。任一失败立即返回错误。
pub fn ensure_dirs(dirs: &[&Path]) -> Result<(), String> {
    for d in dirs {
        std::fs::create_dir_all(d)
            .map_err(|e| format!("create_dir_all {}: {e}", d.display()))?;
    }
    Ok(())
}
