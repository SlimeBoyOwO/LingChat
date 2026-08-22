//! 云 ASR provider 抽象 + qwen-asr 实现（阿里云 DashScope）。
//!
//! 设计目标：v1 只做"调用云 API"的最薄一层；端点检测、会话编排、配置持久化
//! 由同目录其它子模块负责（vad / session / settings，后续 Task）。
//!
//! 复用策略：
//! - HTTP 客户端由调用方传入（`&reqwest::Client`），调用方负责 TLS / 超时（30s）。
//! - 错误统一返回 [`AsrError`]，不外泄 `reqwest::Error` / `serde_json::Error`。
//! - 不引入新依赖（reqwest / serde / serde_json / async-trait / tracing / thiserror
//!   / base64 都已在 Cargo.toml）。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use base64::Engine as _;
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use tracing::{debug, instrument, warn};

use super::error::AsrError;

// ============================================================================
// 公共结果类型
// ============================================================================

/// provider 识别返回结果。
#[derive(Debug, Clone, Serialize)]
pub struct AsrResult {
    /// 识别出的文本。
    pub text: String,
    /// provider 报告的语言代码（可选）。
    pub language: Option<String>,
    /// provider 报告的置信度 0~1（可选）。
    pub confidence: Option<f32>,
    /// provider id（与 `list_provider_info` 一致）。
    pub provider_id: String,
}

// ============================================================================
// Provider 配置元数据
// ============================================================================

/// provider 配置字段类型，供前端 SettingsAsr.vue 渲染输入框。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFieldKind {
    /// 普通文本。
    Text,
    /// 密码框（API key 等敏感字段）。
    Password,
    /// 整数。
    Number,
    /// 布尔开关。
    Boolean,
}

/// provider 在 UI 上展示需要填写的字段。
#[derive(Debug, Clone, Serialize)]
pub struct AsrConfigField {
    /// 字段 key（写入 `provider_configs[id].<key>`）。
    pub key: &'static str,
    /// 字段显示名（前端可自行 i18n）。
    pub label: &'static str,
    /// 字段类型。
    pub kind: ConfigFieldKind,
    /// 是否必填。
    pub required: bool,
    /// 默认值（字符串形式）。
    pub default_value: Option<&'static str>,
    /// 占位提示文字。
    pub placeholder: Option<&'static str>,
    /// 提示说明（鼠标悬停显示）。
    pub hint: Option<&'static str>,
}

/// provider 静态元数据（id / 显示名 / 配置字段）。
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    /// 唯一 id，写入配置 `active_provider` 用。
    pub id: &'static str,
    /// UI 显示名（如 "OpenAI Whisper"）。
    pub display_name: &'static str,
    /// 简短描述。
    pub description: &'static str,
    /// 默认 endpoint。
    pub default_endpoint: &'static str,
    /// 是否支持流式协议（前端据此决定流式开关是否可用）。
    pub supports_streaming: bool,
    /// UI 需要展示的配置字段。
    pub config_fields: Vec<AsrConfigField>,
}

// ============================================================================
// Provider 凭证（最小子集，不依赖 settings.rs）
// ============================================================================

/// provider 运行时凭证：仅 api_key + endpoint + model。
///
/// 设计原因：Task 3 才创建 settings.rs 中的 `AsrSettings` / `ProviderConfig`。
/// 本 Task 先行定义 provider 真正需要的字段，Task 3 做适配层把
/// `AsrSettings::provider_configs[id]` 转成本结构传入。
#[derive(Debug, Clone, Default)]
pub struct ProviderCredentials {
    pub api_key: String,
    pub endpoint: String,
    /// 识别的模型名；空串 = provider 默认模型（如 qwen 的 fun-asr-realtime）。
    pub model: String,
}

impl ProviderCredentials {
    /// 从 endpoint 字符串中剔除末尾 `/`，便于直接拼 `/audio/transcriptions`。
    pub fn normalized_endpoint(&self) -> String {
        self.endpoint.trim_end_matches('/').to_string()
    }

    /// api_key 是否非空（剪掉首尾空白后判断）。
    pub fn has_api_key(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

// ============================================================================
// AsrProvider trait
// ============================================================================

/// 所有云 ASR provider 必须实现的接口。
#[async_trait]
pub trait AsrProvider: Send + Sync {
    /// provider id（与 `ProviderInfo.id` 一致）。
    fn id(&self) -> &'static str;

    /// UI 显示名。
    fn display_name(&self) -> &'static str;

    /// provider 在 SettingsAsr.vue 中渲染所需的配置字段。
    fn config_fields(&self) -> Vec<AsrConfigField>;

    /// 调用云 API 识别一段 WAV 字节。
    ///
    /// - `wav_bytes`：前端 OfflineAudioContext 重采样后的 16kHz mono WAV。
    /// - `language_hint`：可选 BCP-47 码，如 `"zh"` / `"en"` / `"ja"`。
    ///
    /// 错误统一返回 [`AsrError`]。
    async fn recognize(
        &self,
        wav_bytes: Vec<u8>,
        language_hint: Option<&str>,
    ) -> Result<AsrResult, AsrError>;

    /// 是否支持流式协议（WebSocket 实时识别）。默认不支持。
    fn supports_streaming(&self) -> bool {
        false
    }
}


// ============================================================================
// Qwen ASR (DashScope)
// ============================================================================

/// Qwen ASR（阿里云 DashScope）。
///
/// 非流式走 `multimodal-generation` 端点（JSON body + base64 音频）；
/// 流式（paraformer-realtime-v2）走 WebSocket 实时端点（provider_stream.rs）。
pub struct QwenAsrProvider {
    http: reqwest::Client,
    cred: ProviderCredentials,
}

impl QwenAsrProvider {
    const ID: &'static str = "qwen-asr";
    const DISPLAY: &'static str = "Qwen ASR";
    const DEFAULT_ENDPOINT: &'static str =
        "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
    const MODEL: &'static str = "fun-asr-realtime";

    pub fn new(http: reqwest::Client, cred: ProviderCredentials) -> Result<Self, AsrError> {
        if !cred.has_api_key() {
            return Err(AsrError::MissingCredentials(
                "Qwen ASR 需要 DashScope api_key".into(),
            ));
        }
        Ok(Self { http, cred })
    }
}

#[async_trait]
impl AsrProvider for QwenAsrProvider {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn display_name(&self) -> &'static str {
        Self::DISPLAY
    }

    fn config_fields(&self) -> Vec<AsrConfigField> {
        qwen_asr_config_fields()
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    #[instrument(skip(self, wav_bytes), fields(provider = Self::ID))]
    async fn recognize(
        &self,
        wav_bytes: Vec<u8>,
        language_hint: Option<&str>,
    ) -> Result<AsrResult, AsrError> {
        let endpoint = if self.cred.normalized_endpoint().is_empty() {
            Self::DEFAULT_ENDPOINT.to_string()
        } else {
            self.cred.normalized_endpoint()
        };

        // DashScope 非实时 Fun-ASR-Realtime 协议（multimodal-generation）：
        // JSON body + audio 以 data URL（base64 inline）放在 user message 里。
        // 参考官方 SDK Recognition.call + 文档「非实时语音识别（Fun-ASR-Realtime）API参考」。
        // 注：language_hints 仅 paraformer-realtime-v2 支持，fun-asr-realtime 不传。
        let _ = language_hint;
        // 模型自选：cred.model 为空或为流式模型（非流式端点不认识）→ 回退默认非流式模型。
        // 流式模型（paraformer-realtime-*）只能走 WebSocket 实时端点（asr_start_streaming），
        // 否则 DashScope 返回 HTTP 400 "url error"（模型名与端点不匹配）。
        let model = if self.cred.model.is_empty() || qwen_is_streaming_model(&self.cred.model) {
            Self::MODEL
        } else {
            self.cred.model.as_str()
        };
        let b64 = BASE64_STD.encode(&wav_bytes);
        let body = json!({
            "model": model,
            "input": {
                "messages": [{
                    "role": "user",
                    "content": [{
                        "audio": format!("data:audio/wav;base64,{b64}")
                    }]
                }]
            },
            "parameters": {
                "format": "wav",
                "sample_rate": 16000
            },
            "resources": []
        });

        let resp = self
            .http
            .post(&endpoint)
            .bearer_auth(&self.cred.api_key)
            .header("X-DashScope-SSE", "disable")
            .json(&body)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = resp.status();
        if status == reqwest::StatusCode::REQUEST_TIMEOUT
            || status == reqwest::StatusCode::GATEWAY_TIMEOUT
        {
            return Err(AsrError::ProviderTimeout(Self::ID.into()));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AsrError::ProviderApiError {
                provider: Self::ID.into(),
                message: format!("HTTP {status}: {body}"),
            });
        }

        let body_text = resp.text().await.map_err(map_reqwest_error)?;
        let text = parse_qwen_text(&body_text).ok_or_else(|| AsrError::ProviderApiError {
            provider: Self::ID.into(),
            message: format!("无法从响应中提取文本: {body_text}"),
        })?;

        Ok(AsrResult {
            text,
            language: language_hint.map(str::to_string),
            confidence: None,
            provider_id: Self::ID.into(),
        })
    }
}

/// 解析 DashScope multimodal-generation 响应文本。
///
/// Fun-ASR-Realtime 非流式实际响应结构（实测）：
/// `{"output": {"output": {"text": "识别文本", "sentence": {...}}, "usage": {...}}}`
/// 宽松解析：优先 `output.output.text` / `output.output.sentence.text`，
/// 兜底 OpenAI 风格 `output.choices[0].message.content` 及 `text` 字段。
fn parse_qwen_text(body: &str) -> Option<String> {
    let value: JsonValue = serde_json::from_str(body).ok()?;
    // Fun-ASR-Realtime：output.output.text（sentence 内也有一份）
    if let Some(s) = value
        .get("output")
        .and_then(|v| v.get("output"))
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
    {
        return Some(s.to_string());
    }
    if let Some(s) = value
        .get("output")
        .and_then(|v| v.get("output"))
        .and_then(|v| v.get("sentence"))
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
    {
        return Some(s.to_string());
    }
    // OpenAI 风格：output.choices[0].message.content（content 可能是数组）
    if let Some(content) = value
        .get("output")
        .and_then(|v| v.get("choices"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("message"))
        .and_then(|v| v.get("content"))
    {
        if let Some(s) = content.as_str() {
            return Some(s.to_string());
        }
        if let Some(arr) = content.as_array() {
            let joined: String = arr
                .iter()
                .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
                .collect();
            if !joined.is_empty() {
                return Some(joined);
            }
        }
    }
    if let Some(s) = value.get("text").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = value.get("output").and_then(|v| v.get("text")).and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = value.get("result").and_then(|v| v.get("text")).and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    None
}

// ============================================================================
// 工具函数
// ============================================================================

/// 把 `reqwest::Error` 映射成 [`AsrError`]。
///
/// reqwest 的网络/超时/协议错误统一归类为 provider 错误；上层无需关心细节。
fn map_reqwest_error(e: reqwest::Error) -> AsrError {
    if e.is_timeout() {
        AsrError::ProviderTimeout("network".into())
    } else if e.is_connect() || e.is_request() {
        AsrError::ProviderApiError {
            provider: "network".into(),
            message: format!("请求失败: {e}"),
        }
    } else {
        warn!("reqwest 错误: {e}");
        AsrError::ProviderApiError {
            provider: "network".into(),
            message: format!("{e}"),
        }
    }
}

/// 构造一个 30s 超时的默认 reqwest Client。
///
/// 仅供测试 / 内部默认；生产环境调用方应通过 `factory::build_http_client`
/// 注入正确的 TLS 配置。
#[allow(dead_code)]
pub fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("构建默认 HTTP 客户端失败")
}

// ============================================================================
// Provider 注册表
// ============================================================================

/// 列出所有 provider 的静态元数据（目前仅阿里云 qwen-asr）。
pub fn list_provider_info() -> Vec<ProviderInfo> {
    vec![ProviderInfo {
        id: QwenAsrProvider::ID,
        display_name: QwenAsrProvider::DISPLAY,
        description: "阿里云 DashScope ASR（实时 / 非实时）",
        default_endpoint: QwenAsrProvider::DEFAULT_ENDPOINT,
        supports_streaming: true,
        config_fields: qwen_asr_config_fields(),
    }]
}

/// 模型元数据（`asr_list_models` 返回给前端渲染下拉）。
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    /// 模型 id（写入 `provider_configs[id].model`）。
    pub id: &'static str,
    /// UI 显示名。
    pub display_name: &'static str,
    /// 是否支持流式协议（前端流式开关可用性的权威判定）。
    pub supports_streaming: bool,
    /// 是否默认模型（`provider_configs[id].model` 为空时生效）。
    pub is_default: bool,
}

/// qwen（DashScope）语音识别模型静态清单。
///
/// 仅列协议已接入的模型：multimodal-generation 一次性返回（非流式）+
/// WebSocket 实时（流式）。异步任务类（paraformer-v2/-8k）与
/// OpenAI-compatible（qwen-audio-asr）协议未接入，不列出。
pub fn qwen_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "fun-asr-realtime",
            display_name: "Fun-ASR-Realtime（非实时）",
            supports_streaming: false,
            is_default: true,
        },
        ModelInfo {
            id: "paraformer-realtime-v2",
            display_name: "Paraformer-Realtime-V2",
            supports_streaming: true,
            is_default: false,
        },
    ]
}

/// qwen 流式模型集合（与 [`qwen_models`] 保持同步）。
///
/// 流式模型只能走 WebSocket 实时端点；非流式端点（multimodal-generation）
/// 不认识它们，DashScope 会返回 HTTP 400 "url error"（模型名与端点不匹配）。
pub fn qwen_is_streaming_model(model: &str) -> bool {
    matches!(model, "paraformer-realtime-v2")
}

/// 按 provider id 返回模型清单；未接入模型选择的 provider 返回空数组
/// （前端据此隐藏模型下拉）。
pub fn list_models(provider_id: &str) -> Vec<ModelInfo> {
    match provider_id {
        QwenAsrProvider::ID => qwen_models(),
        _ => Vec::new(),
    }
}

/// 按 id 创建 provider 实例。
///
/// 找不到 id 时返回 [`AsrError::ProviderNotFound`]。
pub async fn get_provider(
    id: &str,
    cred: &ProviderCredentials,
    http: &reqwest::Client,
) -> Result<Arc<dyn AsrProvider>, AsrError> {
    debug!("创建 ASR provider: {id}");
    let provider: Arc<dyn AsrProvider> = match id {
        QwenAsrProvider::ID => Arc::new(QwenAsrProvider::new(http.clone(), cred.clone())?),
        other => {
            return Err(AsrError::ProviderNotFound(other.into()));
        }
    };
    Ok(provider)
}

// ============================================================================
// 静态字段（让 trait 方法与 list_provider_info 共用一份数据）
// ============================================================================

fn qwen_asr_config_fields() -> Vec<AsrConfigField> {
    vec![
        AsrConfigField {
            key: "api_key",
            label: "DashScope API Key",
            kind: ConfigFieldKind::Password,
            required: true,
            default_value: None,
            placeholder: Some("sk-..."),
            hint: Some("阿里云百炼 / DashScope 平台 Key"),
        },
        AsrConfigField {
            key: "endpoint",
            label: "Endpoint",
            kind: ConfigFieldKind::Text,
            required: false,
            default_value: Some(QwenAsrProvider::DEFAULT_ENDPOINT),
            placeholder: Some("非实时 Fun-ASR-Realtime 端点"),
            hint: Some("默认 DashScope multimodal-generation；填自建代理时整段替换"),
        },
    ]
}

// ============================================================================
// 单元测试（zero deps：仅覆盖可纯函数测的部分）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_qwen_text_prefers_top_level_text() {
        let body = r#"{"text": "你好"}"#;
        assert_eq!(parse_qwen_text(body).as_deref(), Some("你好"));
    }

    #[test]
    fn parse_qwen_text_falls_back_to_output_text() {
        let body = r#"{"output": {"text": "hi"}}"#;
        assert_eq!(parse_qwen_text(body).as_deref(), Some("hi"));
    }

    #[test]
    fn parse_qwen_text_falls_back_to_result_text() {
        let body = r#"{"result": {"text": "hola"}}"#;
        assert_eq!(parse_qwen_text(body).as_deref(), Some("hola"));
    }

    #[test]
    fn parse_qwen_text_returns_none_for_garbage() {
        assert!(parse_qwen_text("not json").is_none());
        assert!(parse_qwen_text(r#"{"foo": 1}"#).is_none());
    }

    #[test]
    fn provider_credentials_normalizes_trailing_slash() {
        let c = ProviderCredentials {
            api_key: "k".into(),
            endpoint: "http://x.example.com/".into(),
        };
        assert_eq!(c.normalized_endpoint(), "http://x.example.com");
        assert!(c.has_api_key());
    }

    #[test]
    fn provider_credentials_whitespace_api_key_is_empty() {
        let c = ProviderCredentials {
            api_key: "   ".into(),
            endpoint: "".into(),
        };
        assert!(!c.has_api_key());
    }

    #[test]
    fn list_provider_info_only_qwen() {
        let info = list_provider_info();
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].id, "qwen-asr");
    }
}
