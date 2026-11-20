//! ONNX Runtime 运行时初始化。

/// ONNX Runtime 是否可用：尝试构建进程级环境，成功即可用。
///
/// 各平台统一走 ort 的 download-binaries 预编译包，环境由
/// [`ling_chat_notes::ort_env::environment`] 惰性构建并缓存。
pub fn onnx_available() -> bool {
    ling_chat_notes::ort_env::environment().is_ok()
}

// 钦灵：这个我就保留了，防止之后开发者哭哭
#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    /// 运行时验证 pyke onnxruntime：ort API 版本匹配、含 DirectML EP、能用 DML 建 session。
    /// 需先构建一次（copy-dylibs 会把 dll 摆到 target 供运行加载）。
    #[test]
    #[ignore = "需要 copy-dylibs 已放置的 onnxruntime.dll（先构建一次）"]
    fn load_pyke_onnxruntime() {
        let environment =
            ling_chat_notes::ort_env::environment().expect("构建 ONNX Runtime 环境失败");
        // 能建 SessionBuilder = ort api 版本匹配
        let _builder =
            ort::session::Session::builder(environment).expect("SessionBuilder 创建失败");

        // 走 EP bridge 的 GetExecutionProviderApi("DML")：None = 该 dll 不含 DML EP
        assert!(
            ort::ep::directml::api().is_some(),
            "当前 onnxruntime.dll 不含 DirectML EP，选 GPU 会静默回落 CPU"
        );

        // 需要 DX12 GPU；error_on_failure 让「静默回落 CPU」变成硬错误
        let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("data")
            .join("third_party")
            .join("asr_vad")
            .join("silero-vad.onnx");
        assert!(model.exists(), "测试模型不存在: {}", model.display());
        let _gpu_session = ort::session::Session::builder(environment)
            .expect("SessionBuilder 创建失败")
            .with_execution_providers([ort::ep::DirectML::default().build().error_on_failure()])
            .expect("注册 DirectML EP 失败")
            .commit_from_file(&model)
            .expect("用 DirectML 建 session 失败（本机无 DX12 GPU？）");
    }
}
