//! Kimi-Code provider adapter.
//!
//! 参考 AstrBot 的 `kimi_code_source.py`：复用 Anthropic Messages API 协议，
//! 固定 base_url 为 https://api.kimi.com/coding，默认模型 kimi-for-coding，
//! 并强制携带 User-Agent: claude-code/0.1.0。

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::ai_service::llm::provider::{
    LlmModelInfo, LlmProvider, LlmResponseWithTools, ThinkEffortsInfo,
};
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmConfig, LlmUsage};
use crate::ai_service::types::{LlmMessage, ToolCall, ToolDefinition};

#[derive(Debug, Deserialize)]
struct KimiCodeModelsResponse {
    data: Vec<KimiCodeModelRecord>,
}

#[derive(Debug, Deserialize)]
struct KimiCodeModelRecord {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    supports_reasoning: bool,
    #[serde(default)]
    supports_thinking_type: Option<String>,
    #[serde(default)]
    think_efforts: Option<KimiCodeThinkEfforts>,
}

#[derive(Debug, Deserialize)]
struct KimiCodeThinkEfforts {
    #[serde(default)]
    support: bool,
    #[serde(default)]
    valid_efforts: Vec<String>,
    #[serde(default)]
    default_effort: Option<String>,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<String>,
}

fn kimi_code_models_endpoint(base_url: &str) -> String {
    let base = if base_url.trim().is_empty() {
        "https://api.kimi.com/coding"
    } else {
        base_url.trim().trim_end_matches('/')
    };
    if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

pub struct KimiCodeProvider {
    model: String,
    api_key: String,
    base_url: String,
    temperature: Option<f64>,
    top_p: Option<f64>,
    enable_thinking: bool,
    reasoning_effort: Option<String>,
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
            reasoning_effort: cfg.reasoning_effort.clone(),
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
            HeaderValue::from_str(&self.api_key).context("Kimi-Code API key 包含非法字符")?,
        );
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        Ok(headers)
    }

    fn build_request<'a>(
        &'a self,
        messages: &'a [LlmMessage],
        stream: bool,
        tools: Option<&'a [ToolDefinition]>,
        tool_choice: Option<serde_json::Value>,
    ) -> Result<MessagesRequest<'a>> {
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
                    role: "user".to_string(),
                    content: AnthropicMessageContent::Text(m.content.clone()),
                }),
                "assistant" if m.tool_calls.is_some() => {
                    let mut blocks = Vec::new();
                    if !m.content.is_empty() {
                        blocks.push(AnthropicRequestBlock::Text {
                            text: m.content.clone(),
                        });
                    }
                    blocks.extend(
                        m.tool_calls
                            .as_ref()
                            .expect("tool_calls 已通过条件判断")
                            .iter()
                            .map(|call| {
                                let input = serde_json::from_str(&call.function.arguments)
                                    .context("Kimi-Code 工具调用参数不是合法 JSON")?;
                                Ok(AnthropicRequestBlock::ToolUse {
                                    id: call.id.clone(),
                                    name: call.function.name.clone(),
                                    input,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?,
                    );
                    conversation.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: AnthropicMessageContent::Blocks(blocks),
                    });
                }
                "assistant" => conversation.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: AnthropicMessageContent::Text(m.content.clone()),
                }),
                "tool" => {
                    let tool_use_id = m
                        .tool_call_id
                        .clone()
                        .filter(|id| !id.trim().is_empty())
                        .ok_or_else(|| anyhow!("Kimi-Code tool 消息缺少 tool_call_id"))?;
                    conversation.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicMessageContent::Blocks(vec![
                            AnthropicRequestBlock::ToolResult {
                                tool_use_id,
                                content: m.content.clone(),
                            },
                        ]),
                    });
                }
                _ => conversation.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: AnthropicMessageContent::Text(m.content.clone()),
                }),
            }
        }

        // 推理深度（K3）：对齐 kimi-code 的做法，在 thinking 中携带
        // effort: "low"|"high"|"max"，未设置则不携带 effort 字段。
        let reasoning_effort = self.reasoning_effort.clone().filter(|e| !e.is_empty());
        // Anthropic Messages API 的 tool 格式为 {name, description, input_schema}
        // 与项目内部通用的 OpenAI 格式 {type, function} 不同，需要在此转换
        let anthropic_tools: Option<Vec<AnthropicTool<'a>>> = tools.map(|ts| {
            ts.iter()
                .map(|t| AnthropicTool {
                    name: &t.function.name,
                    description: &t.function.description,
                    input_schema: &t.function.parameters,
                })
                .collect()
        });

        // Anthropic tool_choice 格式为 {"type": "auto"|"any"|"tool", "name": "..."}
        let anthropic_tool_choice: Option<AnthropicToolChoice> =
            tool_choice.and_then(|tc| match tc {
                serde_json::Value::String(s) => match s.as_str() {
                    "auto" => Some(AnthropicToolChoice {
                        type_: "auto".to_string(),
                        effort: None,
                        name: None,
                    }),
                    "any" | "required" => Some(AnthropicToolChoice {
                        type_: "any".to_string(),
                        effort: None,
                        name: None,
                    }),

                    "none" => None,
                    _ => Some(AnthropicToolChoice {
                        type_: "auto".to_string(),
                        effort: None,
                        name: None,
                    }),
                },
                serde_json::Value::Object(obj) => {
                    let type_ = obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("auto")
                        .to_string();
                    let name = obj.get("name").and_then(|v| v.as_str()).map(String::from);
                    Some(AnthropicToolChoice {
                        type_,
                        effort: None,
                        name,
                    })
                }
                _ => None,
            });

        Ok(MessagesRequest {
            model: &self.model,
            max_tokens: 65536,
            stream,
            temperature: self.temperature,
            top_p: self.top_p,
            system: if system_text.is_empty() {
                None
            } else {
                Some(system_text)
            },
            messages: conversation,
            tools: anthropic_tools,
            tool_choice: anthropic_tool_choice,
            thinking: {
                // 设置了推理深度时视为启用思考链（K3 始终开启思考）
                if reasoning_effort.is_some() || self.enable_thinking {
                    Some(ThinkingConfig {
                        type_: "enabled".to_string(),
                        effort: reasoning_effort.clone(),
                    })
                } else {
                    Some(ThinkingConfig {
                        type_: "disabled".to_string(),
                        effort: None,
                    })
                }
            },
        })
    }

    fn parse_messages_with_tools_response(
        &self,
        parsed: MessagesResponse,
    ) -> Result<LlmResponseWithTools> {
        let mut content_text = String::new();
        let mut tool_calls: Option<Vec<ToolCall>> = None;
        for block in parsed.content {
            if let Some(t) = block.text {
                content_text.push_str(&t);
            }
            if block.type_ == "tool_use" {
                let id = block
                    .id
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow!("Kimi-Code tool_use 缺少 id"))?;
                let name = block
                    .name
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow!("Kimi-Code tool_use 缺少 name"))?;
                let input = block
                    .input
                    .ok_or_else(|| anyhow!("Kimi-Code tool_use 缺少 input"))?;
                let tc = ToolCall {
                    id,
                    type_: "function".to_string(),
                    function: crate::ai_service::types::FunctionCall {
                        name,
                        arguments: input.to_string(),
                    },
                };
                tool_calls.get_or_insert_with(Vec::new).push(tc);
            }
        }

        Ok(LlmResponseWithTools {
            content: if content_text.is_empty() {
                None
            } else {
                Some(content_text)
            },
            tool_calls,
            // 非流式请求的 usage 落在消息 JSON 的 usage 字段（若响应携带）
            usage: parsed.usage.as_ref().map(|u| LlmUsage {
                prompt_tokens: u.input_tokens.unwrap_or(0),
                completion_tokens: u.output_tokens.unwrap_or(0),
                total_tokens: u.input_tokens.unwrap_or(0) + u.output_tokens.unwrap_or(0),
                cached_tokens: u.cache_read_input_tokens.unwrap_or(0),
            }),
        })
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
    async fn list_models(&self, http: &Client) -> Result<Vec<LlmModelInfo>> {
        if self.api_key.trim().is_empty() {
            return Err(anyhow!("请先填写 Kimi Code API 密钥"));
        }

        let endpoint = kimi_code_models_endpoint(&self.base_url);
        let response = http
            .get(&endpoint)
            .bearer_auth(self.api_key.trim())
            .header(USER_AGENT, "claude-code/0.1.0")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .context("请求 Kimi Code 模型列表失败")?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let detail = detail.chars().take(500).collect::<String>();
            return Err(anyhow!("获取 Kimi Code 模型列表失败 ({status}): {detail}"));
        }

        let payload: KimiCodeModelsResponse = response
            .json()
            .await
            .context("解析 Kimi Code 模型列表失败")?;
        let models = payload
            .data
            .into_iter()
            .filter(|model| !model.id.trim().is_empty())
            .map(|model| LlmModelInfo {
                id: model.id,
                display_name: model.display_name,
                context_length: model.context_length,
                supports_reasoning: model.supports_reasoning,
                supports_thinking_type: model.supports_thinking_type,
                // 仅当模型显式声明支持调档且给出档位列表时才透传，
                // 否则视为不可调档（如 K2.7 思考常开、无 valid_efforts）
                think_efforts: model.think_efforts.and_then(|e| {
                    if e.support && !e.valid_efforts.is_empty() {
                        Some(ThinkEffortsInfo {
                            valid_efforts: e.valid_efforts,
                            default_effort: e.default_effort,
                        })
                    } else {
                        None
                    }
                }),
            })
            .collect::<Vec<_>>();

        if models.is_empty() {
            return Err(anyhow!("Kimi Code 没有返回可用模型"));
        }
        Ok(models)
    }

    async fn complete(&self, http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let body = self.build_request(messages, false, None, None)?;
        crate::utils::llm_request_logger::log_request_body(
            "kimicode",
            &serde_json::to_value(&body).unwrap_or_default(),
        );
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

        let parsed: MessagesResponse =
            resp.json().await.context("解析 Kimi-Code 响应 JSON 失败")?;
        self.parse_messages_response(parsed)
    }

    async fn complete_with_tools(
        &self,
        http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<LlmResponseWithTools> {
        let tool_choice_value = parse_tool_choice(tool_choice);

        let body = self.build_request(messages, false, Some(tools), tool_choice_value)?;
        crate::utils::llm_request_logger::log_request_body(
            "kimicode",
            &serde_json::to_value(&body).unwrap_or_default(),
        );
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
            return Err(anyhow!(
                "Kimi-Code function calling 失败 ({status}): {text}"
            ));
        }

        let parsed: MessagesResponse = resp
            .json()
            .await
            .context("解析 Kimi-Code (tools) 响应 JSON 失败")?;

        self.parse_messages_with_tools_response(parsed)
    }

    async fn complete_stream(&self, http: &Client, messages: &[LlmMessage]) -> Result<ChunkStream> {
        self.stream_impl(http, messages, None, None).await
    }

    /// Kimi-Code 支持 Anthropic SSE 的原生流式 function calling。
    fn supports_streaming_tools(&self) -> bool {
        true
    }

    async fn complete_stream_with_tools(
        &self,
        http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<ChunkStream> {
        let tool_choice_value = parse_tool_choice(tool_choice);
        self.stream_impl(http, messages, Some(tools), tool_choice_value)
            .await
    }
}

fn parse_tool_choice(tool_choice: Option<&str>) -> Option<serde_json::Value> {
    tool_choice.map(|choice| {
        if matches!(choice, "auto" | "none" | "required") {
            serde_json::Value::String(choice.to_string())
        } else {
            serde_json::from_str(choice)
                .unwrap_or_else(|_| serde_json::Value::String("auto".to_string()))
        }
    })
}

impl KimiCodeProvider {
    /// 统一的流式实现：Anthropic SSE 解析。
    ///
    /// 除文本/思考增量外，还处理工具调用块：
    /// `content_block_start`(tool_use) 登记 id/name，
    /// `content_block_delta`(input_json_delta) 累积 partial_json，
    /// 流结束时一次性以 `LlmChunk::ToolCalls` 抛出（与 genai provider 的语义一致）。
    async fn stream_impl(
        &self,
        http: &Client,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<serde_json::Value>,
    ) -> Result<ChunkStream> {
        let body = self.build_request(messages, true, tools, tool_choice)?;
        crate::utils::llm_request_logger::log_request_body(
            "kimicode",
            &serde_json::to_value(&body).unwrap_or_default(),
        );
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
            // 按字节累积：SSE 事件分隔符是 ASCII，按字节切分再整段解码，
            // 才能保证跨分块的多字节字符不被拆坏（详见 find_subslice 处注释）。
            let mut pending: Vec<u8> = Vec::new();
            let mut thinking_buffer = String::new();
            let mut text_buffer = String::new();
            let mut last_flush_len: usize = 0;
            // 流式工具调用累积：块 index → (id, name, partial_json)
            let mut tool_blocks: std::collections::BTreeMap<usize, (String, String, String)> =
                std::collections::BTreeMap::new();
            // Anthropic 流式 usage：input_tokens 在 message_start 一次性给出，
            // output_tokens 在 message_delta 逐段累加。
            let mut input_tokens: u64 = 0;
            let mut output_tokens: u64 = 0;
            let mut cached_tokens: u64 = 0;
            let mut bs = byte_stream;
            while let Some(item) = bs.next().await {
                let chunk = item.map_err(|e| anyhow!("Kimi-Code 流式读取失败: {e}"))?;
                pending.extend_from_slice(&chunk);

                loop {
                    let (pos, seplen) = match find_subslice(&pending, b"\n\n") {
                        Some(pos) => (pos, 2),
                        None => match find_subslice(&pending, b"\r\n\r\n") {
                            Some(pos) => (pos, 4),
                            None => break,
                        },
                    };
                    // pos 落在 ASCII 分隔符上，一定是字符边界，整段解码安全。
                    let event = String::from_utf8_lossy(&pending[..pos]).into_owned();
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
                            }
                            // 如果 text 为空但 thinking 有内容，把 thinking 作为正式回复兜底
                            // 工具调用轮的 thinking 只是决策过程，不能混入正文。
                            if text_buffer.is_empty() && !thinking_buffer.is_empty() && tool_blocks.is_empty() {
                                tracing::info!("[Kimi-Code] text 为空，使用 thinking 作为回复");
                                for line in thinking_buffer.lines() {
                                    yield LlmChunk::Content(line.to_string());
                                }
                            }
                            // 抛出累积完成的工具调用
                            if !tool_blocks.is_empty() {
                                yield LlmChunk::ToolCalls(collect_tool_calls(std::mem::take(&mut tool_blocks)));
                            }
                            // 终止信号：补发 usage（其他 provider 的 usage 随 StreamEnd 携带，
                            // Kimi 流此前没有 StreamEnd，顺带让上层拿到停止原因与用量）
                            yield LlmChunk::StreamEnd {
                                reason: Some("stop".to_string()),
                                usage: Some(LlmUsage {
                                    prompt_tokens: input_tokens,
                                    completion_tokens: output_tokens,
                                    total_tokens: input_tokens + output_tokens,
                                    cached_tokens,
                                }),
                            };
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
                                    if let Some(partial) = delta.partial_json {
                                        let idx = parsed.index.unwrap_or(0);
                                        if let Some(block) = tool_blocks.get_mut(&idx) {
                                            block.2.push_str(&partial);
                                            // 实时汇报参数生成进度（驱动前端「正在写入…N 字」提示）
                                            yield LlmChunk::ToolCallProgress {
                                                name: block.1.clone(),
                                                chars: block.2.chars().count(),
                                            };
                                        }
                                    }
                                }
                            }
                            "content_block_start" => {
                                if let Some(block) = parsed.content_block {
                                    if block.type_ == "tool_use" {
                                        let idx = parsed.index.unwrap_or(0);
                                        let name = block.name.unwrap_or_default();
                                        tool_blocks.insert(
                                            idx,
                                            (
                                                block.id.unwrap_or_default(),
                                                name.clone(),
                                                String::new(),
                                            ),
                                        );
                                        // 工具块一开始就让前端亮出「正在生成」状态
                                        yield LlmChunk::ToolCallProgress { name, chars: 0 };
                                    }
                                }
                            }
                            "message_start" => {
                                // input_tokens 在 message_start 一次性给出（含缓存统计）
                                if let Some(u) = parsed.message.and_then(|m| m.usage) {
                                    if let Some(i) = u.input_tokens {
                                        input_tokens = i;
                                    }
                                    if let Some(c) = u.cache_read_input_tokens {
                                        cached_tokens = c;
                                    }
                                }
                            }
                            "message_delta" => {
                                // output_tokens 是每个 delta 的增量，需累加
                                if let Some(u) = parsed.usage {
                                    if let Some(o) = u.output_tokens {
                                        output_tokens += o;
                                    }
                                }
                            }
                            "content_block_stop" | "message_stop" => {
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
            // 兜底：text 为空时使用 thinking。
            // 但本轮若包含工具调用（tool_use 块），thinking 只是决策过程，
            // 不能当正文下发——否则会污染工具闭环的 assistant 消息与下游句子解析。
            if text_buffer.is_empty() && !thinking_buffer.is_empty() && tool_blocks.is_empty() {
                tracing::info!("[Kimi-Code] text 为空，使用 thinking 作为回复");
                for line in thinking_buffer.lines() {
                    yield LlmChunk::Content(line.to_string());
                }
            }
            // 流正常结束时抛出累积完成的工具调用
            if !tool_blocks.is_empty() {
                yield LlmChunk::ToolCalls(collect_tool_calls(std::mem::take(&mut tool_blocks)));
            }
            // 终止信号（与 [DONE] 分支同语义；无 [DONE] 时也会走到这里）
            yield LlmChunk::StreamEnd {
                reason: Some("stop".to_string()),
                usage: Some(LlmUsage {
                    prompt_tokens: input_tokens,
                    completion_tokens: output_tokens,
                    total_tokens: input_tokens + output_tokens,
                    cached_tokens,
                }),
            };
        };

        Ok(Box::pin(stream))
    }
}

/// 在字节序列中查找子序列首次出现的位置。
///
/// 用于在未解码的 SSE 缓冲区里定位事件分隔符。之所以必须在字节层面定位，
/// 是因为 `bytes_stream()` 的分块边界落在任意字节偏移上：若对每个分块单独调用
/// `String::from_utf8_lossy`，跨分块的多字节字符（中文占 3 字节）会被拆成两截，
/// 前后各自变成 U+FFFD，字符不可恢复。而 U+FFFD 在 JSON 字符串里是合法字符，
/// 后续 `serde_json::from_str` 不会报错，损坏会静默流向界面、历史记录与 TTS。
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}

/// 把流式累积的工具块组装成 ToolCall 列表（按块 index 升序）。
fn collect_tool_calls(
    blocks: std::collections::BTreeMap<usize, (String, String, String)>,
) -> Vec<crate::ai_service::types::ToolCall> {
    blocks
        .into_values()
        .map(|(id, name, arguments)| crate::ai_service::types::ToolCall {
            id,
            type_: "function".to_string(),
            function: crate::ai_service::types::FunctionCall {
                name,
                arguments: if arguments.trim().is_empty() {
                    "{}".to_string()
                } else {
                    arguments
                },
            },
        })
        .collect()
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
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<AnthropicToolChoice>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicMessageContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicMessageContent {
    Text(String),
    Blocks(Vec<AnthropicRequestBlock>),
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum AnthropicRequestBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Anthropic Messages API 的 tool 定义格式：{name, description, input_schema}
#[derive(Serialize)]
struct AnthropicTool<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a serde_json::Value,
}

/// Anthropic Messages API 的 tool_choice 格式：{"type": "auto" | "any" | "tool", "name": "..."}
#[derive(Serialize)]
struct AnthropicToolChoice {
    #[serde(rename = "type")]
    type_: String,
    /// K3 推理深度："low" | "high" | "max"，未设置则不携带该字段
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    /// 非流式响应的用量（Anthropic 顶层 usage 字段）。
    #[serde(default)]
    usage: Option<StreamUsage>,
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
    /// 内容块序号（content_block_start/delta/stop 携带）。
    #[serde(default)]
    index: Option<usize>,
    /// content_block_start 携带的块本体（tool_use 的 id/name 在这里）。
    #[serde(default)]
    content_block: Option<ContentBlock>,
    #[serde(default)]
    delta: Option<MessageDelta>,
    /// message_delta 携带的 usage（output_tokens 为该 delta 的增量）。
    #[serde(default)]
    usage: Option<StreamUsage>,
    /// message_start 携带的 message 头（input_tokens 一次性给出）。
    #[serde(default)]
    message: Option<StreamMessageHeader>,
}

#[derive(Deserialize, Default)]
struct StreamUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    /// Anthropic：cache_read_input_tokens（输入中命中缓存的 token 数）。
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
}

#[derive(Deserialize, Default)]
struct StreamMessageHeader {
    #[serde(default)]
    usage: Option<StreamUsage>,
}

#[derive(Deserialize, Default, Debug)]
struct MessageDelta {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    /// input_json_delta：流式工具调用参数的 JSON 片段。
    #[serde(default)]
    partial_json: Option<String>,
}
