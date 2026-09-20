//! 平台适配。
//!
//! 目前只处理 Windows 上的 WebView2 颜色配置文件：它必须在 WebView2 环境创建
//! （`Builder::build()`）**之前**设置，因此无法走 Tauri 的设置体系，只能在此处
//! 直接读 store 文件并写进程环境变量。本模块原先位于 crate 根（`lib.rs`）。

/// 读取 settings.json 中的「HDR 模式」开关（仅 Windows）。
///
/// 必须在 WebView2 环境创建（`Builder::build()`）之前调用——此时 `AppHandle` 尚不存在，
/// 只能直接解析 store 文件。store 位于 `%APPDATA%\<identifier>\settings.json`
/// （tauri-plugin-store 的 flat 点号键）。文件缺失/解析失败一律视为「未开启」。
#[cfg(target_os = "windows")]
fn read_hdr_mode_enabled(identifier: &str) -> bool {
    use serde_json::Value;

    let Some(appdata) = std::env::var("APPDATA").ok() else {
        return false;
    };
    let path = std::path::Path::new(&appdata)
        .join(identifier)
        .join(crate::config::STORE_FILE);

    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    json.get(crate::config::keys::HDR_MODE_ENABLED)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Windows：设置 WebView2 颜色配置文件（强制使用线性 sRGB）。
///
/// 未开启 HDR 模式时，把 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 设为
/// `--force-color-profile=scrgb-linear`，避免 WebView2 在 HDR 屏上做二次色彩映射。
/// 非 Windows 平台为空实现，以便 `run()` 调用点无需 `#[cfg]`。
#[cfg(target_os = "windows")]
pub fn apply_webview2_color_profile(identifier: &str) {
    if !read_hdr_mode_enabled(identifier) {
        // `std::env::set_var` 在 Rust 2024 起被标记为 unsafe；此处仍在 2021 edition，
        // 但为将来迁移保留 `#[allow(deprecated)]`。调用点在 WebView2 创建之前，
        // 尚无其它线程读取环境变量。
        #[allow(deprecated)]
        unsafe {
            std::env::set_var(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                "--force-color-profile=scrgb-linear",
            );
        }
    }
}

/// 非 Windows 平台无颜色配置文件设置需求。
#[cfg(not(target_os = "windows"))]
pub fn apply_webview2_color_profile(_identifier: &str) {}
