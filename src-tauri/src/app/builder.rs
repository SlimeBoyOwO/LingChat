//! `tauri::Builder` 构建与插件注册。
//!
//! 本模块原先位于 crate 根（`lib.rs`）。泛型显式写成 `tauri::Wry`，不依赖
//! tauri 的 `#[default_runtime]` 属性宏（该宏只在 `wry` feature 开启时提供默认泛型）。

/// 构建 Tauri 应用：注册全部插件并挂上命令注册表。
pub fn build() -> tauri::Builder<tauri::Wry> {
    // 构建 Tauri 应用
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_screenshots::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_android_fs::init());

    // 桌面端额外插件
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    super::commands::register(builder)
}
