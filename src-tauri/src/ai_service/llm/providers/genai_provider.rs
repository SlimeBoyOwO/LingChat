//! 基于 `genai` crate 的多供应商 LLM provider。
//!
//! 替换原先手写 HTTP/SSE 的 OpenAiProvider 和 GeminiProvider。

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use futures_util::StreamExt;
use genai::Client as GenaiClient;
use genai::ServiceTarget;
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatResponse, ChatStreamEvent, ContentPart,
    MessageContent, StopReason, ToolCall as GenaiToolCall, ToolChoice, ToolResponse,
};
use genai::resolver::{AuthData, Endpoint};
use reqwest::Client;

use crate::ai_service::llm::provider::{LlmProvider, LlmResponseWithTools};
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmConfig, LlmUsage};
use crate::ai_service::types::{LlmMessage, ToolDefinition};

// ─── Provider ────────────────────────────────────────────────────
// 钦灵：为了修复 DeepSeek 问题，我在这里预留了两个字段，以备将来使用。

pub struct GenaiProvider {
    client: GenaiClient,
    model: String,
    _provider: String,
    temperature: Option<f64>,
    top_p: Option<f64>,
    enable_thinking: bool,
    _reasoning_effort: Option<String>,
    /// 是否 MiniMax 兼容接口（base_url 或模型名含 minimax）。
    /// MiniMax 的 OpenAI 兼容 API 只接受 thinking.type = "adaptive" / "disabled"，
    /// 传 "enabled" 会直接 400 报错（invalid thinking.type），需单独映射。
    is_minimax: bool,
}

/// 规范化 base_url：确保以 `/` 结尾。
///
/// genai 的 OpenAI 兼容 adapter 用 `Url::join("chat/completions")` 拼接路径
/// （不是字符串拼接）。若 base_url 不以 `/` 结尾（如 `https://api.deepseek.com/v1`），
/// `v1` 会被当作"文件"替换掉，拼出 `https://api.deepseek.com/chat/completions` → 404。
///
/// 修复：在传给 genai 前补上尾斜杠（`https://api.deepseek.com/v1/`）。
fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return raw.to_string();
    }
    format!("{trimmed}/")
}

/// 从 `data:image/<type>;base64,<data>` 的 data URL 前缀解析 MIME 类型，
/// 供 genai 的 Binary content part 使用（`is_image()` 依赖 `image/` 前缀）。
fn infer_image_mime(data_url: &str) -> String {
    let lower = data_url.trim();
    if let Some(rest) = lower.strip_prefix("data:") {
        if let Some(semi) = rest.find(';') {
            let mime = rest[..semi].to_string();
            if !mime.is_empty() {
                return mime;
            }
        }
    }
    // 兜底：无法解析时按通用 JPEG 处理（OpenAI 兼容端点通常忽略具体子类型）
    "image/jpeg".to_string()
}

impl GenaiProvider {
    pub fn new(cfg: &LlmConfig, http: Client) -> Result<Self> {
        let model = cfg.model.clone();
        let mut builder = GenaiClient::builder().with_reqwest(http);

        match cfg.provider.to_lowercase().as_str() {
            "deepseek" => {
                let key = cfg.api_key.clone();
                // 默认 base_url 以 `/` 结尾；用户配置经 normalize 补尾斜杠
                let base = if cfg.base_url.is_empty() {
                    "https://api.deepseek.com/".to_string()
                } else {
                    normalize_base_url(&cfg.base_url)
                };
                builder = builder
                    .with_adapter_kind(AdapterKind::DeepSeek)
                    .with_auth_resolver_fn(move |_| Ok(Some(AuthData::from_single(key))))
                    .with_service_target_resolver_fn(move |mut t: ServiceTarget| {
                        t.endpoint = Endpoint::from_owned(base);
                        Ok(t)
                    });
            },
            "openai" => {
                let key = cfg.api_key.clone();
                builder = builder
                    .with_adapter_kind(AdapterKind::OpenAI)
                    .with_auth_resolver_fn(move |_| Ok(Some(AuthData::from_single(key))));
                if !cfg.base_url.is_empty() {
                    let base = normalize_base_url(&cfg.base_url);
                    builder =
                        builder.with_service_target_resolver_fn(move |mut t: ServiceTarget| {
                            t.endpoint = Endpoint::from_owned(base);
                            Ok(t)
                        });
                }
            },
            "lmstudio" => {
                builder = builder
                    .with_adapter_kind(AdapterKind::OpenAI)
                    .with_service_target_resolver_fn(|mut t: ServiceTarget| {
                        t.endpoint = Endpoint::from_owned("http://localhost:1234/v1/".to_string());
                        Ok(t)
                    });
            },
            "gemini" => {
                let key = cfg.api_key.clone();
                builder = builder
                    .with_adapter_kind(AdapterKind::Gemini)
                    .with_auth_resolver_fn(move |_| Ok(Some(AuthData::from_single(key))));
                if !cfg.base_url.is_empty() {
                    let base = normalize_base_url(&cfg.base_url);
                    builder =
                        builder.with_service_target_resolver_fn(move |mut t: ServiceTarget| {
                            t.endpoint = Endpoint::from_owned(base);
                            Ok(t)
                        });
                }
            },
            other => return Err(anyhow!("GenaiProvider 不支持的 provider: {other}")),
        }

        Ok(Self {
            client: builder.build(),
            model,
            _provider: cfg.provider.to_lowercase(),
            temperature: cfg.temperature,
            top_p: cfg.top_p,
            enable_thinking: cfg.enable_thinking,
            _reasoning_effort: cfg.reasoning_effort.clone(),
            is_minimax: cfg.base_url.to_lowercase().contains("minimax")
                || cfg.model.to_lowercase().contains("minimax"),
        })
    }

    // ── 工具方法 ──────────────────────────────────────────────────

    fn build_chat_request(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<ChatRequest> {
        let mut system_text = String::new();
        let mut genai_messages: Vec<ChatMessage> = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    if !system_text.is_empty() {
                        system_text.push('\n');
                    }
                    system_text.push_str(&msg.content);
                },
                "tool" => {
                    let call_id = msg
                        .tool_call_id
                        .as_deref()
                        .filter(|id| !id.trim().is_empty())
                        .ok_or_else(|| anyhow!("tool 消息缺少 tool_call_id"))?;
                    genai_messages
                        .push(ChatMessage::from(ToolResponse::new(call_id, &msg.content)));
                },
                "assistant" if msg.tool_calls.is_some() => {
                    let calls = msg
                        .tool_calls
                        .as_ref()
                        .expect("tool_calls 已通过条件判断")
                        .iter()
                        .map(|call| {
                            let arguments = serde_json::from_str(&call.function.arguments)
                                .map_err(|error| anyhow!("工具调用参数无法编码: {error}"))?;
                            Ok(GenaiToolCall {
                                call_id: call.id.clone(),
                                fn_name: call.function.name.clone(),
                                fn_arguments: arguments,
                                thought_signatures: None,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    genai_messages.push(ChatMessage::from(calls));
                },
                _ => {
                    let role = match msg.role.as_str() {
                        "assistant" => ChatMessage::assistant(&msg.content),
                        _ => {
                            // 原生多模态：当该用户消息携带图片时，把文本与图片拼成
                            // 多 part 内容（OpenAI 兼容 image_url / Gemini inline_data）。
                            if let Some(data_url) = msg.image_data_url.as_deref() {
                                let mut content = MessageContent::from_parts(Vec::new());
                                if !msg.content.is_empty() {
                                    content.push(ContentPart::Text(msg.content.clone()));
                                }
                                content.push(ContentPart::from_binary_url(
                                    infer_image_mime(data_url),
                                    data_url.to_string(),
                                    None,
                                ));
                                ChatMessage::user(content)
                            } else {
                                ChatMessage::user(&msg.content)
                            }
                        },
                    };
                    genai_messages.push(role);
                },
            }
        }

        let mut req = ChatRequest::new(genai_messages);
        if !system_text.is_empty() {
            req = req.with_system(&system_text);
        }
        if let Some(tools) = tools {
            let gtools: Vec<_> = tools.iter().map(Self::convert_tool_def).collect();
            req = req.with_tools(gtools);
        }
        Ok(req)
    }

    fn build_chat_options(&self, tool_choice: Option<&str>) -> ChatOptions {
        let mut opts = ChatOptions::default()
            .with_capture_tool_calls(true)
            .with_capture_content(true)
            // 捕获 token 用量：流式结束时从 StreamEnd.captured_usage 读取，
            // 非流式从 ChatResponse.usage 读取，供 AI 助手用量统计使用。
            .with_capture_usage(true);

        if let Some(temp) = self.temperature {
            opts = opts.with_temperature(temp);
        }
        if let Some(p) = self.top_p {
            opts = opts.with_top_p(p);
        }

        // DeepSeek Reasoner 等模型在 thinking 字段缺失时默认启用思考，
        // 始终注入 thinking 字段，不区分 provider — 与旧 OpenAiProvider 行为一致。
        // 对不支持该字段的 provider（如纯 OpenAI）通常会被忽略，无害。
        //
        // MiniMax 例外：其 OpenAI 兼容接口只接受 "adaptive" / "disabled"，
        // 传 "enabled" 会返回 400（invalid thinking.type (2013)），启用思考时映射为
        // "adaptive"（由模型自主决定思考深度），关闭时同样是 "disabled"。

        let thinking_type = if self.is_minimax {
            if self.enable_thinking {
                "adaptive"
            } else {
                "disabled"
            }
        } else if self.enable_thinking {
            "enabled"
        } else {
            "disabled"
        };

        opts = opts.with_extra_body(serde_json::json!({
            "thinking": { "type": thinking_type }
        }));

        if self.enable_thinking {
            opts = opts.with_capture_reasoning_content(true);
        }

        if let Some(tc) = tool_choice {
            let choice = match tc {
                "auto" => ToolChoice::Auto,
                "none" => ToolChoice::None,
                "required" => ToolChoice::Required,
                _ => ToolChoice::Auto,
            };
            opts = opts.with_tool_choice(choice);
        }
        opts
    }

    fn convert_tool_def(tool: &ToolDefinition) -> genai::chat::Tool {
        let mut gt = genai::chat::Tool::new(&tool.function.name);
        if !tool.function.description.is_empty() {
            gt = gt.with_description(&tool.function.description);
        }
        if !tool.function.parameters.is_null() {
            gt = gt.with_schema(tool.function.parameters.clone());
        }
        gt
    }

    fn convert_tool_call(tc: &GenaiToolCall) -> crate::ai_service::types::ToolCall {
        crate::ai_service::types::ToolCall {
            id: tc.call_id.clone(),
            type_: "function".to_string(),
            function: crate::ai_service::types::FunctionCall {
                name: tc.fn_name.clone(),
                arguments: tc.fn_arguments.to_string(),
            },
        }
    }

    /// 归一化 genai 的停止原因为稳定字符串，供上层做截断检测等决策。
    fn normalize_stop_reason(reason: &StopReason) -> String {
        match reason {
            StopReason::Completed(_) => "stop".to_string(),
            StopReason::MaxTokens(_) => "max_tokens".to_string(),
            StopReason::ToolCall(_) => "tool_calls".to_string(),
            StopReason::ContentFilter(_) => "content_filter".to_string(),
            StopReason::StopSequence(_) => "stop_sequence".to_string(),
            StopReason::Other(s) => s.clone(),
        }
    }

    /// genai 归一化用量 → 项目 LlmUsage。
    ///
    /// genai 反序列化时把 0 视为 None（跨 provider 一致：OpenAI 常给不适用计数器
    /// 返回 0），这里统一补 0；「全 0 / 未上报」由上层按需过滤。
    /// 缓存命中数取自 prompt_tokens_details.cached_tokens（OpenAI cached_tokens /
    /// Anthropic cache_read_input_tokens 的归一化字段）。
    fn convert_usage(u: &genai::chat::Usage) -> LlmUsage {
        LlmUsage {
            prompt_tokens: u.prompt_tokens.unwrap_or(0).max(0) as u64,
            completion_tokens: u.completion_tokens.unwrap_or(0).max(0) as u64,
            total_tokens: u.total_tokens.unwrap_or(0).max(0) as u64,
            cached_tokens: u
                .prompt_tokens_details
                .as_ref()
                .and_then(|d| d.cached_tokens)
                .unwrap_or(0)
                .max(0) as u64,
        }
    }

    async fn complete_stream_inner(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&str>,
    ) -> Result<ChunkStream> {
        let chat_req = self.build_chat_request(messages, tools)?;
        crate::utils::llm_request_logger::log_request_body(
            &self.model,
            &serde_json::to_value(&chat_req).unwrap_or_default(),
        );
        let opts = self.build_chat_options(tool_choice);
        // 诊断日志：记录实际 ChatOptions，帮助排查 MiniMax 等兼容问题。
        // ChatOptions 字段为 public，直接访问；ToolChoice 转为字符串避免序列化依赖。
        let tool_choice_str = opts.tool_choice.as_ref().map(|tc| match tc {
            ToolChoice::Auto => "auto",
            ToolChoice::None => "none",
            ToolChoice::Required => "required",
            ToolChoice::Tool { .. } => "specific",
        });
        tracing::debug!(
            model = self.model,
            is_minimax = self.is_minimax,
            temperature = ?opts.temperature,
            top_p = ?opts.top_p,
            tool_choice = tool_choice_str,
            extra_body = ?opts.extra_body,
            "GenaiProvider 准备 LLM 请求"
        );

        let stream_resp = self
            .client
            .exec_chat_stream(&self.model, chat_req, Some(&opts))
            .await
            .map_err(|e| anyhow!("genai 流式请求失败: {e}"))?;
        let mut inner = stream_resp.stream;

        let output = async_stream::try_stream! {
            while let Some(event) = inner.next().await {
                match event.map_err(|e| anyhow!("genai 流式事件错误: {e}"))? {
                    ChatStreamEvent::Start | ChatStreamEvent::ThoughtSignatureChunk(_) | ChatStreamEvent::ToolCallChunk(_) => {}
                    ChatStreamEvent::Chunk(chunk) if !chunk.content.is_empty() => {
                        yield LlmChunk::Content(chunk.content);
                    }
                    ChatStreamEvent::ReasoningChunk(chunk) if !chunk.content.is_empty() => {
                        yield LlmChunk::Reasoning(chunk.content);
                    }
                    ChatStreamEvent::Chunk(_) | ChatStreamEvent::ReasoningChunk(_) => {}
                    ChatStreamEvent::End(end) => {
                        if let Some(reasoning) = end.captured_reasoning_content.clone() {
                            if !reasoning.is_empty() {
                                yield LlmChunk::Reasoning(reasoning);
                            }
                        }
                        // 先取走用量与停止原因（captured_into_tool_calls 会移动 end）
                        let usage = end.captured_usage.as_ref().map(Self::convert_usage);
                        let reason = end
                            .captured_stop_reason
                            .as_ref()
                            .map(Self::normalize_stop_reason);
                        if let Some(calls) = end.captured_into_tool_calls() {
                            let calls = calls.iter().map(Self::convert_tool_call).collect();
                            yield LlmChunk::ToolCalls(calls);
                        }
                        // 终止信号：透传归一化停止原因（工具闭环用它检测截断）+ 本轮用量
                        yield LlmChunk::StreamEnd { reason, usage };
                    }
                }
            }
        };

        Ok(Box::pin(output))
    }
}

// ─── LlmProvider 实现 ────────────────────────────────────────────

#[async_trait]
impl LlmProvider for GenaiProvider {
    async fn complete(&self, _http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let chat_req = self.build_chat_request(messages, None)?;
        crate::utils::llm_request_logger::log_request_body(
            &self.model,
            &serde_json::to_value(&chat_req).unwrap_or_default(),
        );
        let opts = self.build_chat_options(None);

        let response: ChatResponse = self
            .client
            .exec_chat(&self.model, chat_req, Some(&opts))
            .await
            .map_err(|e| anyhow!("genai 非流式调用失败: {e}"))?;

        response
            .into_first_text()
            .ok_or_else(|| anyhow!("genai 响应无文本内容"))
    }

    async fn complete_stream(
        &self,
        _http: &Client,
        messages: &[LlmMessage],
    ) -> Result<ChunkStream> {
        self.complete_stream_inner(messages, None, None).await
    }

    fn supports_streaming_tools(&self) -> bool {
        // MiniMax 的 OpenAI 兼容端点在流式工具调用上行为不稳定（实测非流式可
        // 靠返回 tool_calls，流式下模型常直接给文字回复而不调工具）。先降级到
        // 非流式工具调用，保证功能可用；后续抓到真实请求/响应后再恢复流式。
        // TODO: 待拿到 LLM 请求日志并定位流式 tool_calls 解析问题后恢复。
        !self.is_minimax
    }

    async fn complete_stream_with_tools(
        &self,
        _http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<ChunkStream> {
        self.complete_stream_inner(messages, Some(tools), tool_choice)
            .await
    }

    async fn complete_with_tools(
        &self,
        _http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<LlmResponseWithTools> {
        let chat_req = self.build_chat_request(messages, Some(tools))?;
        crate::utils::llm_request_logger::log_request_body(
            &self.model,
            &serde_json::to_value(&chat_req).unwrap_or_default(),
        );
        let opts = self.build_chat_options(tool_choice);
        // 诊断日志：记录实际 ChatOptions。
        let tool_choice_str = opts.tool_choice.as_ref().map(|tc| match tc {
            ToolChoice::Auto => "auto",
            ToolChoice::None => "none",
            ToolChoice::Required => "required",
            ToolChoice::Tool { .. } => "specific",
        });
        tracing::debug!(
            model = self.model,
            is_minimax = self.is_minimax,
            temperature = ?opts.temperature,
            top_p = ?opts.top_p,
            tool_choice = tool_choice_str,
            extra_body = ?opts.extra_body,
            "GenaiProvider 准备非流式 LLM 请求"
        );

        let response: ChatResponse = self
            .client
            .exec_chat(&self.model, chat_req, Some(&opts))
            .await
            .map_err(|e| anyhow!("genai 工具调用失败: {e}"))?;

        // 先借用获取文本/用量，再消费获取 tool_calls。
        // 注意：ChatResponse.usage 是值而非 Option（未上报时字段全为 None）。
        let content = response.first_text().map(|s| s.to_string());
        let usage = if response.usage.prompt_tokens.is_none()
            && response.usage.completion_tokens.is_none()
        {
            None
        } else {
            Some(Self::convert_usage(&response.usage))
        };

        let tool_calls: Option<Vec<crate::ai_service::types::ToolCall>> = {
            let calls = response.into_tool_calls();
            if calls.is_empty() {
                None
            } else {
                Some(calls.iter().map(Self::convert_tool_call).collect())
            }
        };

        Ok(LlmResponseWithTools {
            content,
            tool_calls,
            usage,
        })
    }
}
