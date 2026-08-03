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

/// 路径穿越防护：验证 canonical 路径是否以预期的基础目录开头。
///
/// 原为 `api/mod.rs` 下的共享辅助，迁到 utils 后各域（编辑器路径解析、
/// 局域网同步、字体/素材校验等）都能复用。
pub fn validate_path_in_base(resolved: &Path, base: &Path) -> Result<(), String> {
    let canon_resolved = resolved
        .canonicalize()
        .map_err(|e| format!("路径解析失败: {} - 路径: {:?}", e, resolved))?;

    let canon_base = base
        .canonicalize()
        .map_err(|e| format!("基础目录解析失败: {} - 路径: {:?}", e, base))?;

    if !canon_resolved.starts_with(&canon_base) {
        return Err(format!(
            "非法路径：试图访问基础目录之外的文件\n\
             请求路径: {:?}\n\
             规范路径: {:?}\n\
             基础目录: {:?}\n\
             规范基础目录: {:?}",
            resolved, canon_resolved, base, canon_base
        ));
    }
    Ok(())
}
