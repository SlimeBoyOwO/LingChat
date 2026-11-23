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
    // Android 上 stderr 通向 /dev/null，且 panic 会被 stop_unwind 捕获后 abort；
    // 先挂 logcat 桥，启动早期失败才有记录。
    app::platform::install_panic_logger();

    // TLS 兜底：rustls 依赖图同时启用 aws-lc-rs（本项目显式）与 ring
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // 初始化日志系统，并拿到 genai 调试开关的热重载句柄（setup 阶段使用）。
    // 句柄经 `setup` 的 `FnOnce` 闭包移入 `app::setup::setup`。
    let log_filter = app::logging::init_tracing();

    // 提前构建 Tauri 上下文（读取 bundle identifier，供 Windows HDR 开关定位 settings.json）
    let context = tauri::generate_context!();

    // Windows：设置 WebView2 颜色配置文件（强制使用线性 sRGB）。
    app::platform::apply_webview2_color_profile(&context.config().identifier);

    let result = app::builder::build()
        .setup(move |app| {
            let result = app::setup::setup(app, log_filter);
            if let Err(error) = &result {
                // setup 返回 Err 时 Tauri 只会 panic 最外层消息，底层原因须在这里留痕。
                app::platform::log_error_chain("setup", error.as_ref());
            }
            result
        })
        .run(context);

    if let Err(error) = result {
        app::platform::log_error_chain("事件循环", &error);
        panic!("error while running tauri application: {error}");
    }
}
