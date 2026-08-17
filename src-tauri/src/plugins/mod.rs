//! 插件系统：声明式 TOML manifest + RustPython 脚本后端。
//!
//! 插件是 `data/plugins/<id>/` 目录，含 `manifest.toml`（工具声明）与若干
//! `.py` 脚本。启用后工具注册进 `ToolRegistry`，AI 即可调用；执行时用
//! 嵌入的 RustPython 跑脚本，脚本通过注入的 `ctx` 使用受限能力
//! （HTTP、白名单环境变量），无法访问文件系统/执行命令。
//!
//! # 平台分布
//!
//! - `types` / `manifest` / `installer`：纯数据与文件逻辑，**全平台编译**
//!   （市场安装链路在移动端也要可用）
//! - `http_host` / `python_backend` / `tool` / `manager`：依赖 RustPython，
//!   仅桌面端编译（移动端构建时依赖不可用）
//!
//! # 公开 API
//!
//! - [`PluginManager`](manager::PluginManager)：扫描、启停、配置持久化
//! - [`PluginInfo`](types::PluginInfo)：暴露给前端的插件信息
//! - [`manifest::parse`](manifest::parse)：解析并校验 manifest.toml

pub mod installer;
pub mod manifest;
pub mod types;

#[cfg(desktop)]
pub mod http_host;
#[cfg(desktop)]
pub mod manager;
#[cfg(desktop)]
pub mod python_backend;
#[cfg(desktop)]
pub mod tool;

#[cfg(desktop)]
pub use manager::PluginManager;
pub use types::PluginInfo;
