//! Sherpa-ONNX 模型目录扫描与类型解析。

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct SherpaOnnxModelInfo {
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_type: String,
    pub language: String,
    pub voice: String,
    pub size_bytes: u64,
}

/// 模型根目录（`<name>/` 直接位于其下）。
/// 所有平台统一用应用数据目录下的 `sherpa_onnx_models`，无需存储权限。
pub fn sherpa_onnx_models_root() -> PathBuf {
    crate::data_dir::get_data_dir().join("sherpa_onnx_models")
}

/// 依据模型目录本身解析实际 `model_type`：内置清单 → `config.json` → 文件特征 → 传入值。
pub fn resolve_model_type(model_dir: &Path, configured: &str) -> String {
    if let Some(mtype) = model_dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(crate::api::tts_sherpa::catalog_model_type)
    {
        return mtype.to_string();
    }

    if let Ok(content) = std::fs::read_to_string(model_dir.join("config.json")) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(mtype) = config
                .get("model_type")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return mtype.to_string();
            }
        }
    }

    if let Some(mtype) = infer_model_type(model_dir) {
        return mtype.to_string();
    }

    let configured = configured.trim();
    if configured.is_empty() {
        "vits".to_string()
    } else {
        configured.to_string()
    }
}

/// 按目录内文件特征推断 `model_type`；kokoro/kitten 无法区分时按 kokoro。
fn infer_model_type(model_dir: &Path) -> Option<&'static str> {
    let has = |file: &str| model_dir.join(file).exists();

    if has("fm_decoder.onnx") || has("fm_decoder_int8.onnx") {
        return Some("zipvoice");
    }
    if has("lm_main.int8.onnx") || has("lm_main.onnx") || has("lm_flow.onnx") {
        return Some("pocket");
    }
    if has("duration_predictor.onnx") || has("vector_estimator.onnx") {
        return Some("supertonic");
    }
    if has("model-steps-3.onnx") || has("model-steps-6.onnx") {
        return Some("matcha");
    }
    if has("text_encoder.onnx") {
        return Some("zipvoice");
    }
    if has("voices.bin") && (has("model.int8.onnx") || has("model.onnx")) {
        return Some("kokoro");
    }
    if has("model.onnx") || has("tts-model.onnx") || has("sherpa-onnx-tts.onnx") {
        return Some("vits");
    }
    None
}

/// 扫描模型根目录下的所有可识别模型。
pub async fn scan_models(root: &Path) -> Result<Vec<SherpaOnnxModelInfo>> {
    tokio::fs::create_dir_all(root).await?;
    let mut models = Vec::new();
    let mut entries = tokio::fs::read_dir(root).await?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            if let Ok(model_info) = parse_model_directory(&path).await {
                models.push(model_info);
            }
        }
    }

    Ok(models)
}

async fn parse_model_directory(model_dir: &Path) -> Result<SherpaOnnxModelInfo> {
    let model_name = model_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Invalid model directory name"))?
        .to_string();

    // 覆盖所有支持的模型类型。
    const MODEL_FILES: &[&str] = &[
        // VITS / FastSpeech2
        "model.onnx",
        "tts-model.onnx",
        "sherpa-onnx-tts.onnx",
        // Matcha
        "model-steps-3.onnx",
        "model-steps-6.onnx",
        // ZipVoice
        "fm_decoder.onnx",
        // Kokoro / Kitten
        "model.int8.onnx",
        // Pocket
        "lm_main.int8.onnx",
        "lm_main.onnx",
        "lm_flow.int8.onnx",
        "lm_flow.onnx",
        // Supertonic
        "text_encoder.onnx",
        "duration_predictor.onnx",
    ];
    let model_path = MODEL_FILES
        .iter()
        .map(|file| model_dir.join(file))
        .find(|candidate| candidate.exists())
        .ok_or_else(|| anyhow!("No model file found in directory"))?;

    let size_bytes = tokio::fs::metadata(&model_path).await?.len();

    // 类型以模型目录本身为准，避免角色设置里的错误类型。
    let config_path = model_dir.join("config.json");
    let (config_type, language, voice) = if config_path.exists() {
        load_model_config(&config_path).await?
    } else {
        (String::new(), "zh".to_string(), "female".to_string())
    };
    let model_type = resolve_model_type(model_dir, &config_type);

    Ok(SherpaOnnxModelInfo {
        model_name,
        model_path,
        model_type,
        language,
        voice,
        size_bytes,
    })
}

async fn load_model_config(config_path: &Path) -> Result<(String, String, String)> {
    let content = tokio::fs::read_to_string(config_path).await?;
    let config: serde_json::Value = serde_json::from_str(&content)?;
    let field = |key: &str, default: &str| {
        config
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or(default)
            .to_string()
    };
    Ok((
        field("model_type", "vits"),
        field("language", "zh"),
        field("voice", "female"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn scans_installed_models() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path().join("models");
        let model_dir = root.join("my-vits");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"1234").unwrap();

        let models = scan_models(&root).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_name, "my-vits");
        assert_eq!(models[0].model_type, "vits");
        assert_eq!(models[0].size_bytes, 4);
    }

    #[test]
    fn resolve_model_type_prefers_catalog_dir() {
        // 内置目录名即权威类型，忽略传入的错误默认值。
        let dir = Path::new("/nonexistent/sherpa_onnx_models/kokoro-int8-multi-lang-v1_1");
        assert_eq!(resolve_model_type(dir, "vits"), "kokoro");

        let dir = Path::new("/nonexistent/sherpa_onnx_models/matcha-icefall-zh-baker");
        assert_eq!(resolve_model_type(dir, "vits"), "matcha");
    }

    #[test]
    fn resolve_model_type_infers_from_files() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        // Kokoro/Kitten：voices.bin + model(.int8).onnx
        let kokoro = root.join("my-kokoro");
        std::fs::create_dir_all(&kokoro).unwrap();
        std::fs::write(kokoro.join("model.int8.onnx"), b"x").unwrap();
        std::fs::write(kokoro.join("voices.bin"), b"x").unwrap();
        assert_eq!(resolve_model_type(&kokoro, "vits"), "kokoro");

        // Matcha：model-steps-*.onnx
        let matcha = root.join("my-matcha");
        std::fs::create_dir_all(&matcha).unwrap();
        std::fs::write(matcha.join("model-steps-3.onnx"), b"x").unwrap();
        assert_eq!(resolve_model_type(&matcha, "vits"), "matcha");

        // config.json 声明优先于文件推断。
        let declared = root.join("my-declared");
        std::fs::create_dir_all(&declared).unwrap();
        std::fs::write(declared.join("model.onnx"), b"x").unwrap();
        std::fs::write(declared.join("config.json"), br#"{"model_type":"kitten"}"#).unwrap();
        assert_eq!(resolve_model_type(&declared, "vits"), "kitten");

        // 无法识别时回退到配置值；为空取 vits。
        let plain = root.join("my-plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(resolve_model_type(&plain, "fastspeech2"), "fastspeech2");
        assert_eq!(resolve_model_type(&plain, ""), "vits");
    }
}
