pub mod achievement;
pub mod adventure;
pub mod ambient;
pub mod asset;
pub mod background;
pub mod character;
pub mod chat;
pub mod game;
pub mod music;
pub mod pet;
pub mod save;
pub mod scene;
pub mod schedule;
pub mod screenshot;
pub mod script;
pub mod workshop;

use std::path::PathBuf;

use tauri::Manager;
use crate::AppState;

// ========== 共享路径辅助函数 ==========

pub(crate) fn data_dir() -> PathBuf {
    crate::init::static_copy::get_data_dir().clone()
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

pub(crate) fn music_dir() -> PathBuf {
    game_data_dir().join("musics")
}

pub(crate) fn ambient_dir() -> PathBuf {
    game_data_dir().join("ambients")
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

// ========== 主动对话系统指令 ==========

/// 前端通知后端当前是否具备主动对话投放条件。
/// 仅在最终布尔值翻转时调用。
#[tauri::command]
pub async fn proactive_set_can_deliver(
    app: tauri::AppHandle,
    can_deliver: bool,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    if let Some(ref ps) = state.proactive_system {
        ps.lock().await.set_can_deliver(can_deliver);
    }
    Ok(())
}
