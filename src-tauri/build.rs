fn main() {
    // Windows：python310.lib 导入库搜索路径（由 pylibs/ 提供）。
    // 注意：不做 /DELAYLOAD——python 的 PyExc_* 等数据符号依法不可延迟加载（LNK1194）。
    // python310.dll 随包放在 exe 旁边，进程启动时即由加载器解析；
    // 解释器家目录由 engine_embed 在 Py_Initialize 前经 PYTHONHOME 指向引擎 runtime。
    #[cfg(windows)]
    {
        println!(
            "cargo:rustc-link-search=native={}\\pylibs",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );
    }
    tauri_build::build()
}
