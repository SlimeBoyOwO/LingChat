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
    // Android：panic 只写 stderr 且会被 `stop_unwind` 捕获后 abort，现场看不到原因；
    // 先挂 logcat 桥，保证启动早期（日志系统就绪前）的 panic 可诊断。
    app::platform::install_panic_logger();

    // TLS 兜底：rustls 依赖图同时启用 aws-lc-rs（本项目显式）与 ring
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    app::platform::early_log("run(): TLS provider 就绪");

    // 初始化日志系统，并拿到 genai 调试开关的热重载句柄（setup 阶段使用）。
    // 句柄经 `setup` 的 `FnOnce` 闭包移入 `app::setup::setup`。
    let log_filter = app::logging::init_tracing();
    app::platform::early_log("run(): tracing 已装配");

    // 提前构建 Tauri 上下文（读取 bundle identifier，供 Windows HDR 开关定位 settings.json）
    let context = tauri::generate_context!();

    // Windows：设置 WebView2 颜色配置文件（强制使用线性 sRGB）。
    app::platform::apply_webview2_color_profile(&context.config().identifier);

    let result = app::builder::build()
        .setup(move |app| app::setup::setup(app, log_filter))
        .run(context);

    // 事件循环返回 Err 时会由下面的 `expect` panic（移动端再被 `stop_unwind` 转成
    // abort）——先把完整错误链写进 logcat，否则设备上只剩一个没有原因的闪退。
    if let Err(error) = &result {
        app::platform::early_log(&format!("run(): 返回错误: {error}"));
        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            app::platform::early_log(&format!("run():   caused by: {cause}"));
            source = cause.source();
        }
    } else {
        app::platform::early_log("run(): 事件循环正常退出");
    }

    result.expect("error while running tauri application");
}
