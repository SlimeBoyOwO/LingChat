fn main() {
    // tauri-build 在外壳 crate 的 build.rs 里会发射 desktop / mobile 这两个 cfg，
    // 业务代码里大量 `#[cfg(desktop)]` / `#[cfg(mobile)]` 依赖它们。本 crate 不调用
    // tauri_build::build()（那是外壳的事，且此处无 tauri.conf.json），故自行发射
    // 同一套 cfg，判定依据与 tauri-build 一致：android / ios 视为 mobile。
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" || target_os == "ios" {
        println!("cargo:rustc-cfg=mobile");
    } else {
        println!("cargo:rustc-cfg=desktop");
    }
    println!("cargo:rustc-check-cfg=cfg(desktop)");
    println!("cargo:rustc-check-cfg=cfg(mobile)");

    // cargo test 的测试 harness 不携带应用 manifest，默认绑定 System32 的
    // comctl32 v5.82；但依赖里导入了 TaskDialogIndirect（只有 comctl32 v6 才导出），
    // 导致测试 exe 启动即 STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)。
    // 这里给链接器声明 common-controls v6 依赖，使测试 exe 激活 WinSxS 的 v6。
    // 注意不能用 #[cfg(target_os)]——build.rs 以 host（Windows）编译，cfg 判断
    // 的是 host；Windows 宿主交叉编译 Android 时会把 MSVC 专属参数传给 NDK
    // clang。用 CARGO_CFG_TARGET_OS 判断真正的链接 target。
    if target_os == "windows" {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    emit_sherpa_provider();
}

/// 发射编译期常量 `SHERPA_ONNX_PROVIDER`（`dml` / `cpu`），供
/// `adapters/sherpa_onnx.rs` 判断「链接进来的这套 sherpa-onnx 到底会不会用 DirectML」。
///
/// 判定依据是 `SHERPA_ONNX_LIB_DIR` 里那份 `sherpa-build.json` —— 由
/// scripts/build-desktop-sherpa-dml.mjs 在自编 DirectML 版时写出。以**实际链接的库**
/// 为准：目录里带标记才算 DirectML 版；没有标记（上游 build.rs 自己下官方 CPU 包，或者
/// 没走 `pnpm tauri:build:win` 而直接 cargo build）一律按 CPU 处理 —— 官方包没编
/// DirectML，把 provider 交给 sherpa 只会静默回落，等于给用户一个假的开关。
fn emit_sherpa_provider() {
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_GPU");

    let lib_dir = std::env::var_os("SHERPA_ONNX_LIB_DIR").map(std::path::PathBuf::from);
    let provider = lib_dir
        .as_ref()
        .map(|dir| dir.join("sherpa-build.json"))
        .filter(|marker| marker.is_file())
        .and_then(|marker| std::fs::read_to_string(marker).ok())
        .and_then(|text| parse_sherpa_build_provider(&text));

    let provider = match provider.as_deref() {
        Some("dml") => "dml",
        Some(other) => {
            println!(
                "cargo:warning=sherpa-build.json 里的 provider={other} 无法识别，本次按 CPU 处理"
            );
            "cpu"
        },
        None => {
            if std::env::var("SHERPA_ONNX_GPU").is_ok_and(|v| v.trim().eq_ignore_ascii_case("dml"))
            {
                println!(
                    "cargo:warning=SHERPA_ONNX_GPU=dml，但 SHERPA_ONNX_LIB_DIR 里没有 \
                     sherpa-build.json（不像 DirectML 版产物），本次按 CPU 编译。\
                     请用 `pnpm tauri:build:win`。"
                );
            }
            "cpu"
        },
    };

    println!("cargo:rustc-env=SHERPA_ONNX_PROVIDER={provider}");
}

/// 从 sherpa-build.json 里抠出 `provider` 的值。
/// 只有一个键要读，不值得为它给 build.rs 引 JSON 依赖。
fn parse_sherpa_build_provider(text: &str) -> Option<String> {
    const KEY: &str = "\"provider\"";
    let rest = &text[text.find(KEY)? + KEY.len()..];
    let rest = rest[rest.find(':')? + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
