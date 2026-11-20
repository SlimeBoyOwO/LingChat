//! LingChat 业务主 crate。

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
