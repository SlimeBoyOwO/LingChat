fn main() {
    // Android 上 libffi 静态库（rustpython 的 ctypes 依赖）从仓库内 jniLibs 目录
    // 链接进最终的 app 库。用 build.rs 指令而不是 .cargo/config.toml 的 rustflags：
    // rustflags 全局作用于所有依赖 crate 的链接，registry 依赖 crate 的相对路径
    // 解析不到。相对路径按链接器 cwd（本 crate 根目录 crates/ling-chat-plugins）解析，
    // 故上跳两级回仓库根再进 src-tauri。
    // 注意 build.rs 里的 cfg 是宿主平台，目标平台要用 CARGO_CFG_TARGET_OS。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!(
            "cargo:rustc-link-arg=../../src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/libffi.a"
        );
    }
}
