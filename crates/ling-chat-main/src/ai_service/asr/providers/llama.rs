//! 本地 llama-server（llama.cpp）ASR provider 实现。
//!
//! 结果流式（SSE）实现见 `crate::ai_service::asr::provider_stream_llama`。

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tracing::{debug, instrument};

use crate::ai_service::asr::config_fields::llama_asr_config_fields;
use crate::ai_service::asr::error::{AsrError, map_reqwest_error};
use crate::ai_service::asr::provider::AsrProvider;
use crate::ai_service::asr::provider_meta::AsrConfigField;
use crate::ai_service::asr::provider_types::{AsrOptions, AsrResult, Hotword, ProviderCredentials};

/// 本地 llama-server（llama.cpp）Qwen3-ASR。
///
/// 协议（D:\asr-deploy\API接入文档.md 实测）：
/// - 端点 `POST {endpoint}/v1/audio/transcriptions`（OpenAI 兼容 multipart）
/// - 音频必须是 16kHz 单声道 WAV（前端 OfflineAudioContext 已产出同格式）
/// - `model` 必须是 `/v1/models` 返回的全名（简写会 400 model not found）
/// - 响应 `text` 格式 `language <lang><asr_text><文本>`，切 `<asr_text>` 取文本
/// - 热词：multipart `prompt` 字段做上下文偏置（偏置非强制；来源是调用方传入的
///   `AsrOptions::hotwords`，即逐角色的热词——设置页已无热词入口。
///   另注：llama-server 是否真的消费 `prompt` 未经实测，见 V6）
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

    #[instrument(skip(self, wav_bytes, opts), fields(provider = Self::ID))]
    async fn recognize(
        &self,
        wav_bytes: Vec<u8>,
        opts: &AsrOptions,
    ) -> Result<AsrResult, AsrError> {
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
        // 热词接口：作为 prompt 上下文偏置传入
        //（偏置非强制——提升特定词命中概率，不保证一定识别为热词）
        if let Some(p) = llama_prompt_from_hotwords(&opts.hotwords) {
            if p.dropped > 0 {
                debug!(
                    "[ASR] llama-asr prompt 偏置 {}/{} 字符（保留 {}/{} 条热词，{} 条超预算未发送）: {}",
                    p.text.chars().count(),
                    LLAMA_PROMPT_MAX_CHARS,
                    p.used,
                    p.used + p.dropped,
                    p.dropped,
                    p.text
                );
            } else {
                debug!(
                    "[ASR] llama-asr prompt 偏置 {}/{} 字符（保留 {}/{} 条热词）: {}",
                    p.text.chars().count(),
                    LLAMA_PROMPT_MAX_CHARS,
                    p.used,
                    p.used + p.dropped,
                    p.text
                );
            }
            form = form.text("prompt", p.text);
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
        let text = parse_llama_text(&body_text).ok_or_else(|| AsrError::ProviderApiError {
            provider: Self::ID.into(),
            message: format!("无法从响应中提取文本: {body_text}"),
        })?;

        Ok(AsrResult {
            text,
            confidence: None,
            provider_id: Self::ID.into(),
        })
    }
    /// 结果流式识别：整段 WAV 上传 + SSE 增量 partial（`on_partial` 回调，
    /// 事件发射由调用方负责）→ final。复用整句的端点/模型/热词选择逻辑。
    #[instrument(skip(self, wav_bytes, opts, on_partial), fields(provider = Self::ID))]
    async fn stream_recognize(
        &self,
        wav_bytes: Vec<u8>,
        opts: &AsrOptions,
        on_partial: Option<Arc<dyn for<'a> Fn(&'a str) + Send + Sync + 'static>>,
    ) -> Result<AsrResult, AsrError> {
        let prompt = llama_prompt_from_hotwords(&opts.hotwords);
        if let Some(p) = &prompt {
            if p.dropped > 0 {
                debug!(
                    "[ASR] llama-asr 流式 prompt 偏置 {}/{} 字符（保留 {}/{} 条热词，{} 条超预算未发送）",
                    p.text.chars().count(),
                    LLAMA_PROMPT_MAX_CHARS,
                    p.used,
                    p.used + p.dropped,
                    p.dropped
                );
            }
        }
        crate::ai_service::asr::provider_stream_llama::recognize_stream(
            &self.http,
            &self.cred,
            &self.effective_endpoint(),
            &self.effective_model(),
            prompt.map(|p| p.text),
            wav_bytes,
            on_partial,
        )
        .await
    }
}

/// 热词文本的总字符上限。
/// 部署文档建议按条数限制在 20~30 个，当前按字符计。
const LLAMA_PROMPT_MAX_CHARS: usize = 200;

/// 扣除前缀/后缀后的实际字符预算。
pub(crate) fn llama_prompt_capacity() -> usize {
    LLAMA_PROMPT_MAX_CHARS
        .saturating_sub(PROMPT_PREFIX.chars().count() + PROMPT_SUFFIX.chars().count())
}

const PROMPT_PREFIX: &str = "热词：";
const PROMPT_SUFFIX: &str = "。";

/// 一次 prompt 偏置的构造结果。
pub(crate) struct LlamaPrompt {
    /// 实际写进 multipart `prompt` 字段的文本。
    pub text: String,
    /// 拼进 `text` 的词数。
    pub used: usize,
    /// 因超出字符预算而未发的词数（整词丢弃、丢尾部）。
    pub dropped: usize,
}

/// 热词 → llama-server 的 `prompt` 偏置文本。
///
/// `prompt` 是 `/v1/audio/transcriptions` 唯一的热词通道，没有结构化的
/// `parameters.vocabulary`，只能把词放进前文上下文。发送前做去重与长度上限。
///
/// 音频过短或无有效人声时，返回值可能被模型当成转录原样回吐，调用方需自行校验。
fn llama_prompt_from_hotwords(hotwords: &[Hotword]) -> Option<LlamaPrompt> {
    let mut seen = std::collections::HashSet::new();
    let mut words: Vec<&str> = Vec::new();
    for h in hotwords {
        let t = h.text.trim();
        if t.is_empty() {
            continue;
        }
        if seen.insert(t) {
            words.push(t);
        }
    }
    if words.is_empty() {
        return None;
    }
    let total = words.len();
    let budget = llama_prompt_capacity();
    let mut body = String::new();
    let mut used = 0usize;
    for w in words {
        let sep = if body.is_empty() { "" } else { "、" };
        let extra = sep.chars().count() + w.chars().count();
        if body.chars().count() + extra > budget {
            break;
        }
        body.push_str(sep);
        body.push_str(w);
        used += 1;
    }
    if body.is_empty() {
        return None;
    }
    Some(LlamaPrompt {
        text: format!("{PROMPT_PREFIX}{body}{PROMPT_SUFFIX}"),
        used,
        dropped: total - used,
    })
}

/// 解析 llama-server 转写响应文本。
///
/// 实测格式 `{"text": "language <lang><asr_text><转写文本>"}`，切 `<asr_text>`
/// 取文本并丢弃语言标记，无语音时文本为空。整句识别与 SSE 流式共用。
pub(crate) fn parse_llama_text(body: &str) -> Option<String> {
    let value: JsonValue = serde_json::from_str(body).ok()?;
    let text = value.get("text").and_then(|t| t.as_str())?;
    match text.split_once("<asr_text>") {
        Some((_, content)) => Some(content.to_string()),
        None => Some(text.to_string()),
    }
}

/// 解析 llama-server `/v1/models` 响应，提取模型全名列表。
///
/// 兼容两种结构：`data[].id`（OpenAI 兼容）与 `models[].name`。
pub(crate) fn parse_llama_models(body: &str) -> Vec<String> {
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
