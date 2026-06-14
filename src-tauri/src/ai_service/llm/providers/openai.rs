//! OpenAI-compatible provider (OpenAI / DeepSeek / 通义千问 / etc.)
//!
//! 对标 Python `WebLLMProvider`。使用标准 `/v1/chat/completions` SSE 协议。

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::ai_service::llm::provider::{LlmProvider, LlmResponseWithTools};
use crate::ai_service::llm::{ChunkStream, LlmConfig};
use crate::ai_service::types::{LlmMessage, ToolCall, ToolDefinition};

pub struct OpenAiProvider {
    model: String,
    api_key: String,
    base_url: String,
    temperature: Option<f64>,
    top_p: Option<f64>,
    enable_thinking: bool,
}

impl OpenAiProvider {
    pub fn from_config(cfg: &LlmConfig) -> Result<Self> {
        tracing::info!(
            "[OpenAI] from_config: enable_thinking={}, model={}",
            cfg.enable_thinking,
            cfg.model
        );
        Ok(Self {
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone(),
            base_url: cfg.base_url.clone(),
            temperature: cfg.temperature,
            top_p: cfg.top_p,
            enable_thinking: cfg.enable_thinking,
        })
    }

    fn endpoint(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        if base.is_empty() {
            "https://api.openai.com/v1/chat/completions".to_string()
        } else if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{base}/chat/completions")
        }
    }

    fn build_request<'a>(
        &'a self,
        messages: &'a [LlmMessage],
        stream: bool,
        tools: Option<&'a [ToolDefinition]>,
        tool_choice: Option<serde_json::Value>,
    ) -> ChatRequest<'a> {
        // DeepSeek reasoner 等模型在 thinking 字段缺失时默认启用思考模式，
        // 因此必须显式发送 "disabled" 才能真正关闭。
        let thinking = if self.enable_thinking {
            tracing::info!("[OpenAI] build_request: thinking=enabled");
            ThinkingConfig {
                type_: "enabled".to_string(),
            }
        } else {
            tracing::info!("[OpenAI] build_request: thinking=disabled");
            ThinkingConfig {
                type_: "disabled".to_string(),
            }
        };
        ChatRequest {
            model: &self.model,
            messages,
            stream,
            temperature: self.temperature,
            top_p: self.top_p,
            thinking,
            tools,
            tool_choice,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let body = self.build_request(messages, false, None, None);

        let resp = http
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("LLM 请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("LLM 非流式调用失败 ({status}): {text}"));
        }

        let parsed: ChatCompletionResponse =
            resp.json().await.context("解析 LLM 响应 JSON 失败")?;
        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| anyhow!("LLM 响应无可用内容"))
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
                // 尝试解析为 JSON object（如 {"type":"function","function":{"name":"xxx"}}）
                serde_json::from_str(tc).unwrap_or(serde_json::Value::String("auto".to_string()))
            }
        });

        let body = self.build_request(messages, false, Some(tools), tool_choice_value);

        let resp = http
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("LLM (tools) 请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("LLM function calling 失败 ({status}): {text}"));
        }

        let parsed: ChatCompletionResponse =
            resp.json().await.context("解析 LLM (tools) 响应 JSON 失败")?;

        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("LLM (tools) 响应无可用选项"))?;

        Ok(LlmResponseWithTools {
            content: choice.message.content,
            tool_calls: choice.message.tool_calls,
        })
    }

    async fn complete_stream(&self, http: &Client, messages: &[LlmMessage]) -> Result<ChunkStream> {
        let body = self.build_request(messages, true, None, None);
        tracing::info!(
            "[OpenAI] complete_stream: thinking 字段 = {}",
            body.thinking.type_
        );
        let resp = http
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("LLM 流式请求发送失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("LLM 流式调用失败 ({status}): {text}"));
        }

        let byte_stream = resp.bytes_stream();
        let stream = async_stream::try_stream! {
            let mut pending = String::new();
            let mut bs = byte_stream;
            while let Some(item) = bs.next().await {
                let chunk = item.map_err(|e| anyhow!("LLM 流式读取失败: {e}"))?;
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
                        if data == "[DONE]" { return; }
                        if data.is_empty() { continue; }
                        let parsed: ChatStreamChunk = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if let Some(choice) = parsed.choices.into_iter().next() {
                            // 思考模式内容（reasoning_content）：仅记录日志，不输出
                            if let Some(reasoning) = choice.delta.reasoning_content {
                                if !reasoning.is_empty() {
                                    tracing::info!("[LLM Thinking] {}", reasoning);
                                }
                            }
                            if let Some(content) = choice.delta.content {
                                if !content.is_empty() {
                                    yield content;
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
// HTTP payload types (OpenAI format)
// ============================================================

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [LlmMessage],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    thinking: ThinkingConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageContent,
}

#[derive(Deserialize)]
struct ChatMessageContent {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Deserialize)]
struct ChatStreamChunk {
    choices: Vec<ChatStreamChoice>,
}

#[derive(Deserialize)]
struct ChatStreamChoice {
    delta: ChatStreamDelta,
}

#[derive(Deserialize, Default)]
struct ChatStreamDelta {
    #[serde(default)]
    content: Option<String>,
    /// 思考模式内容（DeepSeek R1 等模型的 reasoning_content）。
    #[serde(default)]
    reasoning_content: Option<String>,
}
