//! 基于 `genai` crate 的多供应商 LLM provider。
//!
//! 替换原先手写 HTTP/SSE 的 OpenAiProvider 和 GeminiProvider。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatResponse, ChatStreamEvent,
    ToolCall as GenaiToolCall, ToolChoice, ToolResponse,
};
use genai::resolver::{AuthData, Endpoint};
use genai::Client as GenaiClient;
use genai::ServiceTarget;
use reqwest::Client;

use crate::ai_service::llm::provider::{LlmProvider, LlmResponseWithTools};
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmConfig};
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
            }
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
            }
            "lmstudio" => {
                builder = builder
                    .with_adapter_kind(AdapterKind::OpenAI)
                    .with_service_target_resolver_fn(|mut t: ServiceTarget| {
                        t.endpoint = Endpoint::from_owned("http://localhost:1234/v1/".to_string());
                        Ok(t)
                    });
            }
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
            }
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
                }
                "tool" => {
                    let call_id = msg
                        .tool_call_id
                        .as_deref()
                        .filter(|id| !id.trim().is_empty())
                        .ok_or_else(|| anyhow!("tool 消息缺少 tool_call_id"))?;
                    genai_messages
                        .push(ChatMessage::from(ToolResponse::new(call_id, &msg.content)));
                }
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
                }
                _ => {
                    let role = match msg.role.as_str() {
                        "assistant" => ChatMessage::assistant(&msg.content),
                        _ => ChatMessage::user(&msg.content),
                    };
                    genai_messages.push(role);
                }
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
            .with_capture_content(true);

        if let Some(temp) = self.temperature {
            opts = opts.with_temperature(temp);
        }
        if let Some(p) = self.top_p {
            opts = opts.with_top_p(p);
        }

        // DeepSeek Reasoner 等模型在 thinking 字段缺失时默认启用思考，
        // 始终注入 thinking 字段，不区分 provider — 与旧 OpenAiProvider 行为一致。
        // 对不支持该字段的 provider（如纯 OpenAI）通常会被忽略，无害。[TODO] 需要测试

        let thinking_type = if self.enable_thinking {
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
                        if let Some(calls) = end.captured_into_tool_calls() {
                            let calls = calls.iter().map(Self::convert_tool_call).collect();
                            yield LlmChunk::ToolCalls(calls);
                        }
                    }
                }
            }
        };

        Ok(Box::pin(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::{FunctionCall, ToolCall};

    fn provider() -> GenaiProvider {
        GenaiProvider::new(
            &LlmConfig {
                provider: "openai".to_string(),
                model: "test-model".to_string(),
                api_key: "test".to_string(),
                base_url: String::new(),
                timeout_secs: 30,
                temperature: None,
                top_p: None,
                enable_thinking: false,
                reasoning_effort: None,
            },
            Client::new(),
        )
        .unwrap()
    }

    #[test]
    fn serializes_plain_messages() {
        let request = provider()
            .build_chat_request(
                &[
                    LlmMessage::system("系统"),
                    LlmMessage::user("你好"),
                    LlmMessage::assistant("你好呀"),
                ],
                None,
            )
            .unwrap();
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["system"], "系统");
        assert_eq!(value["messages"][0]["role"], "User");
        assert_eq!(value["messages"][1]["role"], "Assistant");
    }

    #[test]
    fn serializes_tool_call_and_response_with_matching_id() {
        let call = ToolCall {
            id: "call-1".to_string(),
            type_: "function".to_string(),
            function: FunctionCall {
                name: "get_current_time".to_string(),
                arguments: "{}".to_string(),
            },
        };
        let request = provider()
            .build_chat_request(
                &[
                    LlmMessage::user("几点了"),
                    LlmMessage::tool(vec![call]),
                    LlmMessage::tool_result("call-1", r#"{"local_time":"now"}"#),
                ],
                None,
            )
            .unwrap();
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["messages"][1]["role"], "Assistant");
        assert_eq!(
            value["messages"][1]["content"][0]["ToolCall"]["call_id"],
            "call-1"
        );
        assert_eq!(
            value["messages"][1]["content"][0]["ToolCall"]["fn_name"],
            "get_current_time"
        );
        assert_eq!(value["messages"][2]["role"], "Tool");
        assert_eq!(
            value["messages"][2]["content"][0]["ToolResponse"]["call_id"],
            "call-1"
        );
    }

    #[test]
    fn rejects_tool_result_without_call_id() {
        let mut message = LlmMessage::tool_result("call-1", "{}");
        message.tool_call_id = None;
        let error = provider()
            .build_chat_request(&[message], None)
            .err()
            .unwrap();
        assert!(error.to_string().contains("缺少 tool_call_id"));
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
        true
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

        let response: ChatResponse = self
            .client
            .exec_chat(&self.model, chat_req, Some(&opts))
            .await
            .map_err(|e| anyhow!("genai 工具调用失败: {e}"))?;

        // 先借用获取文本，再消费获取 tool_calls
        let content = response.first_text().map(|s| s.to_string());

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
        })
    }
}
