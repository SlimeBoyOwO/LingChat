pub mod achievement;
pub mod adventure;
pub mod ambient;
pub mod asr;
pub mod asset;
pub mod background;
pub mod character;
pub mod chat;
pub mod codex;
pub mod font;
pub mod game;
pub mod live2d;
pub mod locale;
pub mod music;
pub mod pet;
pub mod player;
pub mod plugins;
pub mod save;
pub mod scene;
pub mod schedule;
pub mod screenshot;
pub mod script;
pub mod script_editor;
pub mod settings;
pub mod tool_settings;
pub mod workshop;

use std::path::PathBuf;

use crate::AppState;
use tauri::Manager;

// ========== 共享辅助函数 ==========

/// 资源来源字段默认值（游戏自有）。
pub(crate) fn default_source() -> String {
    "game".to_string()
}

/// 文件修改时间戳（秒，含小数），读取失败返回 "0"。
pub(crate) fn mtime_secs(path: &std::path::Path) -> String {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64().to_string())
                .unwrap_or_else(|_| "0".to_string())
        })
        .unwrap_or_else(|| "0".to_string())
}

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

/// 插件角色的 `resource_folder` 编码前缀：`plugin:<plugin_id>/<folder>`。
pub const PLUGIN_ROLE_PREFIX: &str = "plugin:";

/// 编码插件角色的 resource_folder 值。
pub fn encode_plugin_folder(plugin_id: &str, folder: &str) -> String {
    format!("{PLUGIN_ROLE_PREFIX}{plugin_id}/{folder}")
}

/// 解析插件角色编码值为 (plugin_id, folder)。非插件编码返回 None。
pub fn decode_plugin_folder(resource_folder: &str) -> Option<(&str, &str)> {
    let rest = resource_folder.strip_prefix(PLUGIN_ROLE_PREFIX)?;
    let (plugin_id, folder) = rest.split_once('/')?;
    if plugin_id.is_empty() || folder.is_empty() {
        return None;
    }
    Some((plugin_id, folder))
}

/// 把角色的 resource_folder 解析为实际目录（游戏 characters 或插件 characters）。
///
/// 这是所有「硬编码 characters_dir().join(resource_folder)」的统一替换点：
/// `plugin:<id>/<folder>` → `data/plugins/<id>/characters/<folder>`；
/// 其余 → `game_data/characters/<folder>`（原行为）。
pub fn resolve_character_dir(resource_folder: &str) -> PathBuf {
    resolve_character_dir_in(&data_dir(), resource_folder)
}

/// `resolve_character_dir` 的显式 base 版本（供已持有 data_dir 的调用方使用，如 role_repo）。
/// 复用底层 `utils::path::resolve_character_path`（其内部已处理 `plugin:` 编码前缀）。
pub fn resolve_character_dir_in(base_data_dir: &std::path::Path, resource_folder: &str) -> PathBuf {
    crate::utils::path::resolve_character_path(base_data_dir, resource_folder)
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

pub(crate) fn fonts_dir() -> PathBuf {
    data_dir().join("fonts")
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
pub mod role_archive;
