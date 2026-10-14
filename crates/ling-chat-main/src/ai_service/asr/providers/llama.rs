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
        // 记账用字符口径：llama 的容量是**字符预算**，不是条数（D44/CapacityD45）
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
///
/// ⚠️ 口径存疑：部署包文档 `API接入文档.md` 的建议是**按条数** ≤ 20~30 个
/// （「过多会稀释偏置效果」），而这里按字符 200——中文词 2~8 字，200 字符可塞
/// 25~100 个词，容易远超文档建议的条数。修复时一并改按条数。
const LLAMA_PROMPT_MAX_CHARS: usize = 200;

/// 实现实际使用的预算（**测试断言与实现读同一个函数**，不写死 196）。
///
/// ⚠️ 这个值会随 prefix/suffix 变化——见 `llama_prompt_from_hotwords` 的 TODO，
/// 那次回归修复会把它们删掉、预算随之变成满额。断言写死数字会在那时失效。
pub(crate) fn llama_prompt_capacity() -> usize {
    LLAMA_PROMPT_MAX_CHARS
        .saturating_sub(PROMPT_PREFIX.chars().count() + PROMPT_SUFFIX.chars().count())
}

/// ⚠️ 这两行是 `4ca2e79c` 引入的回归点（见函数文档 TODO）：裸逗号列表才是
/// 部署包文档验证过的形式。修的时候连 prefix/suffix 一起去掉，并同步预算算法。
const PROMPT_PREFIX: &str = "热词：";
const PROMPT_SUFFIX: &str = "。";

/// 一次 prompt 偏置的构造结果：文本 + **保留与丢弃的计数**（D45 的记账）。
pub(crate) struct LlamaPrompt {
    /// 实际写进 multipart `prompt` 字段的文本。
    pub text: String,
    /// 拼进 `text` 的词数。
    pub used: usize,
    /// 因超出字符预算而未发的词数（整词丢弃、丢尾部，D43）。
    pub dropped: usize,
}

/// 热词 → llama-server 的 `prompt` 偏置文本。
///
/// `prompt` 是 llama-server `/v1/audio/transcriptions` **唯一**的热词通道——
/// 它没有 qwen 那种结构化的 `parameters.vocabulary`，只能把词塞进前文上下文。
///
/// 保留的两项处理（原实现直接 `join(", ")`，都没有）：
/// - **去重**：同一角色可能重复配置，重复词只会挤占窗口
/// - **长度上限**：见上，口径待改
///
/// ⚠️ **TODO（2026-10-09 实测发现，待修复）**
///
/// **现象**：音频过短 / 无有效人声时，模型把这段 prompt 当成已完成的转录直接收尾，
/// **整段 `r.text` 与返回值逐字相同**，用户拿到的是词表而不是说的话
/// （日志形如 `[ASR] llama-asr 识别结果: 热词：A、B。`）。
///
/// **定性：这是一次回归，不是本形式的固有风险。** 最初实现（`01b047e9`）用的是
/// `hotwords.join(", ")` 裸逗号列表——与部署包文档的 curl / Rust 示例逐字相同，
/// 且文档有对照实验与 `input_tokens` 证据（`API接入文档.md` §三「动态热词（已验证）」，
/// 并明确「无需 `keywords:` 前缀，实测两种都生效」）。
/// `4ca2e79c` 把它改成 `热词：A、B、C。`，依据是「裸列表容易被当成待续的转录文本」——
/// **该依据与文档实测相反**：裸列表恰恰是验证过有效的形式，而中文成句更像一句待续的
/// 转录文本，正是现在被整体回吐的那串字符。
///
/// **修复方向（首选第 1 条，其余视情况）**：
/// 1. 回到文档验证过的**裸逗号列表**，保留去重与长度上限（去重/上限是 `4ca2e79c`
///    的正当收益，不随形式一起回退）；上限口径同时改按条数
/// 2. 在 `recognize` / `stream_recognize` 加退化兜底：`r.text` 与本函数返回值相等时
///    判为无结果（防线，不替代第 1 条）
/// 3. 下策：不发 `prompt`——等于放弃 llama-asr 的全部热词能力
///
/// **修复前请先确认调试期的观测能力不被一并删掉**——退化时这段词表是当前唯一能直接
/// 看到「模型实际收到哪些热词」的窗口，配合调用点那行 debug 日志一起用于排查。
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
/// 实测格式：`{"text": "language <lang><asr_text><转写文本>"}`（无语音时
/// `<lang>` 为 `None`、文本为空）。切 `<asr_text>` 取后半的文本；前段的
/// `language <lang>` 标记一并丢弃。供整句识别与 SSE 结果流式
///（provider_stream_llama.rs）共用。
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llama_effective_endpoint_falls_back_when_empty() {
        // 空 endpoint → 默认 127.0.0.1:8080（否则拼出相对 URL，reqwest builder error）
        let p = LlamaAsrProvider::new(
            reqwest::Client::new(),
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
            reqwest::Client::new(),
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
            reqwest::Client::new(),
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
        let text = parse_llama_text(body).expect("应解析成功");
        assert_eq!(text, "甚至出现交易几乎停滞的情况。");
    }

    #[test]
    fn parse_llama_text_no_speech_is_empty() {
        // 无语音：`<asr_text>` 标记后为空
        let body = r#"{"text":"language None<asr_text>"}"#;
        assert_eq!(parse_llama_text(body).expect("应解析成功"), "");
    }

    #[test]
    fn parse_llama_text_plain_text_fallback() {
        // 无 <asr_text> 标记（协议异常兜底）：整体当文本
        let body = r#"{"text":"你好"}"#;
        assert_eq!(parse_llama_text(body).expect("应解析成功"), "你好");
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
