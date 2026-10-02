//! LingChat 业务主 crate。
//!
//! 原本是 `src-tauri` 内的单体模块集合，拆分为独立 crate 后，`src-tauri` 只保留
//! Tauri 外壳（lib.rs / main.rs 与 app 装配层），业务模块全部在此，由外壳与
//! `ling-chat-plugins` 共同依赖。
//!
//! 模块一律 `pub mod`：插件 crate 需要经此访问 `AppState`、`api`、`db`、
//! `ai_service` 等宿主能力。

pub mod achievements;
pub mod adventures;
pub mod ai_service;
pub mod api;
pub mod cast;
pub mod config;
pub mod data_dir;
pub mod db;
pub mod lan_sync;
pub mod manifest;
pub mod migration;
pub mod plugin_contract;
pub mod resource_sync;
pub mod state;
pub mod utils;

// 全局状态容器定义在 `state`，这里重导出以保持 `crate::AppState` 等既有路径不变。
pub use state::{AppState, ChatComponents, InnerAppState, ScreenshotCaptureState};
