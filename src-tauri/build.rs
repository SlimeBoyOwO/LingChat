fn main() {
    tauri_build::build();

    // Android 上 libffi 静态库（rustpython 的 ctypes 依赖）从仓库内 jniLibs 目录
    // 链接进最终的 app 库。用 build.rs 指令而不是 .cargo/config.toml 的 rustflags：
    // rustflags 全局作用于所有依赖 crate 的链接，registry 依赖 crate 的相对路径
    // 解析不到。相对路径按链接器 cwd（本 crate 根目录 src-tauri）解析。
    // 注意 build.rs 里的 cfg 是宿主平台，目标平台要用 CARGO_CFG_TARGET_OS。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg=gen/android/app/src/main/jniLibs/arm64-v8a/libffi.a");
    }

    // cargo test 的测试 harness 不携带应用 manifest，默认绑定 System32 的
    // comctl32 v5.82；但依赖里导入了 TaskDialogIndirect（只有 comctl32 v6 才导出），
    // 导致测试 exe 启动即 STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)。
    // 这里给链接器声明 common-controls v6 依赖，使测试 exe 激活 WinSxS 的 v6。
    // 对应用二进制无害：它自带的同名 manifest 会被合并去重。
    #[cfg(target_os = "windows")]
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}
