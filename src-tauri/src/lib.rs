//! Tauri 外壳 crate。
//!
//! 业务模块（api / ai_service / db / …）已拆至 `ling-chat-main`，插件系统拆至
//! `ling-chat-plugins`；本 crate 只保留 Tauri 外壳：`lib.rs` / `main.rs` 与
//! `app/` 组合根（状态注入、命令注册、setup 编排、日志与平台适配）。

mod app;

// 全局状态容器定义在 `ling_chat_main::state`，这里重导出以保持外壳内部
// `crate::AppState` 等既有路径不变。
pub use ling_chat_main::{AppState, ChatComponents, InnerAppState, ScreenshotCaptureState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // TLS 兜底：rustls 依赖图同时启用 aws-lc-rs（本项目显式）与 ring
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // 初始化日志系统，并拿到 genai 调试开关的热重载句柄（setup 阶段使用）。
    // 句柄经 `setup` 的 `FnOnce` 闭包移入 `app::setup::setup`。
    let log_filter = app::logging::init_tracing();

    // 提前构建 Tauri 上下文（读取 bundle identifier，供 Windows HDR 开关定位 settings.json）
    let context = tauri::generate_context!();

    // Windows：设置 WebView2 颜色配置文件（强制使用线性 sRGB）。
    app::platform::apply_webview2_color_profile(&context.config().identifier);

    app::builder::build()
        .setup(move |app| app::setup::setup(app, log_filter))
        .run(context)
        .expect("error while running tauri application");
}
