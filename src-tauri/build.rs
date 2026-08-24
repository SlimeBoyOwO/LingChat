fn main() {
    tauri_build::build();

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
