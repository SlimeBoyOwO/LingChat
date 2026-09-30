//! ONNX Runtime 运行时初始化。
//!
//! `ort` 默认静态链接 pyke 预编译的 x86-64-v3 二进制（要求 AVX2/FMA），旧 CPU 上启动即
//! 非法指令崩溃。**仅 Windows** 改用 `load-dynamic`（见 Cargo.toml），运行时从这里加载
//! 应用目录下的 onnxruntime.dll（由 scripts/download-onnxruntime.mjs 取回，含 DirectML EP）。
//!
//! `ort::init_from` 只在 `load-dynamic` 下存在，故本模块限定 `#[cfg(target_os = "windows")]`。
//! 找不到 dll 时返回 false，上层据此降级。

#[cfg(target_os = "windows")]
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "windows")]
use tauri::{AppHandle, Manager};

/// ONNX Runtime 是否可用。
/// Windows（load-dynamic）：取决于 onnxruntime.dll 是否加载成功；
/// 非 Windows（download-binaries 静态链接）：恒可用。
///
/// ⚠️ 必须在任何 `ort::Session` 创建**之前**检查：若 dll 缺失而直接创建
/// Session，ort 的 `setup_api()` 会因加载失败而 panic（`.expect`），
/// 导致整个应用崩溃。各 onnx 使用点（情绪分类/VAD/本地 TTS）应据此降级。
pub fn onnx_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        ONNX_AVAILABLE.load(Ordering::Relaxed)
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

#[cfg(target_os = "windows")]
static ONNX_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// 定位 onnxruntime.dll 并用 `ort::init_from` 显式加载。
///
/// 探测顺序：
/// 1. exe 同目录（开发/便携：`target/debug/onnxruntime.dll` 或 exe 旁）
/// 2. tauri 资源目录（打包：`resources/onnxruntime.dll`）
///
/// 返回 true 表示 dll 已就绪；false 表示未找到或加载失败（调用方应降级 onnx 功能）。
///
/// 仅 Windows（`load-dynamic` 限定 Windows target；`ort::init_from` 依赖该 feature）。
#[cfg(target_os = "windows")]
pub fn init_onnx_runtime(app: &AppHandle) -> bool {
    let candidates: Vec<PathBuf> = {
        let mut v = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                v.push(dir.join("onnxruntime.dll"));
            }
        }
        if let Ok(res) = app.path().resource_dir() {
            v.push(res.join("onnxruntime.dll"));
        }
        v
    };

    // 找第一个存在的候选（exe 同目录 → resource_dir）。
    // ⚠️ `ort::init_from` 只能调用一次：加载失败后 ort 内部 `load_dynamic::init`
    //    的 `G_ORT_LIB` inserter 已被消费，再次调用会走 `unwrap_unchecked` UB，
    //    实测导致 0xC0000409 崩溃（损坏/空 dll 文件场景）。因此无论成败，
    //    都只尝试加载第一个存在的文件，绝不循环重试。
    let Some(path) = candidates.iter().find(|p| p.exists()) else {
        tracing::warn!(
            "[ONNX] 未找到 onnxruntime.dll（已尝试: {:?}）。ONNX 相关功能将降级。",
            candidates
        );
        return false;
    };

    match ort::init_from(path.as_path()) {
        Ok(_) => {
            tracing::info!("[ONNX] onnxruntime.dll 就绪: {}", path.display());
            ONNX_AVAILABLE.store(true, Ordering::Relaxed);
            // CPU 兼容提示：官方 dll 为 SSE3 基线，但若检测不到 AVX2 仅提示（不影响运行）
            #[cfg(target_arch = "x86_64")]
            if !std::arch::is_x86_feature_detected!("avx2") {
                tracing::info!(
                    "[ONNX] 当前 CPU 不支持 AVX2，使用官方 onnxruntime（SSE3 基线）可正常推理"
                );
            }
            true
        },
        Err(e) => {
            tracing::warn!(
                "[ONNX] 加载 {} 失败: {e}。ONNX 相关功能将降级。",
                path.display()
            );
            false
        },
    }
}

// 钦灵：这个我就保留了，防止之后开发者哭哭
#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    /// 运行时验证 onnxruntime.dll：ort API 版本匹配、含 DirectML EP、能用 DML 建 session。
    /// 需先跑 `node scripts/download-onnxruntime.mjs`。
    #[test]
    #[ignore = "需要已下载的 onnxruntime.dll（scripts/download-onnxruntime.mjs）"]
    fn load_official_onnxruntime() {
        let dll = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("onnxruntime.dll");
        assert!(dll.exists(), "onnxruntime.dll 不存在: {}", dll.display());

        let _env = ort::init_from(&dll).expect("ort::init_from 加载失败");
        // 能建 SessionBuilder = ort api 版本匹配
        let _builder = ort::session::Session::builder().expect("SessionBuilder 创建失败");

        // 走 EP bridge 的 GetExecutionProviderApi("DML")：None = 该 dll 不含 DML EP
        assert!(
            ort::ep::directml::api().is_some(),
            "当前 onnxruntime.dll 不含 DirectML EP，选 GPU 会静默回落 CPU"
        );

        // 需要 DX12 GPU；error_on_failure 让「静默回落 CPU」变成硬错误
        let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("data")
            .join("third_party")
            .join("asr_vad")
            .join("silero-vad.onnx");
        assert!(model.exists(), "测试模型不存在: {}", model.display());
        let _gpu_session = ort::session::Session::builder()
            .expect("SessionBuilder 创建失败")
            .with_execution_providers([ort::ep::DirectML::default().build().error_on_failure()])
            .expect("注册 DirectML EP 失败")
            .commit_from_file(&model)
            .expect("用 DirectML 建 session 失败（本机无 DX12 GPU？）");
    }
}
