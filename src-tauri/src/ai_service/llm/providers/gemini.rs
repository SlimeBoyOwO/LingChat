//! Google Gemini provider.
//!
//! 对标 Python `GeminiProvider`。使用原生 Gemini REST API，而非 OpenAI 兼容格式。
//!
//! 关键差异：
//! - 端点：`{base}/models/{model}:generateContent` / `:streamGenerateContent?alt=sse`
//! - 认证：API key 作为 `?key=` 查询参数
//! - 消息格式：`{role, parts: [{text}]}`，system → `systemInstruction`
//! - 响应路径：`candidates[0].content.parts[*].text`

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::ai_service::llm::provider::LlmProvider;
use crate::ai_service::llm::{ChunkStream, LlmConfig};
use crate::ai_service::types::LlmMessage;

pub struct GeminiProvider {
    model: String,
    api_key: String,
    base_url: String,
    temperature: Option<f64>,
    top_p: Option<f64>,
    enable_thinking: bool,
}

impl GeminiProvider {
    pub fn from_config(cfg: &LlmConfig) -> Result<Self> {
        let base_url = if cfg.base_url.is_empty() {
            "https://generativelanguage.googleapis.com/v1beta".to_string()
        } else {
            cfg.base_url.trim_end_matches('/').to_string()
        };
        Ok(Self {
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone(),
            base_url,
            temperature: cfg.temperature,
            top_p: cfg.top_p,
            enable_thinking: cfg.enable_thinking,
        })
    }

    fn endpoint(&self, stream: bool) -> String {
        let action = if stream {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        format!(
            "{}models/{}:{}?key={}",
            self.base_url,
            self.model,
            action,
            self.api_key,
        )
    }

    /// 将 OpenAI-format 消息转换为 Gemini 的 `contents` + `systemInstruction` 格式。
    fn convert_messages(&self, messages: &[LlmMessage]) -> (Vec<GeminiContent>, Option<GeminiSystemInstruction>) {
        let mut contents: Vec<GeminiContent> = Vec::new();
        let mut system_parts: Vec<GeminiTextPart> = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system_parts.push(GeminiTextPart {
                        text: msg.content.clone(),
                    });
                }
                _ => {
                    let role = if msg.role == "assistant" {
                        "model".to_string()
                    } else {
                        msg.role.clone()
                    };
                    contents.push(GeminiContent {
                        role,
                        parts: vec![GeminiPart::Text(GeminiTextPart {
                            text: msg.content.clone(),
                        })],
                    });
                }
            }
        }

        let system_instruction = if system_parts.is_empty() {
            None
        } else {
            Some(GeminiSystemInstruction {
                parts: GeminiSystemParts { parts: system_parts },
            })
        };

        (contents, system_instruction)
    }

    fn build_generation_config(&self) -> GeminiGenerationConfig {
        let thinking_config = if self.enable_thinking {
            Some(GeminiThinkingConfig {
                thinking_budget: 1024,
            })
        } else {
            None
        };
        GeminiGenerationConfig {
            temperature: self.temperature,
            top_p: self.top_p,
            thinking_config,
        }
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn complete(&self, http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let (contents, system_instruction) = self.convert_messages(messages);

        let generation_config = self.build_generation_config();

        let body = GeminiRequest {
            contents: &contents,
            system_instruction: system_instruction.as_ref(),
            generation_config: Some(generation_config),
        };

        let resp = http
            .post(self.endpoint(false))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Gemini 请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Gemini 调用失败 ({status}): {text}"));
        }

        let parsed: GeminiResponse = resp
            .json()
            .await
            .context("解析 Gemini 响应 JSON 失败")?;

        let text: String = parsed
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Gemini 响应无候选"))?
            .content
            .parts
            .into_iter()
            .filter_map(|p| match p {
                GeminiResponsePart::Text(t) => Some(t.text),
            })
            .collect();

        if text.is_empty() {
            return Err(anyhow!("Gemini 响应无文本内容"));
        }
        Ok(text)
    }

    async fn complete_stream(
        &self,
        http: &Client,
        messages: &[LlmMessage],
    ) -> Result<ChunkStream> {
        let (contents, system_instruction) = self.convert_messages(messages);

        let generation_config = self.build_generation_config();

        let body = GeminiRequest {
            contents: &contents,
            system_instruction: system_instruction.as_ref(),
            generation_config: Some(generation_config),
        };

        // 流式端点需要加 `&alt=sse`
        let mut url = self.endpoint(true);
        url.push_str("&alt=sse");

        let resp = http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Gemini 流式请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Gemini 流式调用失败 ({status}): {text}"));
        }

        let byte_stream = resp.bytes_stream();
        let stream = async_stream::try_stream! {
            let mut pending = String::new();
            let mut bs = byte_stream;
            while let Some(item) = bs.next().await {
                let chunk = item.map_err(|e| anyhow!("Gemini 流式读取失败: {e}"))?;
                let text = String::from_utf8_lossy(&chunk).to_string();
                pending.push_str(&text);

                loop {
                    let sep = pending.find("\n\n").or_else(|| pending.find("\r\n\r\n"));
                    let Some(pos) = sep else { break };
                    let seplen = if pending[pos..].starts_with("\n\n") { 2 } else { 4 };
                    let event = pending[..pos].to_string();
                    pending.drain(..pos + seplen);

                    for raw_line in event.lines() {
                        let line = raw_line.trim_start();
                        let Some(data) = line.strip_prefix("data:") else { continue };
                        let data = data.trim();
                        if data.is_empty() { continue; }

                        let parsed: GeminiStreamChunk = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        for candidate in parsed.candidates.into_iter() {
                            for part in candidate.content.parts.into_iter() {
                                match part {
                                    GeminiResponsePart::Text(t) => {
                                        if !t.text.is_empty() {
                                            yield t.text;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

// ============================================================
// Gemini JSON types
// ============================================================

#[derive(Serialize)]
struct GeminiRequest<'a> {
    contents: &'a [GeminiContent],
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<&'a GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum GeminiPart {
    Text(GeminiTextPart),
}

#[derive(Serialize, Deserialize, Clone)]
struct GeminiTextPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: GeminiSystemParts,
}

#[derive(Serialize)]
struct GeminiSystemParts {
    parts: Vec<GeminiTextPart>,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_config: Option<GeminiThinkingConfig>,
}

#[derive(Serialize)]
struct GeminiThinkingConfig {
    #[serde(rename = "thinkingBudget")]
    thinking_budget: i32,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum GeminiResponsePart {
    Text(GeminiTextPart),
}

/// 流式 chunk：Gemini SSE 返回的 JSON 结构与非流式相似，但 candidates 可能不完整。
#[derive(Deserialize)]
struct GeminiStreamChunk {
    candidates: Vec<GeminiStreamCandidate>,
}

#[derive(Deserialize)]
struct GeminiStreamCandidate {
    content: GeminiStreamContent,
}

#[derive(Deserialize)]
struct GeminiStreamContent {
    parts: Vec<GeminiResponsePart>,
}
