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
use std::sync::Arc;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::{Store, StoreExt};

// 向后兼容：保持原有公开 API 路径不变
pub use app_config::{get_setting_string, AppConfig};
pub use tree::build_config_tree;
pub use types::{ConfigSetting, ConfigTree};

/// 打开 settings.json 对应的持久化 store。
pub fn settings_store(app: &AppHandle) -> Result<Arc<Store<Wry>>> {
    app.store(STORE_FILE)
        .context("Failed to open settings store")
}
