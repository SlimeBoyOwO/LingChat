//! 记忆嵌入配置（`AppConfig.embedding`）。
//!
//! 嵌入在 Rust 侧直接以 ONNX Runtime 推理（见
//! `ai_service/embedding/service.rs`），与 Python 解耦：
//! - `model_dir`：一个小型中文嵌入模型目录（sentence-transformers 或 ONNX 格式）。
//!   空/不存在时嵌入功能自动禁用，不影响主流程。
//! - `python`：旧版遗留字段，仅保留解析，不再参与服务（向后兼容配置文件）。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::keys;

fn default_python() -> String {
    "".into()
}

fn default_model_dir() -> String {
    "".into()
}

fn default_backend() -> String {
    "auto".into()
}

fn default_enabled() -> bool {
    false
}

fn default_query_prefix() -> String {
    String::new()
}

fn default_passage_prefix() -> String {
    String::new()
}

/// 记忆嵌入配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// 是否启用记忆嵌入（需配合 model_dir 就绪）。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Python 解释器绝对路径（遗留字段，已不使用）。
    #[serde(default = "default_python")]
    pub python: String,
    /// 嵌入模型目录。
    #[serde(default = "default_model_dir")]
    pub model_dir: String,
    /// 后端：auto / onnx / st（Rust 侧仅 onnx）。
    #[serde(default = "default_backend")]
    pub backend: String,
    /// 查询文本前缀（E5 系/INSTRUCTOR 系模型建议；本模型无需，默认为空）。
    #[serde(default = "default_query_prefix")]
    pub query_prefix: String,
    /// 语料/记忆片段文本前缀（同上，默认为空）。
    #[serde(default = "default_passage_prefix")]
    pub passage_prefix: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            python: default_python(),
            model_dir: default_model_dir(),
            backend: default_backend(),
            query_prefix: default_query_prefix(),
            passage_prefix: default_passage_prefix(),
        }
    }
}

impl EmbeddingConfig {
    /// 从已打开的 store 读取。缺失项使用默认值。
    pub fn from_store(store: Option<&tauri_plugin_store::Store<tauri::Wry>>) -> Self {
        let get_string = |key: &str, default: &str| -> String {
            store
                .and_then(|s| s.get(key))
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| default.to_string())
        };
        let get_bool = |key: &str, default: bool| -> bool {
            store
                .and_then(|s| s.get(key))
                .and_then(|v| v.as_bool())
                .unwrap_or(default)
        };

        Self {
            enabled: get_bool(keys::EMBEDDING_ENABLED, default_enabled()),
            python: get_string(keys::EMBEDDING_PYTHON, &default_python()),
            model_dir: get_string(keys::EMBEDDING_MODEL_DIR, &default_model_dir()),
            backend: get_string(keys::EMBEDDING_BACKEND, &default_backend()),
            query_prefix: get_string(keys::EMBEDDING_QUERY_PREFIX, &default_query_prefix()),
            passage_prefix: get_string(keys::EMBEDDING_PASSAGE_PREFIX, &default_passage_prefix()),
        }
    }

    /// 转成供 [`crate::ai_service::embedding::service::EmbeddingManager`] 使用的配置。
    ///
    /// - `data_dir` 用于解析相对 model_dir（默认基准为 `data/third_party/embedding/`）。
    /// - `resource_dir`（打包后的 app 资源目录）：`model_dir` 未配置时，优先回退
    ///   随包分发的 `data/third_party/embedding_paraphrase_multilingual_minilm_l12_v2/`
    ///   （打包内置模型），否则用数据目录默认路径。
    pub fn to_service_config(
        &self,
        data_dir: &PathBuf,
        resource_dir: Option<&std::path::Path>,
    ) -> crate::ai_service::embedding::EmbeddingConfig {
        let model_dir = if self.model_dir.trim().is_empty() {
            default_model_path(data_dir, resource_dir)
        } else {
            let p = PathBuf::from(self.model_dir.trim());
            if p.is_absolute() { p } else { data_dir.join(p) }
        };
        crate::ai_service::embedding::EmbeddingConfig {
            model_dir,
            backend: self.backend.clone(),
            query_prefix: self.query_prefix.clone(),
            passage_prefix: self.passage_prefix.clone(),
        }
    }
}

/// 默认模型目录：优先打包资源内的内置模型，否则数据目录默认路径。
fn default_model_path(data_dir: &PathBuf, resource_dir: Option<&std::path::Path>) -> PathBuf {
    if let Some(r) = resource_dir {
        let bundled = r
            .join("data")
            .join("third_party")
            .join("embedding_paraphrase_multilingual_minilm_l12_v2");
        if ["model.onnx", "model_quantized.onnx", "model_int8.onnx"]
            .iter()
            .any(|name| bundled.join(name).exists())
        {
            return bundled;
        }
    }
    data_dir.join("third_party").join("embedding")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_service_config_resolves_default_model_dir() {
        let data_dir = std::env::temp_dir().join(format!("emb-cfg-test-{}", std::process::id()));
        let cfg = EmbeddingConfig::default();
        // 无资源目录 → 数据目录默认路径
        let sc = cfg.to_service_config(&data_dir, None);
        assert_eq!(sc.model_dir, data_dir.join("third_party").join("embedding"));
        assert!(sc.backend == "auto");
    }

    #[test]
    fn to_service_config_prefers_bundled_model_in_resource_dir() {
        let base = std::env::temp_dir().join(format!("emb-cfg-test-{}-bundle", std::process::id()));
        let data_dir = base.join("data");
        let bundled = base
            .join("data")
            .join("third_party")
            .join("embedding_paraphrase_multilingual_minilm_l12_v2");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(bundled.join("model.onnx"), b"").unwrap();
        let cfg = EmbeddingConfig::default();
        let sc = cfg.to_service_config(&data_dir, Some(base.as_path()));
        assert_eq!(sc.model_dir, bundled);
    }

    #[test]
    fn to_service_config_keeps_absolute_model_dir() {
        let data_dir = std::env::temp_dir();
        let mut cfg = EmbeddingConfig::default();
        cfg.model_dir = "/mnt/models".into();
        let sc = cfg.to_service_config(&data_dir, None);
        assert_eq!(sc.model_dir, std::path::PathBuf::from("/mnt/models"));
    }
}
