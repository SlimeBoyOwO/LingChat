//! OpenAI Codex（ChatGPT 订阅）provider。
//!
//! 协议：Responses API（`POST https://chatgpt.com/backend-api/codex/responses`，SSE 流式）。
//! 凭据来自 `codex_auth`（设备码 OAuth，自动刷新）；链路经 `utils::proxy`
//! 自动探测本地代理。
//!
//! 参考 dsh-codex / pi-ai 的请求规格：
//! - 头：`Authorization: Bearer`、`chatgpt-account-id`、`originator`、
//!   `OpenAI-Beta: responses=experimental`、`accept: text/event-stream`
//! - 体：`store:false, stream:true, instructions, input, include:[reasoning.encrypted_content]`，
//!   `reasoning:{effort, summary:"auto"}`（Default/Off 档整体省略），
//!   Fast Mode（1.5×）= `service_tier:"priority"`
//! - 推理档位映射：`minimal→low`，`xhigh/max` 原样，其余原样

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use reqwest::Client;
use serde_json::{json, Value};

use crate::ai_service::llm::codex_auth::{self, CodexCredential};
use crate::ai_service::llm::provider::{
    LlmModelInfo, LlmProvider, LlmResponseWithTools, ThinkEffortsInfo,
};
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmConfig, LlmUsage};
use crate::ai_service::types::{FunctionCall, LlmMessage, ToolCall, ToolDefinition};
use crate::utils::proxy::build_proxied_client;

const RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const DEFAULT_MODEL: &str = "gpt-5.6-sol";
const MAX_RETRIES: usize = 5;

pub struct CodexProvider {
    model: String,
    reasoning_effort: Option<String>,
    fast_mode: bool,
    temperature: Option<f64>,
    timeout_secs: u64,
}

impl CodexProvider {
    pub fn from_config(cfg: &LlmConfig) -> Result<Self> {
        let model = if cfg.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            cfg.model.trim().to_string()
        };
        tracing::info!(
            "[Codex] from_config: model={}, reasoning_effort={:?}, fast_mode={}",
            model,
            cfg.reasoning_effort,
            cfg.fast_mode
        );
        Ok(Self {
            model,
            reasoning_effort: cfg.reasoning_effort.clone(),
            fast_mode: cfg.fast_mode,
            temperature: cfg.temperature,
            timeout_secs: cfg.timeout_secs.max(15),
        })
    }

    /// 构建带代理的 client 并取有效凭据（自动刷新）。
    async fn client_and_token(&self) -> Result<(Client, CodexCredential)> {
        let http = build_proxied_client(self.timeout_secs).await?;
        let cred = codex_auth::get_valid_credential(&http)
            .await?
            .context("未登录 Codex：请先在「大模型管理」中登录 ChatGPT 订阅")?;
        Ok((http, cred))
    }

    fn headers(&self, cred: &CodexCredential) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", cred.access))
                .context("Codex access_token 含非法字符")?,
        );
        headers.insert(
            "chatgpt-account-id",
            HeaderValue::from_str(&cred.account_id).context("Codex account_id 含非法字符")?,
        );
        headers.insert("originator", HeaderValue::from_static("lingchat"));
        headers.insert("OpenAI-Beta", HeaderValue::from_static("responses=experimental"));
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static("lingchat/0.5 (Windows)"));
        Ok(headers)
    }

    /// reasoning effort 映射：default/off/None → 省略 reasoning 字段；
    /// minimal→low；xhigh/max 及其余原样（dsh-codex/pi-ai 同款映射表）。
    fn mapped_effort(&self) -> Option<String> {
        let effort = self.reasoning_effort.as_deref()?.trim().to_lowercase();
        match effort.as_str() {
            "" | "default" | "off" => None,
            "minimal" => Some("low".to_string()),
            other => Some(other.to_string()),
        }
    }

    fn build_body(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&str>,
    ) -> Value {
        // system → instructions；其余按 Responses API input item 转换
        let mut instructions = String::new();
        let mut input: Vec<Value> = Vec::new();
        for m in messages {
            match m.role.as_str() {
                "system" => {
                    if !instructions.is_empty() {
                        instructions.push_str("\n\n");
                    }
                    instructions.push_str(&m.content);
                }
                "assistant" => {
                    if !m.content.is_empty() {
                        input.push(json!({
                            "type": "message",
                            "role": "assistant",
                            "content": [{ "type": "output_text", "text": m.content }],
                        }));
                    }
                    if let Some(calls) = &m.tool_calls {
                        for call in calls {
                            input.push(json!({
                                "type": "function_call",
                                "call_id": call.id,
                                "name": call.function.name,
                                "arguments": call.function.arguments,
                            }));
                        }
                    }
                }
                "tool" => {
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": m.tool_call_id.clone().unwrap_or_default(),
                        "output": m.content,
                    }));
                }
                _ => {
                    input.push(json!({
                        "type": "message",
                        "role": "user",
                        "content": [{ "type": "input_text", "text": m.content }],
                    }));
                }
            }
        }

        let mut body = json!({
            "model": self.model,
            "store": false,
            "stream": true,
            "instructions": if instructions.is_empty() { "You are a helpful assistant." } else { &instructions },
            "input": input,
            "include": ["reasoning.encrypted_content"],
        });

        if let Some(effort) = self.mapped_effort() {
            body["reasoning"] = json!({ "effort": effort, "summary": "auto" });
        }
        if self.fast_mode {
            body["service_tier"] = json!("priority");
        }
        if let Some(temperature) = self.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(definitions) = tools {
            if !definitions.is_empty() {
                let converted: Vec<Value> = definitions
                    .iter()
                    .map(|d| {
                        json!({
                            "type": "function",
                            "name": d.function.name,
                            "description": d.function.description,
                            "parameters": d.function.parameters,
                        })
                    })
                    .collect();
                body["tools"] = json!(converted);
                body["parallel_tool_calls"] = json!(true);
                body["tool_choice"] = match tool_choice {
                    Some("none") => json!("none"),
                    Some("any") | Some("required") => json!("required"),
                    _ => json!("auto"),
                };
            }
        }
        body
    }

    /// 发送请求（带 429/5xx 重试，最多 5 次，backoff 1s 起步封顶 30s）。
    /// 只在「流尚未产出任何内容」的建立阶段重试。
    async fn send_with_retry(&self, http: &Client, cred: &CodexCredential, body: &Value) -> Result<reqwest::Response> {
        let headers = self.headers(cred)?;
        let mut last_error = String::new();
        for attempt in 0..=MAX_RETRIES {
            let result = http
                .post(RESPONSES_URL)
                .headers(headers.clone())
                .json(body)
                .send()
                .await;
            let resp = match result {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = format!("请求发送失败: {e}");
                    if attempt < MAX_RETRIES && (e.is_connect() || e.is_timeout()) {
                        backoff_sleep(attempt, None).await;
                        continue;
                    }
                    return Err(anyhow!(last_error));
                }
            };
            let status = resp.status();
            if status.is_success() {
                return Ok(resp);
            }
            let retry_after_ms = resp
                .headers()
                .get("retry-after-ms")
                .or_else(|| resp.headers().get("retry-after"))
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let text = resp.text().await.unwrap_or_default();
            last_error = format!("Codex 请求失败 ({status}): {text}");
            let retryable = matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504);
            if attempt < MAX_RETRIES && retryable {
                tracing::warn!("[Codex] {status} 第 {} 次重试", attempt + 1);
                backoff_sleep(attempt, retry_after_ms).await;
                continue;
            }
            // 401/403：提示重新登录
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(anyhow!("Codex 登录状态失效（{status}），请重新登录: {text}"));
            }
            return Err(anyhow!(last_error));
        }
        Err(anyhow!(last_error))
    }

    async fn open_stream(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&str>,
    ) -> Result<ChunkStream> {
        let (http, cred) = self.client_and_token().await?;
        let body = self.build_body(messages, tools, tool_choice);
        crate::utils::llm_request_logger::log_request_body("codex", &body);
        let resp = self.send_with_retry(&http, &cred, &body).await?;

        let stream = async_stream::try_stream! {
            // 按字节累积：SSE 分隔符 \n\n 是 ASCII，切点必落在字符边界，整段解码才安全
            let mut buffer: Vec<u8> = Vec::new();
            // 工具调用累积：item_id → (call_id, name, arguments 片段)
            let mut pending_calls: BTreeMap<String, (String, String, String)> = BTreeMap::new();
            let mut finished_calls: Vec<ToolCall> = Vec::new();
            let mut usage: Option<LlmUsage> = None;
            let mut end_reason: Option<String> = None;
            // 思考链累积：流末打到日志窗口（与 [Kimi-Code Thinking] 行为对齐）
            let mut thinking_buffer = String::new();
            let mut byte_stream = resp.bytes_stream();

            'outer: while let Some(item) = byte_stream.next().await {
                let bytes = item.context("读取 Codex 流失败")?;
                buffer.extend_from_slice(&bytes);
                // SSE 事件以空行分隔（字节层面定位分隔符，再对完整事件解码一次）
                while let Some(pos) = find_subslice(&buffer, b"\n\n") {
                    let raw_event = String::from_utf8_lossy(&buffer[..pos]).into_owned();
                    buffer.drain(..pos + 2);
                    // 一个事件可能多行 data:，拼起来
                    let mut data = String::new();
                    for line in raw_event.lines() {
                        if let Some(d) = line.strip_prefix("data:") {
                            if !data.is_empty() {
                                data.push('\n');
                            }
                            data.push_str(d.trim_start());
                        }
                    }
                    let data = data.trim();
                    if data.is_empty() || data == "[DONE]" {
                        continue;
                    }
                    let Ok(event) = serde_json::from_str::<Value>(data) else {
                        continue;
                    };
                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match event_type {
                        "response.output_text.delta" => {
                            if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                                yield LlmChunk::Content(delta.to_string());
                            }
                        }
                        "response.reasoning_summary_text.delta"
                        | "response.reasoning_text.delta" => {
                            if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                                thinking_buffer.push_str(delta);
                                yield LlmChunk::Reasoning(delta.to_string());
                            }
                        }
                        "response.output_item.added" => {
                            if let Some(item) = event.get("item") {
                                if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                                    let item_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    pending_calls.insert(item_id, (call_id, name, String::new()));
                                }
                            }
                        }
                        "response.function_call_arguments.delta" => {
                            let item_id = event.get("item_id").and_then(|v| v.as_str()).unwrap_or("");
                            if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                                if let Some((_, name, args)) = pending_calls.get_mut(item_id) {
                                    args.push_str(delta);
                                    yield LlmChunk::ToolCallProgress {
                                        name: name.clone(),
                                        chars: args.len(),
                                    };
                                }
                            }
                        }
                        "response.output_item.done" => {
                            if let Some(item) = event.get("item") {
                                if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                                    let item_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                    let (call_id, name, mut args) = pending_calls
                                        .remove(item_id)
                                        .unwrap_or_default();
                                    // done 事件里通常带完整 arguments，以它为准
                                    if let Some(full) = item.get("arguments").and_then(|v| v.as_str()) {
                                        if !full.is_empty() {
                                            args = full.to_string();
                                        }
                                    }
                                    let final_call_id = item
                                        .get("call_id")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or(call_id);
                                    finished_calls.push(ToolCall {
                                        id: final_call_id,
                                        type_: "function".to_string(),
                                        function: FunctionCall {
                                            name,
                                            arguments: args,
                                        },
                                    });
                                }
                            }
                        }
                        "response.completed" | "response.incomplete" => {
                            if let Some(u) = event.pointer("/response/usage") {
                                usage = Some(LlmUsage {
                                    prompt_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                                    completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                                    total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                                    cached_tokens: u
                                        .pointer("/input_tokens_details/cached_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0),
                                });
                            }
                            if event_type == "response.incomplete" {
                                end_reason = Some("max_tokens".to_string());
                            }
                            break 'outer;
                        }
                        "response.failed" | "error" => {
                            let message = event
                                .pointer("/response/error/message")
                                .or_else(|| event.pointer("/error/message"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("未知错误");
                            Err(anyhow!("Codex 流式响应失败: {message}"))?;
                        }
                        _ => {}
                    }
                }
            }

            // 流末日志：思考链汇总 + token 用量（对齐 [Kimi-Code Thinking] 的日志窗口行为）
            if !thinking_buffer.is_empty() {
                tracing::info!("[Codex Thinking] {}", thinking_buffer);
            }
            if let Some(u) = usage {
                tracing::info!(
                    "[Codex] usage: prompt={} completion={} cached={}",
                    u.prompt_tokens,
                    u.completion_tokens,
                    u.cached_tokens
                );
            }

            if !finished_calls.is_empty() {
                yield LlmChunk::ToolCalls(std::mem::take(&mut finished_calls));
            }
            yield LlmChunk::StreamEnd {
                reason: end_reason,
                usage,
            };
        };
        Ok(Box::pin(stream))
    }

    /// 非流式 = 流式收集拼接（Codex 后端按流式工作）。
    async fn collect(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&str>,
    ) -> Result<(String, Vec<ToolCall>, Option<LlmUsage>)> {
        let mut stream = self.open_stream(messages, tools, tool_choice).await?;
        let mut content = String::new();
        let mut calls: Vec<ToolCall> = Vec::new();
        let mut usage = None;
        while let Some(chunk) = stream.next().await {
            match chunk? {
                LlmChunk::Content(text) => content.push_str(&text),
                LlmChunk::ToolCalls(list) => calls.extend(list),
                LlmChunk::StreamEnd { usage: u, .. } => usage = u,
                _ => {}
            }
        }
        Ok((content, calls, usage))
    }
}

async fn backoff_sleep(attempt: usize, retry_after_ms: Option<u64>) {
    let base = retry_after_ms.unwrap_or_else(|| 1000u64 << attempt.min(5));
    let capped = base.min(30_000);
    // ±20% 抖动
    let jitter = (rand_f64() * 0.4 - 0.2) * capped as f64;
    let delay = (capped as f64 + jitter).max(200.0) as u64;
    tokio::time::sleep(Duration::from_millis(delay)).await;
}

/// 在字节序列中查找子序列首次出现的位置。
///
/// 用于在未解码的 SSE 缓冲区里定位事件分隔符。之所以按字节定位，
/// 是因为 `bytes_stream()` 的分块边界落在任意字节偏移：若对每个分块单独
/// `String::from_utf8_lossy`，跨分块的多字节字符（中文占 3 字节）会被拆成
/// 两截、各自变成 U+FFFD；而 U+FFFD 在 JSON 字符串里是合法字符，
/// 后续 `serde_json::from_str` 不报错，损坏会静默流向回复、历史与 TTS
/// （与 Kimi-Code 的流式修复同款）。
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn rand_f64() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos % 1000) as f64 / 1000.0
}

#[async_trait]
impl LlmProvider for CodexProvider {
    async fn list_models(&self, _http: &Client) -> Result<Vec<LlmModelInfo>> {
        // 内置目录（与 dsh-codex/pi-ai 的 openai-codex.json 同步）：
        // 5.3/5.4/5.5 系档位 off~xhigh；5.6 系追加 max。
        let standard_efforts = || {
            Some(ThinkEffortsInfo {
                valid_efforts: vec![
                    "off".into(),
                    "minimal".into(),
                    "low".into(),
                    "medium".into(),
                    "high".into(),
                    "xhigh".into(),
                ],
                default_effort: None,
            })
        };
        let extended_efforts = || {
            Some(ThinkEffortsInfo {
                valid_efforts: vec![
                    "off".into(),
                    "minimal".into(),
                    "low".into(),
                    "medium".into(),
                    "high".into(),
                    "xhigh".into(),
                    "max".into(),
                ],
                default_effort: None,
            })
        };
        let model = |id: &str, name: &str, context: u64, extended: bool| LlmModelInfo {
            id: id.to_string(),
            display_name: Some(name.to_string()),
            context_length: Some(context),
            supports_reasoning: true,
            supports_thinking_type: None,
            think_efforts: if extended { extended_efforts() } else { standard_efforts() },
        };
        Ok(vec![
            model("gpt-5.3-codex-spark", "GPT-5.3 Codex Spark", 128000, false),
            model("gpt-5.4", "GPT-5.4", 272000, false),
            model("gpt-5.4-mini", "GPT-5.4 mini", 272000, false),
            model("gpt-5.5", "GPT-5.5", 272000, false),
            model("gpt-5.6-luna", "GPT-5.6 Luna", 272000, true),
            model("gpt-5.6-sol", "GPT-5.6 Sol", 272000, true),
            model("gpt-5.6-terra", "GPT-5.6 Terra", 272000, true),
        ])
    }

    async fn complete(&self, _http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let (content, _, _) = self.collect(messages, None, None).await?;
        Ok(content)
    }

    async fn complete_stream(&self, _http: &Client, messages: &[LlmMessage]) -> Result<ChunkStream> {
        self.open_stream(messages, None, None).await
    }

    fn supports_streaming_tools(&self) -> bool {
        true
    }

    async fn complete_stream_with_tools(
        &self,
        _http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<ChunkStream> {
        self.open_stream(messages, Some(tools), tool_choice).await
    }

    async fn complete_with_tools(
        &self,
        _http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<LlmResponseWithTools> {
        let (content, calls, usage) = self.collect(messages, Some(tools), tool_choice).await?;
        Ok(LlmResponseWithTools {
            content: if content.is_empty() { None } else { Some(content) },
            tool_calls: if calls.is_empty() { None } else { Some(calls) },
            usage,
        })
    }
}
