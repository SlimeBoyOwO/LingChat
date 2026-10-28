fn main() {
    specify_linux_runtime_lib_path();
    tauri_build::build();
}

/// Linux 上 ort 的 webgpu 后端把 Dawn 以动态库形式提供（`libwebgpu_dawn.so`，
/// 无法静态链接），但链接产物不带 RUNPATH，运行期只能靠系统默认搜索路径，
/// 于是「exe 旁边有库」和「安装在资源目录里」两种形态都加载失败。
///
/// 这里给可执行文件写入三条搜索路径：
/// - `$ORIGIN`：开发态与便携态，库与 exe 同目录（ort 的 copy-dylibs 会放在这里）
/// - `$ORIGIN/../lib`：AppImage 的 usr/lib（linuxdeploy 收集的依赖库在此）
/// - `$ORIGIN/../lib/LingChat/lib`：deb 安装态，库随资源落到 /usr/lib/LingChat/lib
///
/// 只作用于 bin target，不影响依赖 rlib；目录不存在时动态加载器会静默跳过。
fn specify_linux_runtime_lib_path() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    for rpath in ["$ORIGIN", "$ORIGIN/../lib", "$ORIGIN/../lib/LingChat/lib"] {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{rpath}");
    }
}
