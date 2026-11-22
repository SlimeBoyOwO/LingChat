//! 平台适配。
//!
//! - Windows：WebView2 颜色配置文件。它必须在 WebView2 环境创建
//!   （`Builder::build()`）**之前**设置，因此无法走 Tauri 的设置体系，只能在此处
//!   直接读 store 文件并写进程环境变量。
//! - Android：panic 与错误链写入 logcat。移动端 stderr 通向 /dev/null，panic 又被
//!   `stop_unwind` 捕获后直接 abort，不显式转投 logcat 就只剩「打开即闪退」。
//!
//! 本模块原先位于 crate 根（`lib.rs`）。

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
        .join(ling_chat_main::config::STORE_FILE);

    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    json.get(ling_chat_main::config::keys::HDR_MODE_ENABLED)
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

/// 经 liblog 写一行到 logcat（tag = `LingChatRust`）。
#[cfg(target_os = "android")]
fn android_log_write(msg: &str) {
    use std::ffi::CString;
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn __android_log_write(prio: i32, tag: *const c_char, text: *const c_char) -> i32;
    }

    // ANDROID_LOG_ERROR
    const PRIO_ERROR: i32 = 6;
    // logcat 中用于过滤本桥输出的 tag。
    const TAG: &[u8] = b"LingChatRust\0";

    if let Ok(text) = CString::new(msg) {
        // SAFETY: 两个指针都指向 NUL 结尾的合法 C 字符串，liblog 只读取它们。
        unsafe {
            __android_log_write(PRIO_ERROR, TAG.as_ptr().cast(), text.as_ptr());
        }
    }
}

/// 启动早期诊断通道：Android 写 logcat，其它平台写 stderr。
///
/// tracing 订阅器装配之前的失败没有任何可见输出（Android 上 stdout/stderr 不可见），
/// 启动编排各阶段用它留痕与打印错误链。
pub fn early_log(msg: &str) {
    #[cfg(target_os = "android")]
    android_log_write(msg);

    #[cfg(not(target_os = "android"))]
    eprintln!("[early] {msg}");
}

/// 把错误及其完整 `source()` 链写入诊断通道。
///
/// anyhow 的 `Display` 只给最外层，而 Tauri 的 setup 失败最终只 panic 出最外层消息，
/// 底层原因（SQL、IO 等）不会出现在任何地方。
pub fn log_error_chain(step: &str, error: &(dyn std::error::Error + 'static)) {
    early_log(&format!("{step} 失败: {error}"));
    let mut source = error.source();
    while let Some(cause) = source {
        early_log(&format!("  caused by: {cause}"));
        source = cause.source();
    }
}

/// Android：把 Rust panic 信息写入 logcat。
///
/// 移动端 `run()` 被 `tauri::mobile_entry_point` 的 `stop_unwind` 包裹：panic 会被
/// 捕获后立刻 abort，panic 详情只写 stderr，而 Android 上它通向 /dev/null。因此必须
/// 在 `run()` 最前面挂上这个桥，否则任何启动 panic 都只表现为「打开即闪退」。
#[cfg(target_os = "android")]
pub fn install_panic_logger() {
    use std::sync::Once;

    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // 先发固定标记：即使消息因任何原因写不出去，也能确认 hook 触发过。
            android_log_write("PANIC 触发");
            match info.location() {
                Some(loc) => android_log_write(&format!(
                    "PANIC: {}:{}:{}",
                    loc.file(),
                    loc.line(),
                    loc.column()
                )),
                None => android_log_write("PANIC: 位置未知"),
            }
            // liblog 走 C 字符串，消息里的 NUL 会让 CString 构造失败而整条丢失。
            android_log_write(&format!("PANIC 消息: {}", info).replace('\0', "\\0"));
            // 保留 std 原有行为（stderr 打印），便于连接调试器时对照。
            previous(info);
        }));
    });
}

/// 非 Android 平台 stderr 可见，无需 logcat 桥。
#[cfg(not(target_os = "android"))]
pub fn install_panic_logger() {}
