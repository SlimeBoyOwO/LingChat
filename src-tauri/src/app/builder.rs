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
        .plugin(tauri_plugin_process::init())
        .plugin({
            use crate::ai_service::asr::global_hotkey::{self, PttGlobalEvent};
            use tauri::Emitter;
            use tauri_plugin_global_shortcut::ShortcutState;

            // with_handler：对任意已注册的快捷键统一转发（插件事件分发自带
            // 已注册表查询，本功能只注册一个 PTT 键，无需按键分发；事件由
            // 设置页保存 / 启动恢复注册触发）。handler 内按 HotKey 相等判别
            // 是否为 PTT 键（global_hotkey::is_ptt_shortcut，PR 审查——字符串
            // 比较不可靠：注册串与 HotKey 规范化输出格式不一致；后续引入其它
            // 全局快捷键时各自比对，互不误触发）。
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // 非 PTT 键的事件直接丢弃（扩展性，PR 审查）
                    if !global_hotkey::is_ptt_shortcut(app, shortcut) {
                        return;
                    }
                    let state = match event.state() {
                        ShortcutState::Pressed => "pressed",
                        ShortcutState::Released => "released",
                    };
                    // main 是唯一运行 useAsrInput 的窗口（设置/日志窗口无 ASR）
                    let _ = app.emit_to("main", "asr:ptt-global", PttGlobalEvent { state });
                })
                .build()
        });

    super::commands::register(builder)
}
