//! 视觉模型（VLM）调用。
//!
//! 集中所有「把图片/视频交给视觉模型识别」的请求逻辑，替代原先散落在
//! `screen_analyzer` 与 `tools/read_media_file` 里的两套手写 HTTP：
//! - 图片走 genai（OpenAI 兼容 adapter），与项目其余 LLM 调用统一；
//! - 视频保留裸 reqwest（genai 0.6.5 不支持 `video_url`），但代码位置也收敛到这里。
//!
//! 说明：视觉请求固定使用 OpenAI 兼容协议 + `vision_base_url` 改写后的端点，
//! 不经过 `GenaiProvider`——后者会无条件下发 chat 专属的 `thinking` 字段与
//! reasoning 剥名逻辑，强加到任意视觉端点上会有回归风险。

use anyhow::{Result, anyhow};
use base64::Engine as _;
use genai::Client as GenaiClient;
use genai::ServiceTarget;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ContentPart, MessageContent};
use genai::resolver::{AuthData, Endpoint};
use reqwest::Client;
use serde_json::Value;

use crate::ai_service::llm::provider_config::LlmProviderConfig;
use crate::ai_service::llm::providers::normalize_base_url;

/// 一次视觉识别的结果（文本 + 用量）。
pub struct VisionResult {
    pub text: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// 视觉请求的目标端点（`base_url` 已按视觉协议改写为 OpenAI 兼容端点）。
///
/// 与 `LlmProviderConfig` 区分：这里的 `base_url` 已经过 [`vision_base_url`]
/// 改写（如 Kimi Code 的 Anthropic 兼容聊天入口会换成其 OpenAI 兼容入口）。
pub struct VisionTarget {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl VisionTarget {
    pub fn from_provider(provider: &LlmProviderConfig) -> Self {
        Self {
            api_key: provider.api_key.clone(),
            base_url: vision_base_url(provider),
            model: provider.model.clone(),
        }
    }
}

/// 将 provider 的 base_url 适配为视觉分析使用的 OpenAI 兼容端点前缀
/// （请求时拼接 `{base}/chat/completions`）。
/// Kimi Code 的聊天入口是 Anthropic 兼容协议，视觉请求需要改用
/// 官方提供的 OpenAI 兼容入口。
pub fn vision_base_url(provider: &LlmProviderConfig) -> String {
    let base = provider.base_url.trim().trim_end_matches('/');
    match provider.provider.as_str() {
        "kimicode" => {
            if base.is_empty() {
                "https://api.kimi.com/coding/v1".to_string()
            } else if base.ends_with("/v1/messages") {
                base.trim_end_matches("/messages").to_string()
            } else if base.ends_with("/v1/chat/completions") {
                base.trim_end_matches("/chat/completions").to_string()
            } else if base.ends_with("/v1") {
                base.to_string()
            } else {
                format!("{base}/v1")
            }
        },
        "openai" if base.is_empty() => "https://api.openai.com/v1".to_string(),
        "deepseek" if base.is_empty() => "https://api.deepseek.com".to_string(),
        _ => base.to_string(),
    }
}

/// 图片识别：走 genai（固定 OpenAI 兼容 adapter）。
///
/// `mime` 为完整 MIME 类型（如 `image/jpeg`）——`ContentPart` 的 `is_image()`
/// 依赖 `image/` 前缀，data URL 也据此拼接。
/// `auto_compress` 为全局开关：图片超过端点硬限制时是否自动压缩（见
/// [`crate::utils::image::clamp_to_inline_limits`]）；关闭则原样发送。
pub async fn analyze_image(
    http: &Client,
    target: &VisionTarget,
    prompt: &str,
    image_bytes: &[u8],
    mime: &str,
    max_tokens: u32,
    auto_compress: bool,
) -> Result<VisionResult> {
    // 端点有 32 MiB / 8192px 的硬限制，开启时超限先缩放重编码，避免整条请求被拒。
    let (image_bytes, mime) =
        crate::utils::image::clamp_to_inline_limits(image_bytes, mime, auto_compress)?;
    let data_url = format!(
        "data:{mime};base64,{}",
        base64::prelude::BASE64_STANDARD.encode(&image_bytes)
    );
    let content = MessageContent::from_parts(vec![
        ContentPart::Text(prompt.to_string()),
        ContentPart::from_binary_url(mime.to_string(), data_url, None),
    ]);
    let chat_req = ChatRequest::new(vec![ChatMessage::user(content)]);

    // genai 的 OpenAI adapter 用 `Url::join("chat/completions")` 拼路径，
    // base_url 必须以 `/` 结尾，否则 `v1` 会被当成文件替换掉（同 #787 的坑）。
    let key = target.api_key.clone();
    let base = normalize_base_url(&target.base_url);
    let client = GenaiClient::builder()
        .with_reqwest(http.clone())
        .with_adapter_kind(AdapterKind::OpenAI)
        .with_auth_resolver_fn(move |_| Ok(Some(AuthData::from_single(key))))
        .with_service_target_resolver_fn(move |mut t: ServiceTarget| {
            t.endpoint = Endpoint::from_owned(base);
            Ok(t)
        })
        .build();

    let opts = ChatOptions::default().with_max_tokens(max_tokens);

    let response = client
        .exec_chat(&target.model, chat_req, Some(&opts))
        .await
        .map_err(|e| anyhow!("视觉模型请求失败: {e}"))?;

    let text = response
        .first_text()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("视觉模型响应中没有可用的文本识别结果"))?;

    Ok(VisionResult {
        text,
        input_tokens: response.usage.prompt_tokens.map(|n| n.max(0) as u32),
        output_tokens: response.usage.completion_tokens.map(|n| n.max(0) as u32),
    })
}

/// 视频识别：genai 0.6.5 无视频支持，保留裸 reqwest（OpenAI 兼容 `video_url`）。
pub async fn analyze_video(
    http: &Client,
    target: &VisionTarget,
    prompt: &str,
    video_bytes: &[u8],
    mime: &str,
    max_tokens: u32,
) -> Result<VisionResult> {
    let data_url = format!(
        "data:{mime};base64,{}",
        base64::prelude::BASE64_STANDARD.encode(video_bytes)
    );
    let payload = serde_json::json!({
        "model": target.model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {"type": "video_url", "video_url": {"url": data_url}}
            ]
        }],
        "max_tokens": max_tokens,
    });

    let endpoint = format!("{}/chat/completions", target.base_url.trim_end_matches('/'));
    let response = http
        .post(endpoint)
        .bearer_auth(&target.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow!("视觉模型请求失败: {e}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| anyhow!("读取视觉模型响应失败: {e}"))?;
    if !status.is_success() {
        let detail: String = body.chars().take(2000).collect();
        return Err(anyhow!(
            "视觉模型返回 HTTP {}: {}；当前视觉模型可能不支持 OpenAI 兼容的 video_url 输入，可关闭视频识别或改用支持视频的视觉模型",
            status.as_u16(),
            detail
        ));
    }
    let value: Value =
        serde_json::from_str(&body).map_err(|e| anyhow!("解析视觉模型响应失败: {e}"))?;
    let text = extract_response_text(&value)
        .ok_or_else(|| anyhow!("视觉模型响应中没有可用的文本识别结果"))?;

    Ok(VisionResult {
        text,
        input_tokens: value["usage"]["prompt_tokens"].as_u64().map(|n| n as u32),
        output_tokens: value["usage"]["completion_tokens"]
            .as_u64()
            .map(|n| n as u32),
    })
}

/// 从 OpenAI 兼容响应中提取文本（`content` 可能是字符串或多模态块数组）。
fn extract_response_text(value: &Value) -> Option<String> {
    let message = value.get("choices")?.get(0)?.get("message")?;
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        let trimmed = content.trim();
        return (!trimmed.is_empty()).then(|| trimmed.to_string());
    }
    let parts = message.get("content")?.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| {
            part.get("text")
                .and_then(Value::as_str)
                .or_else(|| part.get("content").and_then(Value::as_str))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
