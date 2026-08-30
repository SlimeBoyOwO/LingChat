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
//!
//! ============================================================================
//! 扩展指南（新增 provider / 接入 OpenAI 兼容服务）
//! ============================================================================
//!
//! 新增一个**专用协议** provider（3 步，参照 [`QwenAsrProvider`]）：
//! 1. 实现 [`AsrProvider`] trait（recognize 必选；流式见下）
//! 2. 写 `config_fields()`——设置页据此动态渲染输入框，前端零改动
//! 3. 注册到 `list_provider_info()` 与 `get_provider()`
//!
//! 接入**OpenAI 兼容服务**（whisper.cpp / faster-whisper-server / Groq /
//! 阿里云 DashScope compatible-mode 的 qwen-audio-asr 等）：
//! - 协议是 `POST {endpoint}/v1/audio/transcriptions`（multipart：file/model/
//!   可选 language/prompt）+ `GET /v1/models` 动态模型列表
//! - 现成的可复用件：
//!   - [`provider_stream_llama::recognize_stream`]——通用 SSE 结果流式客户端
//!     （对 `<asr_text>` 标记自动检测，纯 OpenAI 文本也能解析；llama-asr 已在用）
//!   - [`parse_llama_text`] / [`parse_llama_models`]——响应与模型列表解析
//! - 泛化方案备忘（未实施，2026-08 评审预留）：
//!   - 新增固定 id `openai-compatible` 的通用 struct（id 保持 `&'static str`，
//!     trait 无需改；参照 LLM 侧 `lmstudio` 固定 id + 可配 endpoint 的先例）
//!   - llama-asr 保留（默认端点/默认模型/热词有友好默认值，老配置不断），
//!     与通用 struct 共享上述复用件
//!   - 前端 `isLlamaStream()`（useAsrInput.ts）需改为覆盖 SSE 类 provider
//!     的集合判定——llama-asr 与 openai-compatible 都是"整段上传 + SSE
//!     partial"，必须同链路，新增第三个 SSE 类 provider 时同步扩展该判定

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
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

/// provider 运行时凭证：api_key + endpoint + model + 热词。
#[derive(Debug, Clone, Default)]
pub struct ProviderCredentials {
    pub api_key: String,
    pub endpoint: String,
    /// 识别的模型名；空串 = provider 默认模型（如 qwen 的 fun-asr-realtime）。
    pub model: String,
    /// 热词列表（llama-asr 的 `prompt` 偏置字段；qwen 忽略）。
    /// 来源 `ProviderConfig.extra["hotwords"]`（逗号分隔，见 settings.rs）。
    pub hotwords: Vec<String>,
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

    /// 结果流式识别（SSE 类协议：音频整段上传、结果增量返回）。默认不支持。
    ///
    /// 与 WS 会话流式（`asr_start_streaming`/`stop_streaming`）独立：llama-asr
    /// 走这里（provider_stream_llama.rs），qwen 走 WebSocket 会话路径。
    /// 默认实现返回 [`AsrError::StreamingNotSupported`]。
    ///
    /// `on_partial`：增量文本回调（整段累积视图，每次整体替换）——由调用方
    /// （session / 命令层）注入，provider 不直接依赖 Tauri 事件发射（展示
    /// 与识别解耦，provider 可脱离 Tauri 环境测试）。
    async fn stream_recognize(
        &self,
        _wav_bytes: Vec<u8>,
        _on_partial: Option<Arc<dyn for<'a> Fn(&'a str) + Send + Sync + 'static>>,
    ) -> Result<AsrResult, AsrError> {
        Err(AsrError::StreamingNotSupported(self.id().into()))
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
        // 非流式端点校验：仅接受 http(s) URL。流式预设（wss://...）或空值
        // 一律回退默认 HTTP 端点——否则 reqwest 对 wss:// 报 builder error。
        let endpoint = {
            let e = self.cred.normalized_endpoint();
            if e.is_empty() || !(e.starts_with("http://") || e.starts_with("https://")) {
                Self::DEFAULT_ENDPOINT.to_string()
            } else {
                e
            }
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
    if let Some(s) = value
        .get("output")
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
    {
        return Some(s.to_string());
    }
    if let Some(s) = value
        .get("result")
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
    {
        return Some(s.to_string());
    }
    None
}

// ============================================================================
// Llama ASR (llama.cpp llama-server，本地 Qwen3-ASR)
// ============================================================================

/// 本地 llama-server（llama.cpp）Qwen3-ASR。
///
/// 协议（D:\asr-deploy\API接入文档.md 实测）：
/// - 端点 `POST {endpoint}/v1/audio/transcriptions`（OpenAI 兼容 multipart）
/// - 音频必须是 16kHz 单声道 WAV（前端 OfflineAudioContext 已产出同格式）
/// - `model` 必须是 `/v1/models` 返回的全名（简写会 400 model not found）
/// - 响应 `text` 格式 `language <lang><asr_text><文本>`，切 `<asr_text>` 取文本
/// - 热词：multipart `prompt` 字段做上下文偏置（偏置非强制；热词接口保留，
///   设置页暂不做输入 UI，来源 `ProviderConfig.extra["hotwords"]` 逗号分隔）
/// - 流式：llama-server 走 SSE（HTTP，OpenAI 兼容语义——每条 data 是当前
///   累积的完整转录）——结果流式经 `stream_recognize` 接入（provider_stream_llama.rs），
///   partial 经 `asr://stream_partial` 事件实时 emit；音频仍整段上传
///   （Qwen3-ASR 非因果 encoder，无流式音频输入）
pub struct LlamaAsrProvider {
    http: reqwest::Client,
    cred: ProviderCredentials,
}

impl LlamaAsrProvider {
    pub const ID: &'static str = "llama-asr";
    pub const DISPLAY: &'static str = "本地 ASR（llama-server）";
    pub const DEFAULT_ENDPOINT: &'static str = "http://127.0.0.1:8080";
    pub const DEFAULT_MODEL: &'static str = "models/Qwen3-ASR-1.7B-Q8_0.gguf";

    pub fn new(http: reqwest::Client, cred: ProviderCredentials) -> Self {
        Self { http, cred }
    }

    /// 模型选择：配置非空用配置，否则默认 1.7B 全名。
    fn effective_model(&self) -> String {
        if self.cred.model.trim().is_empty() {
            Self::DEFAULT_MODEL.to_string()
        } else {
            self.cred.model.trim().to_string()
        }
    }

    /// 端点选择：配置非空且为 http(s) URL 时用配置，否则默认 `127.0.0.1:8080`。
    ///
    /// 与 qwen 同款校验——空 endpoint 会拼出相对 URL，reqwest 报 builder error
    ///（设置页未填 endpoint 时配置为空串，必须回退默认）。
    fn effective_endpoint(&self) -> String {
        let e = self.cred.normalized_endpoint();
        if e.is_empty() || !(e.starts_with("http://") || e.starts_with("https://")) {
            Self::DEFAULT_ENDPOINT.to_string()
        } else {
            e
        }
    }
}

#[async_trait]
impl AsrProvider for LlamaAsrProvider {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn display_name(&self) -> &'static str {
        Self::DISPLAY
    }

    fn config_fields(&self) -> Vec<AsrConfigField> {
        llama_asr_config_fields()
    }

    fn supports_streaming(&self) -> bool {
        // 结果流式（SSE）已接入（stream_recognize / provider_stream_llama.rs）；
        // 与 llama_models() 的模型级 supports_streaming=true 保持一致
        true
    }

    #[instrument(skip(self, wav_bytes), fields(provider = Self::ID))]
    async fn recognize(
        &self,
        wav_bytes: Vec<u8>,
        language_hint: Option<&str>,
    ) -> Result<AsrResult, AsrError> {
        // llama-server 转写不支持语言提示（模型自动判语言），忽略。
        let _ = language_hint;
        let endpoint = format!("{}/v1/audio/transcriptions", self.effective_endpoint());

        let mut form = reqwest::multipart::Form::new()
            .text("model", self.effective_model())
            .text("response_format", "json")
            .part(
                "file",
                reqwest::multipart::Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| AsrError::ProviderApiError {
                        provider: Self::ID.into(),
                        message: format!("构造 multipart 失败: {e}"),
                    })?,
            );
        // 热词接口：extra["hotwords"] 非空时作为 prompt 上下文偏置传入
        //（偏置非强制——提升特定词命中概率，不保证一定识别为热词）
        if !self.cred.hotwords.is_empty() {
            form = form.text("prompt", self.cred.hotwords.join(", "));
        }

        let mut req = self.http.post(&endpoint).multipart(form);
        // api_key 可选：本地服务无鉴权时不发；带 --api-key 部署时用 Bearer
        if self.cred.has_api_key() {
            req = req.bearer_auth(&self.cred.api_key);
        }
        let resp = req.send().await.map_err(map_reqwest_error)?;

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
        let (text, language) =
            parse_llama_text(&body_text).ok_or_else(|| AsrError::ProviderApiError {
                provider: Self::ID.into(),
                message: format!("无法从响应中提取文本: {body_text}"),
            })?;

        Ok(AsrResult {
            text,
            language,
            confidence: None,
            provider_id: Self::ID.into(),
        })
    }
    /// 结果流式识别：整段 WAV 上传 + SSE 增量 partial（`on_partial` 回调，
    /// 事件发射由调用方负责）→ final。复用整句的端点/模型/热词选择逻辑。
    #[instrument(skip(self, wav_bytes, on_partial), fields(provider = Self::ID))]
    async fn stream_recognize(
        &self,
        wav_bytes: Vec<u8>,
        on_partial: Option<Arc<dyn for<'a> Fn(&'a str) + Send + Sync + 'static>>,
    ) -> Result<AsrResult, AsrError> {
        super::provider_stream_llama::recognize_stream(
            &self.http,
            &self.cred,
            &self.effective_endpoint(),
            &self.effective_model(),
            wav_bytes,
            on_partial,
        )
        .await
    }
}

/// 解析 llama-server 转写响应文本。
///
/// 实测格式：`{"text": "language <lang><asr_text><转写文本>"}`（无语音时
/// `<lang>` 为 `None`、文本为空）。切 `<asr_text>`：后半是文本，前半是语言。
/// 供整句识别与 SSE 结果流式（provider_stream_llama.rs）共用。
pub(crate) fn parse_llama_text(body: &str) -> Option<(String, Option<String>)> {
    let value: JsonValue = serde_json::from_str(body).ok()?;
    let text = value.get("text").and_then(|t| t.as_str())?;
    match text.split_once("<asr_text>") {
        Some((lang_part, content)) => {
            // `language Chinese` → Chinese；`language None` → None
            let lang = lang_part
                .strip_prefix("language")
                .map(str::trim)
                .filter(|s| !s.is_empty() && *s != "None")
                .map(str::to_string);
            Some((content.to_string(), lang))
        },
        None => Some((text.to_string(), None)),
    }
}

/// 解析 llama-server `/v1/models` 响应，提取模型全名列表。
///
/// 兼容两种结构：`data[].id`（OpenAI 兼容）与 `models[].name`。
fn parse_llama_models(body: &str) -> Vec<String> {
    let value: JsonValue = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    if let Some(data) = value.get("data").and_then(|v| v.as_array()) {
        for item in data {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                names.push(id.to_string());
            }
        }
    }
    if names.is_empty() {
        if let Some(models) = value.get("models").and_then(|v| v.as_array()) {
            for item in models {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
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

/// 构造一个 30s 超时的默认 reqwest Client（TLS 走统一的 webpki-roots 配置，
/// Android 上 rustls-platform-verifier 未初始化会 panic，见 utils/tls.rs）。
///
/// 仅供测试 / 内部默认；生产环境调用方应通过 `factory::build_http_client`
/// 注入正确的 TLS 配置。
#[allow(dead_code)]
pub fn default_http_client() -> reqwest::Client {
    let tls = crate::utils::tls::build_tls_config().expect("构建默认 TLS 配置失败");
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("构建默认 HTTP 客户端失败")
}

// ============================================================================
// Provider 注册表
// ============================================================================

/// 列出所有 provider 的静态元数据。
pub fn list_provider_info() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: QwenAsrProvider::ID,
            display_name: QwenAsrProvider::DISPLAY,
            description: "阿里云 DashScope ASR（实时 / 非实时）",
            default_endpoint: QwenAsrProvider::DEFAULT_ENDPOINT,
            supports_streaming: true,
            config_fields: qwen_asr_config_fields(),
        },
        ProviderInfo {
            id: LlamaAsrProvider::ID,
            display_name: LlamaAsrProvider::DISPLAY,
            description: "本地 llama-server Qwen3-ASR（整句识别）",
            default_endpoint: LlamaAsrProvider::DEFAULT_ENDPOINT,
            supports_streaming: false,
            config_fields: llama_asr_config_fields(),
        },
    ]
}

/// 模型元数据（`asr_list_models` 返回给前端渲染下拉）。
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    /// 模型 id（写入 `provider_configs[id].model`）。qwen 是协议名；
    /// llama-asr 是 `/v1/models` 返回的动态全名（非静态，用 String）。
    pub id: String,
    /// UI 显示名。
    pub display_name: String,
    /// 是否支持流式协议（前端流式开关可用性的权威判定）。
    pub supports_streaming: bool,
    /// 是否默认模型（`provider_configs[id].model` 为空时生效）。
    pub is_default: bool,
    /// 协议端点预设（选中该模型时同步填入 endpoint 配置；None = 无预设，
    /// 用当前 endpoint 配置——llama-asr 的端点与模型无关）。
    pub endpoint: Option<String>,
}

/// qwen（DashScope）语音识别模型静态清单。
///
/// 仅列协议已接入的模型：multimodal-generation 一次性返回（非流式）+
/// WebSocket 实时（流式）。异步任务类（paraformer-v2/-8k）与
/// OpenAI-compatible（qwen-audio-asr）协议未接入，不列出。
pub fn qwen_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "fun-asr-realtime".into(),
            display_name: "Fun-ASR-Realtime（非实时）".into(),
            supports_streaming: false,
            is_default: true,
            endpoint: Some(QwenAsrProvider::DEFAULT_ENDPOINT.to_string()),
        },
        ModelInfo {
            id: "paraformer-realtime-v2".into(),
            display_name: "Paraformer-Realtime-V2".into(),
            supports_streaming: true,
            is_default: false,
            endpoint: Some(super::provider_stream::WS_URL.to_string()),
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

/// 按 provider id 返回模型清单。
///
/// qwen 返回静态清单；llama-asr 动态请求服务端 `/v1/models`（endpoint 取
/// 当前设置，默认 `http://127.0.0.1:8080`；服务未启动/模型列表为空时返回
/// `ProviderApiError`，前端展示错误并回退为模型文本输入）。未接入模型选择
/// 的 provider 返回空数组（前端据此隐藏模型下拉）。
pub async fn list_models(
    provider_id: &str,
    app: &tauri::AppHandle,
    http: &reqwest::Client,
) -> Result<Vec<ModelInfo>, AsrError> {
    match provider_id {
        QwenAsrProvider::ID => Ok(qwen_models()),
        LlamaAsrProvider::ID => llama_models(app, http).await,
        _ => Ok(Vec::new()),
    }
}

/// 请求 llama-server `/v1/models`，映射为 ModelInfo 列表（llama-asr 全部非流式）。
///
/// 显示名取模型文件名的最后一段（`models/Qwen3-ASR-1.7B-Q8_0.gguf` →
/// `Qwen3-ASR-1.7B-Q8_0.gguf`），id 保留全名（服务端按全名匹配模型）。
async fn llama_models(
    app: &tauri::AppHandle,
    http: &reqwest::Client,
) -> Result<Vec<ModelInfo>, AsrError> {
    let settings = super::settings::load(app)?;
    let cred = settings
        .provider_configs
        .get(LlamaAsrProvider::ID)
        .cloned()
        .unwrap_or_default();
    let endpoint = if cred.endpoint.trim().is_empty() {
        LlamaAsrProvider::DEFAULT_ENDPOINT.to_string()
    } else {
        cred.endpoint.trim_end_matches('/').to_string()
    };
    let url = format!("{endpoint}/v1/models");
    debug!("[ASR] 拉取模型列表: {url}");
    let resp = http.get(&url).send().await.map_err(map_reqwest_error)?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AsrError::ProviderApiError {
            provider: LlamaAsrProvider::ID.into(),
            message: format!("HTTP {status}: {body}"),
        });
    }
    let body_text = resp.text().await.map_err(map_reqwest_error)?;
    let names = parse_llama_models(&body_text);
    if names.is_empty() {
        return Err(AsrError::ProviderApiError {
            provider: LlamaAsrProvider::ID.into(),
            message: "模型列表为空（/v1/models 未返回任何模型）".into(),
        });
    }
    Ok(names
        .into_iter()
        .enumerate()
        .map(|(i, id)| ModelInfo {
            display_name: id.rsplit('/').next().unwrap_or(&id).to_string(),
            // 结果流式（SSE）：音频整段上传、结果增量返回。与 qwen WS 真流式
            // 语义不同，但前端流式开关可用（录音结束出 partial 而非边录边出）
            supports_streaming: true,
            is_default: i == 0,
            endpoint: None,
            id,
        })
        .collect())
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
        LlamaAsrProvider::ID => Arc::new(LlamaAsrProvider::new(http.clone(), cred.clone())),
        other => {
            return Err(AsrError::ProviderNotFound(other.into()));
        },
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

fn llama_asr_config_fields() -> Vec<AsrConfigField> {
    vec![
        AsrConfigField {
            key: "endpoint",
            label: "服务地址",
            kind: ConfigFieldKind::Text,
            required: false,
            default_value: Some(LlamaAsrProvider::DEFAULT_ENDPOINT),
            placeholder: Some("http://127.0.0.1:8080"),
            hint: Some("llama-server 地址（Qwen3-ASR 本地部署）；局域网部署改 http://<IP>:8080"),
        },
        AsrConfigField {
            key: "model",
            label: "模型",
            kind: ConfigFieldKind::Text,
            required: false,
            default_value: Some(LlamaAsrProvider::DEFAULT_MODEL),
            placeholder: Some("models/Qwen3-ASR-1.7B-Q8_0.gguf"),
            hint: Some("从上方模型列表选择，或用 /v1/models 查询服务端全名"),
        },
        AsrConfigField {
            key: "api_key",
            label: "API Key（可选）",
            kind: ConfigFieldKind::Password,
            required: false,
            default_value: None,
            placeholder: Some("本地服务无需填写"),
            hint: Some("llama-server 带 --api-key 部署时填写，否则留空"),
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
            model: String::new(),
            hotwords: Vec::new(),
        };
        assert_eq!(c.normalized_endpoint(), "http://x.example.com");
        assert!(c.has_api_key());
    }

    #[test]
    fn provider_credentials_whitespace_api_key_is_empty() {
        let c = ProviderCredentials {
            api_key: "   ".into(),
            endpoint: "".into(),
            model: String::new(),
            hotwords: Vec::new(),
        };
        assert!(!c.has_api_key());
    }

    #[test]
    fn list_provider_info_has_two_providers() {
        let info = list_provider_info();
        assert_eq!(info.len(), 2);
        assert!(info.iter().any(|p| p.id == "qwen-asr"));
        assert!(info.iter().any(|p| p.id == "llama-asr"));
    }

    #[test]
    fn llama_effective_endpoint_falls_back_when_empty() {
        // 空 endpoint → 默认 127.0.0.1:8080（否则拼出相对 URL，reqwest builder error）
        let p = LlamaAsrProvider::new(
            default_http_client(),
            ProviderCredentials {
                endpoint: String::new(),
                ..Default::default()
            },
        );
        assert_eq!(p.effective_endpoint(), LlamaAsrProvider::DEFAULT_ENDPOINT);
    }

    #[test]
    fn llama_effective_endpoint_rejects_non_http_scheme() {
        // 缺 http:// 前缀（手填 127.0.0.1:8080）→ 回退默认
        let p = LlamaAsrProvider::new(
            default_http_client(),
            ProviderCredentials {
                endpoint: "127.0.0.1:8080".into(),
                ..Default::default()
            },
        );
        assert_eq!(p.effective_endpoint(), LlamaAsrProvider::DEFAULT_ENDPOINT);
    }

    #[test]
    fn llama_effective_endpoint_keeps_valid_url_and_trims_slash() {
        let p = LlamaAsrProvider::new(
            default_http_client(),
            ProviderCredentials {
                endpoint: "http://192.168.1.5:9000/".into(),
                ..Default::default()
            },
        );
        assert_eq!(p.effective_endpoint(), "http://192.168.1.5:9000");
    }

    #[test]
    fn parse_llama_text_splits_asr_text_marker() {
        let body = r#"{"text":"language Chinese<asr_text>甚至出现交易几乎停滞的情况。"}"#;
        let (text, lang) = parse_llama_text(body).expect("应解析成功");
        assert_eq!(text, "甚至出现交易几乎停滞的情况。");
        assert_eq!(lang.as_deref(), Some("Chinese"));
    }

    #[test]
    fn parse_llama_text_none_language_is_empty() {
        // 无语音：`language None<asr_text>` 空文本
        let body = r#"{"text":"language None<asr_text>"}"#;
        let (text, lang) = parse_llama_text(body).expect("应解析成功");
        assert_eq!(text, "");
        assert_eq!(lang, None);
    }

    #[test]
    fn parse_llama_text_plain_text_fallback() {
        // 无 <asr_text> 标记（协议异常兜底）：整体当文本
        let body = r#"{"text":"你好"}"#;
        let (text, lang) = parse_llama_text(body).expect("应解析成功");
        assert_eq!(text, "你好");
        assert_eq!(lang, None);
    }

    #[test]
    fn parse_llama_text_garbage_returns_none() {
        assert!(parse_llama_text("not json").is_none());
        assert!(parse_llama_text(r#"{"foo": 1}"#).is_none());
    }

    #[test]
    fn parse_llama_models_extracts_data_ids() {
        let body = r#"{"data":[{"id":"models/Qwen3-ASR-1.7B-Q8_0.gguf"},{"id":"models/Qwen3-ASR-0.6B-Q8_0.gguf"}]}"#;
        assert_eq!(
            parse_llama_models(body),
            vec![
                "models/Qwen3-ASR-1.7B-Q8_0.gguf",
                "models/Qwen3-ASR-0.6B-Q8_0.gguf"
            ]
        );
    }

    #[test]
    fn parse_llama_models_falls_back_to_models_names() {
        // llama.cpp 某些版本返回 models[].name 而非 data[].id
        let body = r#"{"models":[{"name":"models/Qwen3-ASR-1.7B-Q8_0.gguf"}]}"#;
        assert_eq!(
            parse_llama_models(body),
            vec!["models/Qwen3-ASR-1.7B-Q8_0.gguf"]
        );
    }

    #[test]
    fn parse_llama_models_empty_or_garbage() {
        assert!(parse_llama_models("not json").is_empty());
        assert!(parse_llama_models(r#"{"data":[]}"#).is_empty());
    }
}
