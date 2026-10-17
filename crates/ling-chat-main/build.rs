fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" || target_os == "ios" {
        println!("cargo:rustc-cfg=mobile");
    } else {
        println!("cargo:rustc-cfg=desktop");
    }
    println!("cargo:rustc-check-cfg=cfg(desktop)");
    println!("cargo:rustc-check-cfg=cfg(mobile)");

    if target_os == "windows" {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    emit_sherpa_gpu_variant();
}

/// 发射编译期常量 `SHERPA_ONNX_GPU_VARIANT`，供 `adapters/sherpa_onnx.rs` 判断
/// 「链接进来的 sherpa-onnx 是不是官方 CUDA 包」。
///
/// 以前这个常量由 vendored 的 sherpa-onnx-sys build.rs 发射，那份副本删掉之后
/// 改由本 crate 负责：`SHERPA_ONNX_GPU` 决定要哪个变体，`SHERPA_ONNX_LIB_DIR`
/// （由 scripts/fetch-sherpa-onnx-libs.mjs 导出）决定实际链接哪套库 —— 两者不一致
/// 时以**实际链接的库**为准，宁可退回 CPU 也不要让运行期去点不存在的 CUDA provider。
///
/// 别名表与 scripts/stage-desktop-runtime-libs.mjs 的 `resolveVariant`、
/// scripts/fetch-sherpa-onnx-libs.mjs 保持一致；改一处要改三处。
fn emit_sherpa_gpu_variant() {
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_GPU");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");

    let raw = std::env::var("SHERPA_ONNX_GPU").unwrap_or_default();
    let requested = match raw.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "off" | "false" | "cpu" | "none" => "cpu",
        "12" | "cuda" | "cuda12" | "cuda-12" => "cuda12",
        "13" | "cuda13" | "cuda-13" => "cuda13",
        other => panic!("SHERPA_ONNX_GPU 取值无法识别: {other}（可用: cpu / cuda12 / cuda13）"),
    };

    let lib_dir = std::env::var_os("SHERPA_ONNX_LIB_DIR").map(std::path::PathBuf::from);
    let variant = match (&lib_dir, requested) {
        // 没给库目录：上游 build.rs 会自己下官方 CPU 包，这次编出来就是 CPU 版。
        (None, "cpu") => "cpu",
        (None, want) => {
            println!(
                "cargo:warning=SHERPA_ONNX_GPU={raw}，但没设 SHERPA_ONNX_LIB_DIR：本次按 CPU 包编译。\
                 先跑 `node scripts/fetch-sherpa-onnx-libs.mjs --variant={want} --print-env` 并 eval 它，\
                 或直接用 `pnpm tauri:build:gpu`（会串好这两步）。"
            );
            "cpu"
        },
        // 给了库目录却没看到 CUDA provider 库：目录与变体不匹配，按 CPU 处理（有日志）。
        (Some(dir), want) if want != "cpu" && !has_cuda_provider(dir) => {
            println!(
                "cargo:warning=SHERPA_ONNX_LIB_DIR={} 里没有 CUDA 执行提供方库，\
                 与 SHERPA_ONNX_GPU={raw} 不符：本次按 CPU 包编译。",
                dir.display()
            );
            "cpu"
        },
        (Some(_), want) => want,
    };

    if variant != "cpu" && std::env::var_os("CARGO_FEATURE_SHERPA_SHARED").is_none() {
        println!(
            "cargo:warning=GPU 变体需要 `--features sherpa-shared`：官方 CUDA 包只有动态库，\
             静态链接会链不上（上游 build.rs 也拒绝 static 与 shared 同时开）。"
        );
    }

    println!("cargo:rustc-env=SHERPA_ONNX_GPU_VARIANT={variant}");
}

/// 目录里有没有 CUDA 执行提供方（`libonnxruntime_providers_cuda.so` / `.dll`）。
fn has_cuda_provider(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with("libonnxruntime_providers_cuda")
            || entry
                .file_name()
                .to_string_lossy()
                .starts_with("onnxruntime_providers_cuda")
    })
}
