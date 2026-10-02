//! 应用外壳模块。
//!
//! 子模块：
//! - `logging`：tracing 装配、日志设置应用、genai 调试开关热重载
//! - `platform`：平台适配（Windows WebView2 颜色配置文件）
//! - `builder`：`tauri::Builder` 构建与插件注册
//! - `commands`：全部 Tauri 命令的注册表（`generate_handler!`）
//! - `setup`：Tauri setup 阶段的启动编排
//!
//! 与 `init` 的分工：
//! - `init`：**数据与服务引导**——种子数据目录、打开数据库、同步角色、构建 AIService 等。
//! - `app`：**应用外壳**——持有全局状态、装配 Tauri、注册命令、初始化日志与平台适配。
//!
//! 全局状态容器 `AppState` 等已随业务模块拆入 `ling_chat_main::state`，
//! 由外壳 `lib.rs` 以 `pub use` 重导出，因此 `crate::AppState` 等路径保持不变。

pub mod builder;
pub mod commands;
pub mod logging;
pub mod platform;
pub mod setup;
