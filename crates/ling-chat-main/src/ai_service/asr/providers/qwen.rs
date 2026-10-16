//! Qwen ASR（阿里云 DashScope）provider 实现。
//!
//! 非流式走 `multimodal-generation` 端点；WS 流式经
//! `crate::ai_service::asr::provider_stream`。模型能力查询在
//! `crate::ai_service::asr::qwen_models`。

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use serde_json::{Value as JsonValue, json};
use tracing::{instrument, warn};

use crate::ai_service::asr::config_fields::qwen_asr_config_fields;
use crate::ai_service::asr::error::{AsrError, map_reqwest_error};
use crate::ai_service::asr::provider::AsrProvider;
use crate::ai_service::asr::provider_meta::AsrConfigField;
use crate::ai_service::asr::provider_types::{AsrOptions, AsrResult, Hotword, ProviderCredentials};
use crate::ai_service::asr::qwen_models::{
    qwen_default_model, qwen_is_streaming_model, qwen_supports_inline_vocabulary,
    qwen_uses_legacy_audio_content,
};

/// Qwen ASR（阿里云 DashScope）。
///
/// 非流式走 `multimodal-generation` 端点（JSON body + base64 音频）；
/// 流式（paraformer-realtime-v2）走 WebSocket 实时端点（provider_stream.rs）。
pub struct QwenAsrProvider {
    http: reqwest::Client,
    cred: ProviderCredentials,
}

impl QwenAsrProvider {
    pub const ID: &'static str = "qwen-asr";
    pub const DISPLAY: &'static str = "Qwen ASR（阿里云百炼）";

    pub fn new(http: reqwest::Client, cred: ProviderCredentials) -> Result<Self, AsrError> {
        if !cred.has_api_key() {
            // 带上地域名：Key 是分地域存的，只说「需要 api_key」无法指出该去哪个
            // 地域的输入框填（切到没配过的地域时最容易撞上）
            return Err(AsrError::MissingCredentials(format!(
                "Qwen ASR 需要 DashScope api_key（当前地域：{}）",
                cred.region_enum().label()
            )));
        }
        Ok(Self { http, cred })
    }

    /// 同步路径生效的模型。
    ///
    /// 配置为空、或配了一个流式模型（同步端点不认识它们，DashScope 会返回
    /// HTTP 400 "url error"）→ 回退到该地域的同步默认模型。
    fn effective_batch_model(&self) -> &str {
        let m = self.cred.model.trim();
        if m.is_empty() || qwen_is_streaming_model(m) {
            qwen_default_model(false, self.cred.region_enum())
        } else {
            m
        }
    }
}

/// 热词列表 → DashScope 即时热词对象 `{"词": 权重}`。
///
/// 同词重复时保留**先出现**的权重（调用方已按优先级排序，见
/// [`AsrOptions::hotwords`] 的覆盖语义）。
pub(crate) fn hotwords_to_vocabulary_json(hotwords: &[Hotword]) -> JsonValue {
    let mut map = serde_json::Map::new();
    for h in hotwords {
        let text = h.text.trim();
        if text.is_empty() {
            continue;
        }
        map.entry(text.to_string())
            .or_insert_with(|| json!(h.weight));
    }
    JsonValue::Object(map)
}

/// 构造同步端点（multimodal-generation）的请求体。
///
/// 该端点的 `parameters` 只有 format / sample_rate / vocabulary（无 language_hints）。
/// 热词在此门控：仅 qwen-audio-3.x 系支持 `vocabulary`，对其余模型发它很可能直接 400。
fn build_batch_body(model: &str, data_url: &str, hotwords: &[Hotword]) -> JsonValue {
    // 音频内容格式按模型分流：qwen-audio-3.x 系用 input_audio；历史 Fun-ASR-Realtime
    // 用旧格式 audio。一刀切会让老配置从「能用」变成「未知错误」。
    let legacy = qwen_uses_legacy_audio_content(model);
    let audio_content = if legacy {
        json!({ "audio": data_url })
    } else {
        json!({ "type": "input_audio", "input_audio": { "data": data_url } })
    };
    let mut body = json!({
        "model": model,
        "input": {
            "messages": [{
                "role": "user",
                "content": [audio_content]
            }]
        },
        "parameters": {
            "format": "wav",
            "sample_rate": 16000
        }
    });
    if legacy {
        // 旧协议实测带这个字段；新协议按官方示例不带
        body["resources"] = json!([]);
    }
    if !hotwords.is_empty() {
        if qwen_supports_inline_vocabulary(model) {
            body["parameters"]["vocabulary"] = hotwords_to_vocabulary_json(hotwords);
        } else {
            warn!(
                "[ASR] 模型 {model} 不支持即时热词，已忽略 {} 条热词（如需热词请改用 qwen-audio-3.x 系）",
                hotwords.len()
            );
        }
    }
    body
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

    #[instrument(skip(self, wav_bytes, opts), fields(provider = Self::ID))]
    async fn recognize(
        &self,
        wav_bytes: Vec<u8>,
        opts: &AsrOptions,
    ) -> Result<AsrResult, AsrError> {
        let endpoint = self.cred.effective_http_endpoint();
        let model = self.effective_batch_model();
        let b64 = BASE64_STD.encode(&wav_bytes);
        let data_url = format!("data:audio/wav;base64,{b64}");
        let body = build_batch_body(model, &data_url, &opts.hotwords);

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
            // 同步端点既不返回语言也不接受 language_hints：回填 hint 会伪造一个没人检测过的值
            language: None,
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
