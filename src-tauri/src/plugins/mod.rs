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

pub mod http_host;
pub mod manager;
pub mod manifest;
pub mod python_backend;
pub mod tool;
pub mod types;

pub use manager::PluginManager;
pub use types::PluginInfo;
