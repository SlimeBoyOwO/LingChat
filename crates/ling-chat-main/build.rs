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
}
