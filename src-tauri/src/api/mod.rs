pub mod asset;
pub mod background;
pub mod character;
pub mod chat;
pub mod game;
pub mod media;
pub mod save;
pub mod script;
pub mod pet;

use std::path::PathBuf;

use crate::init::static_copy;

// ========== 共享路径辅助函数 ==========

pub(crate) fn data_dir() -> PathBuf {
    static_copy::resolve_data_dir()
}

pub(crate) fn game_data_dir() -> PathBuf {
    data_dir().join("game_data")
}

pub(crate) fn characters_dir() -> PathBuf {
    game_data_dir().join("characters")
}

pub(crate) fn backgrounds_dir() -> PathBuf {
    game_data_dir().join("backgrounds")
}

pub(crate) fn voice_dir() -> PathBuf {
    data_dir().join("voice")
}

/// 路径穿越防护：验证 canonical 路径是否以预期的基础目录开头
pub(crate) fn validate_path_in_base(resolved: &PathBuf, base: &PathBuf) -> Result<(), String> {
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
