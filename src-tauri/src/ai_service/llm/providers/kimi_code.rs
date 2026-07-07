//! Kimi-Code provider adapter.
//!
//! 参考 AstrBot 的 `kimi_code_source.py`：复用 Anthropic Messages API 协议，
//! 固定 base_url 为 https://api.kimi.com/coding，默认模型 kimi-for-coding，
//! 并强制携带 User-Agent: claude-code/0.1.0。

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::ai_service::llm::provider::{LlmProvider, LlmResponseWithTools};
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmConfig};
use crate::ai_service::types::{LlmMessage, ToolCall, ToolDefinition};

pub struct KimiCodeProvider {
    model: String,
    api_key: String,
    base_url: String,
    temperature: Option<f64>,
    top_p: Option<f64>,
    enable_thinking: bool,
}

impl KimiCodeProvider {
    pub fn from_config(cfg: &LlmConfig) -> Result<Self> {
        let base_url = if cfg.base_url.trim().is_empty() {
            "https://api.kimi.com/coding".to_string()
        } else {
            cfg.base_url.trim_end_matches('/').to_string()
        };
        let model = if cfg.model.trim().is_empty() {
            "kimi-for-coding".to_string()
        } else {
            cfg.model.clone()
        };
        tracing::info!("[KimiCode] from_config: model={}", model);
        Ok(Self {
            model,
            api_key: cfg.api_key.clone(),
            base_url,
            temperature: cfg.temperature,
            top_p: cfg.top_p,
            enable_thinking: cfg.enable_thinking,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    fn headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("claude-code/0.1.0"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.api_key)
                .context("Kimi-Code API key 包含非法字符")?,
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static("2023-06-01"),
        );
        Ok(headers)
    }

    fn build_request<'a>(
        &'a self,
        messages: &'a [LlmMessage],
        stream: bool,
        tools: Option<&'a [ToolDefinition]>,
        tool_choice: Option<serde_json::Value>,
    ) -> MessagesRequest<'a> {
        // 拆分 system 与对话消息
        let mut system_text = String::new();
        let mut conversation: Vec<AnthropicMessage> = Vec::new();
        for m in messages {
            match m.role.as_str() {
                "system" => {
                    if !system_text.is_empty() {
                        system_text.push('\n');
                    }
                    system_text.push_str(&m.content);
                }
                "user" => conversation.push(AnthropicMessage {
                    role: "user",
                    content: &m.content,
                }),
                "assistant" | "tool" => conversation.push(AnthropicMessage {
                    role: "assistant",
                    content: &m.content,
                }),
                _ => conversation.push(AnthropicMessage {
                    role: "user",
                    content: &m.content,
                }),
            }
        }

        MessagesRequest {
            model: &self.model,
            max_tokens: 65536,
            stream,
            temperature: self.temperature,
            top_p: self.top_p,
            system: if system_text.is_empty() { None } else { Some(system_text) },
            messages: conversation,
            tools,
            tool_choice,
            thinking: if self.enable_thinking {
                Some(ThinkingConfig {
                    type_: "enabled".to_string(),
                })
            } else {
                Some(ThinkingConfig {
                    type_: "disabled".to_string(),
                })
            },
        }
    }

    fn parse_messages_response(&self, parsed: MessagesResponse) -> Result<String> {
        let mut text = String::new();
        for block in parsed.content {
            if let Some(t) = block.text {
                text.push_str(&t);
            }
        }
        if text.is_empty() {
            return Err(anyhow!("Kimi-Code 响应无可用文本内容"));
        }
        Ok(text)
    }
}

#[async_trait]
impl LlmProvider for KimiCodeProvider {
    async fn complete(&self, http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let body = self.build_request(messages, false, None, None);
        let resp = http
            .post(self.endpoint())
            .headers(self.headers()?)
            .json(&body)
            .send()
            .await
            .context("Kimi-Code 请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Kimi-Code 非流式调用失败 ({status}): {text}"));
        }

        let parsed: MessagesResponse = resp.json().await.context("解析 Kimi-Code 响应 JSON 失败")?;
        self.parse_messages_response(parsed)
    }

    async fn complete_with_tools(
        &self,
        http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<LlmResponseWithTools> {
        let tool_choice_value = tool_choice.map(|tc| {
            if tc == "auto" || tc == "none" || tc == "required" {
                serde_json::Value::String(tc.to_string())
            } else {
                serde_json::from_str(tc).unwrap_or(serde_json::Value::String("auto".to_string()))
            }
        });

        let body = self.build_request(messages, false, Some(tools), tool_choice_value);
        let resp = http
            .post(self.endpoint())
            .headers(self.headers()?)
            .json(&body)
            .send()
            .await
            .context("Kimi-Code (tools) 请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Kimi-Code function calling 失败 ({status}): {text}"));
        }

        let parsed: MessagesResponse = resp
            .json()
            .await
            .context("解析 Kimi-Code (tools) 响应 JSON 失败")?;

        let mut content_text = String::new();
        let mut tool_calls: Option<Vec<ToolCall>> = None;
        for block in parsed.content {
            if let Some(t) = block.text {
                content_text.push_str(&t);
            }
            if block.type_ == "tool_use" {
                if let (Some(id), Some(name), Some(input)) = (block.id, block.name, block.input) {
                    let args = input.to_string();
                    let tc = ToolCall {
                        id,
                        type_: "function".to_string(),
                        function: crate::ai_service::types::FunctionCall { name, arguments: args },
                    };
                    tool_calls.get_or_insert_with(Vec::new).push(tc);
                }
            }
        }

        Ok(LlmResponseWithTools {
            content: if content_text.is_empty() { None } else { Some(content_text) },
            tool_calls,
        })
    }

    async fn complete_stream(&self, http: &Client, messages: &[LlmMessage]) -> Result<ChunkStream> {
        let body = self.build_request(messages, true, None, None);
        let resp = http
            .post(self.endpoint())
            .headers(self.headers()?)
            .json(&body)
            .send()
            .await
            .context("Kimi-Code 流式请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Kimi-Code 流式调用失败 ({status}): {text}"));
        }

        let byte_stream = resp.bytes_stream();
        let stream = async_stream::try_stream! {
            let mut pending = String::new();
            let mut thinking_buffer = String::new();
            let mut text_buffer = String::new();
            let mut last_flush_len: usize = 0;
            let mut bs = byte_stream;
            while let Some(item) = bs.next().await {
                let chunk = item.map_err(|e| anyhow!("Kimi-Code 流式读取失败: {e}"))?;
                pending.push_str(&String::from_utf8_lossy(&chunk));

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
                        if data == "[DONE]" {
                            // 流式响应结束前输出剩余的 thinking 内容
                            if !thinking_buffer.is_empty() {
                                tracing::info!("[Kimi-Code Thinking] {}", thinking_buffer);
                                yield LlmChunk::Reasoning(thinking_buffer.clone());
                                thinking_buffer.clear();
                                last_flush_len = 0;
                            }
                            // 如果 text 为空但 thinking 有内容，把 thinking 作为正式回复兜底
                            if text_buffer.is_empty() && !thinking_buffer.is_empty() {
                                tracing::info!("[Kimi-Code] text 为空，使用 thinking 作为回复");
                                for line in thinking_buffer.lines() {
                                    yield LlmChunk::Content(line.to_string());
                                }
                            }
                            return;
                        }
                        if data.is_empty() { continue; }
                        let parsed: MessagesStreamChunk = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::debug!("[Kimi-Code] 无法解析 SSE 数据: {e}, data={data}");
                                continue;
                            }
                        };
                        match parsed.type_.as_str() {
                            "content_block_delta" => {
                                if let Some(delta) = parsed.delta {
                                    if let Some(t) = delta.text {
                                        if !t.is_empty() {
                                            text_buffer.push_str(&t);
                                            yield LlmChunk::Content(t);
                                        }
                                    }
                                    if let Some(thinking) = delta.thinking {
                                        if !thinking.is_empty() {
                                            thinking_buffer.push_str(&thinking);
                                        }
                                    }
                                }
                            }
                            "content_block_start" | "content_block_stop" | "message_start" | "message_delta" | "message_stop" => {
                                tracing::debug!("[Kimi-Code SSE] type={}, delta={:?}", parsed.type_, parsed.delta);
                            }
                            other => {
                                tracing::debug!("[Kimi-Code SSE] 未处理的事件类型: {other}");
                            }
                        }
                    }
                }

                // 每次处理完 chunk 后，如果 thinking 累计新增了一定长度，就输出增量部分
                if thinking_buffer.len() > last_flush_len && thinking_buffer.len() - last_flush_len >= 60 {
                    let delta = &thinking_buffer[last_flush_len..];
                    if !delta.is_empty() {
                        yield LlmChunk::Reasoning(delta.to_string());
                    }
                    last_flush_len = thinking_buffer.len();
                }
            }
            // 流正常结束时也输出未打印的 thinking
            if !thinking_buffer.is_empty() {
                tracing::info!("[Kimi-Code Thinking] {}", thinking_buffer);
                yield LlmChunk::Reasoning(thinking_buffer.clone());
            }
            // 兜底：text 为空时使用 thinking
            if text_buffer.is_empty() && !thinking_buffer.is_empty() {
                tracing::info!("[Kimi-Code] text 为空，使用 thinking 作为回复");
                for line in thinking_buffer.lines() {
                    yield LlmChunk::Content(line.to_string());
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

// ============================================================
// Anthropic Messages API payload types
// ============================================================

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize, Default)]
struct ContentBlock {
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct MessagesStreamChunk {
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    delta: Option<MessageDelta>,
}

#[derive(Deserialize, Default, Debug)]
struct MessageDelta {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
}
