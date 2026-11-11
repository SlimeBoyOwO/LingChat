//! Sherpa-ONNX 模型管理器：扫描、解析与查询本地模型。

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct SherpaOnnxModelInfo {
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_type: String,
    pub language: String,
    pub voice: String,
    pub size_bytes: u64,
    pub is_valid: bool,
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

#[derive(Debug)]
pub struct SherpaOnnxManager {
    models: Arc<RwLock<HashMap<String, SherpaOnnxModelInfo>>>,
    models_root: PathBuf,
}

impl SherpaOnnxManager {
    /// 创建模型管理器。
    pub fn new(models_root: PathBuf) -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            models_root,
        }
    }

    pub fn model_dir(&self) -> PathBuf {
        self.models_root.clone()
    }

    pub async fn initialize(&self) -> Result<()> {
        tokio::fs::create_dir_all(self.model_dir()).await?;
        self.scan_existing_models().await?;

        Ok(())
    }

    async fn scan_existing_models(&self) -> Result<()> {
        let model_dir = self.model_dir();
        if !model_dir.exists() {
            return Ok(());
        }

        let mut models = self.models.write().await;
        // 每次从磁盘重建，避免外部删除后残留旧记录。
        models.clear();
        let mut entries = tokio::fs::read_dir(&model_dir).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(model_info) = self.parse_model_directory(&path).await {
                    models.insert(model_info.model_name.clone(), model_info);
                }
            }
        }

        Ok(())
    }

    async fn parse_model_directory(&self, model_dir: &Path) -> Result<SherpaOnnxModelInfo> {
        let model_name = model_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("Invalid model directory name"))?
            .to_string();

        // 覆盖所有支持的模型类型。
        let model_files = [
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
        let mut model_path = None;

        for file in &model_files {
            let candidate = model_dir.join(file);
            if candidate.exists() {
                model_path = Some(candidate);
                break;
            }
        }

        let model_path = model_path.ok_or_else(|| anyhow!("No model file found in directory"))?;

        // 获取模型文件大小
        let metadata = tokio::fs::metadata(&model_path).await?;
        let size_bytes = metadata.len();

        // 类型以模型目录本身为准，避免角色设置里的错误类型。
        let config_path = model_dir.join("config.json");
        let (config_type, language, voice) = if config_path.exists() {
            self.load_model_config(&config_path).await?
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
            is_valid: true, // TODO: 实现模型验证
        })
    }

    async fn load_model_config(&self, config_path: &Path) -> Result<(String, String, String)> {
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

    pub async fn get_models(&self) -> Vec<SherpaOnnxModelInfo> {
        let models = self.models.read().await;
        models.values().cloned().collect()
    }

    pub async fn get_model(&self, model_name: &str) -> Option<SherpaOnnxModelInfo> {
        let models = self.models.read().await;
        models.get(model_name).cloned()
    }

    pub async fn model_exists(&self, model_name: &str) -> bool {
        let models = self.models.read().await;
        models.contains_key(model_name)
    }

    pub async fn add_model(&self, model_info: SherpaOnnxModelInfo) -> Result<()> {
        let mut models = self.models.write().await;
        models.insert(model_info.model_name.clone(), model_info);
        Ok(())
    }

    pub async fn remove_model(&self, model_name: &str) -> Result<()> {
        let mut models = self.models.write().await;

        if let Some(model_info) = models.remove(model_name) {
            if let Some(parent) = model_info.model_path.parent() {
                if parent.exists() {
                    tokio::fs::remove_dir_all(parent).await?;
                }
            }
        }

        Ok(())
    }

    async fn with_model<T>(
        &self,
        model_name: &str,
        f: impl FnOnce(&SherpaOnnxModelInfo) -> T,
    ) -> Option<T> {
        self.models.read().await.get(model_name).map(f)
    }

    pub async fn get_model_path(&self, model_name: &str) -> Option<PathBuf> {
        self.with_model(model_name, |info| info.model_path.clone())
            .await
    }

    pub async fn get_model_type(&self, model_name: &str) -> Option<String> {
        self.with_model(model_name, |info| info.model_type.clone())
            .await
    }

    pub async fn get_model_language(&self, model_name: &str) -> Option<String> {
        self.with_model(model_name, |info| info.language.clone())
            .await
    }

    pub async fn get_model_voice(&self, model_name: &str) -> Option<String> {
        self.with_model(model_name, |info| info.voice.clone()).await
    }

    pub async fn get_model_size(&self, model_name: &str) -> Option<u64> {
        self.with_model(model_name, |info| info.size_bytes).await
    }

    pub async fn validate_model(&self, model_name: &str) -> Result<bool> {
        let models = self.models.read().await;
        let info = models
            .get(model_name)
            .ok_or_else(|| anyhow!("Model not found: {}", model_name))?;
        // TODO: 实现更详细的模型验证（文件大小、模型格式等）。
        Ok(info.model_path.exists())
    }

    pub async fn get_storage_stats(&self) -> Result<(u64, usize)> {
        let models = self.models.read().await;
        let total_size = models.values().map(|info| info.size_bytes).sum();
        let model_count = models.len();

        Ok((total_size, model_count))
    }

    pub async fn cleanup_invalid_models(&self) -> Result<usize> {
        let mut models = self.models.write().await;
        let before = models.len();
        models.retain(|_, info| info.is_valid);
        Ok(before - models.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_model_manager() {
        let temp_dir = tempdir().unwrap();
        let manager = SherpaOnnxManager::new(temp_dir.path().to_path_buf());

        assert!(manager.initialize().await.is_ok());
        assert_eq!(manager.model_dir(), temp_dir.path().to_path_buf());

        let models = manager.get_models().await;
        assert!(models.is_empty());

        let (size, count) = manager.get_storage_stats().await.unwrap();
        assert_eq!(size, 0);
        assert_eq!(count, 0);
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
