//! 应用配置模块。
//!
//! 子模块：
//! - `keys`：settings.json 存储键常量
//! - `types`：前端配置树的类型定义
//! - `app_config`：AppConfig 结构体、默认值、store 读写
//! - `proactive`：ProactiveConfig（主动对话系统）
//! - `tts`：TtsConfig（TTS 引擎配置）
//! - `tree`：build_config_tree()（前端"高级设置"页面数据源）

pub mod app_config;
pub mod keys;
pub mod proactive;
pub mod session;
pub mod tree;
pub mod tts;
pub mod types;

pub const STORE_FILE: &str = "settings.json";

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::{Store, StoreExt};

// 向后兼容：保持原有公开 API 路径不变
pub use app_config::{get_setting_string, AppConfig};
pub use tree::build_config_tree;
pub use types::{ConfigSetting, ConfigTree};

/// settings.json 在 DATA_DIR 下的绝对路径（外部存储，与 DB/游戏数据同目录）。
/// 统一配置存储目录，避免 Android 内部/外部存储生命周期不同导致配置丢失。
pub fn store_path() -> PathBuf {
    crate::init::static_copy::get_data_dir().join("settings.json")
}

/// 打开 settings.json 对应的持久化 store（统一在 DATA_DIR 外部存储）。
pub fn settings_store(app: &AppHandle) -> Result<Arc<Store<Wry>>> {
    app.store(store_path())
        .context("Failed to open settings store")
}

// ========== "当前进行"存档（galgame 语义） ==========

/// 某角色的"当前进行"存档 id 在 settings.json 里的 key。
pub fn last_save_key(role_id: i32) -> String {
    format!("{}{}", keys::LAST_SAVE_ID_PREFIX, role_id)
}

/// 上次游玩的角色 ID（与 load_default_character / select_character 一致）。
pub fn get_last_character_id(app: &AppHandle) -> Option<i32> {
    let store = settings_store(app).ok()?;
    store
        .get(keys::LAST_CHARACTER_ID)
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
}

/// 读取某角色的"当前进行"存档 id（不校验存档是否存在，由调用方负责）。
pub fn get_last_save_id(app: &AppHandle, role_id: i32) -> Option<i32> {
    let store = settings_store(app).ok()?;
    store
        .get(&last_save_key(role_id))
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
}

/// 记录某角色的"当前进行"存档 id（启动/继续恢复用）。失败静默忽略，不影响主流程。
/// 值未变化时跳过写盘（减少 settings.json 全量写频率，降低外部存储写坏风险）。
pub fn set_last_save_id(app: &AppHandle, role_id: i32, save_id: i32) {
    let Ok(store) = settings_store(app) else {
        return;
    };
    let key = last_save_key(role_id);
    if store
        .get(&key)
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        == Some(save_id)
    {
        return;
    }
    backup_settings_file(app);
    store.set(key, JsonValue::Number((save_id as i64).into()));
    let _ = store.save();
}

// ========== settings.json 写坏兜底 ==========

/// settings.json 的备份路径（`settings.json.bak`）。
fn settings_backup_path() -> PathBuf {
    let mut p = store_path();
    p.set_extension("json.bak");
    p
}

/// 保存前把现有 settings.json 备份为 .bak。
/// tauri-plugin-store 的 save() 是 `fs::write` 直接覆盖写（非原子），
/// Android 外部存储（FUSE）写盘中途被杀会损坏文件 → 下次启动解析失败 → 空 store 覆盖 → 配置"离奇重置"。
pub fn backup_settings_file(app: &AppHandle) {
    let path = store_path();
    if path.exists() {
        let _ = std::fs::copy(&path, settings_backup_path());
    }
}

/// 启动时检测 settings.json 是否损坏，若 .bak 完好则自动恢复。
pub fn recover_settings_if_corrupted() {
    let path = store_path();
    let bak = settings_backup_path();
    if !path.exists() || !bak.exists() {
        return;
    }
    let corrupted = std::fs::read_to_string(&path)
        .map(|s| serde_json::from_str::<serde_json::Value>(&s).is_err())
        .unwrap_or(true);
    if !corrupted {
        return;
    }
    let bak_ok = std::fs::read_to_string(&bak)
        .map(|s| serde_json::from_str::<serde_json::Value>(&s).is_ok())
        .unwrap_or(false);
    if bak_ok {
        match std::fs::copy(&bak, &path) {
            Ok(_) => tracing::warn!("[Config] settings.json 损坏，已从 .bak 恢复"),
            Err(e) => tracing::warn!("[Config] settings.json 损坏且恢复失败: {e}"),
        }
    } else {
        tracing::warn!("[Config] settings.json 损坏，.bak 也无效，跳过恢复");
    }
}
