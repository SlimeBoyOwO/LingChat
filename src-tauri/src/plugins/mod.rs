//! 插件系统：声明式 TOML manifest + RustPython 脚本后端。
//!
//! 插件是 `data/plugins/<id>/` 目录，含 `manifest.toml`（工具声明）与若干
//! `.py` 脚本。启用后工具注册进 `ToolRegistry`，AI 即可调用；执行时用
//! 嵌入的 RustPython 跑脚本，脚本通过注入的 `ctx` 使用受限能力
//! （HTTP、白名单环境变量），无法访问文件系统/执行命令。
//!
//! # 公开 API
//!
//! - [`PluginManager`](manager::PluginManager)：扫描、启停、配置持久化
//! - [`PluginInfo`](types::PluginInfo)：暴露给前端的插件信息
//! - [`manifest::parse`](manifest::parse)：解析并校验 manifest.toml
//! - [`importer::do_import_plugin`](importer::do_import_plugin)：从 zip/7z 压缩包安装插件
//! - [`signal::SignalRegistry`](signal::SignalRegistry)：宿主信号登记与插件订阅派发

pub mod host_api;
pub mod importer;
pub mod manager;
pub mod manifest;
pub mod python_backend;
pub mod resources;
pub mod signal;
pub mod tool;
pub mod types;

use std::sync::OnceLock;

use tauri::AppHandle;

pub use manager::PluginManager;
pub use resources::PluginResourceEntry;
pub use types::{PluginInfo, ResourceKind};

/// 宿主 `AppHandle` 的全局副本。
///
/// 插件脚本在 `spawn_blocking` 线程里执行，那条路径上没有调用方的 `AppHandle`，
/// 而少数宿主能力（如 `plugin_host.switch_character`）必须经它访问 `AppState`。
/// 启动时登记一次；未登记时相关能力返回错误，而不是 panic。
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// 启动时登记宿主句柄（与 `utils::log_bridge::set_app_handle` 同一时机）。
pub fn set_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// 取宿主句柄；启动完成前为 `None`。
pub(crate) fn app_handle() -> Option<AppHandle> {
    APP_HANDLE.get().cloned()
}
