//! 基于 `genai` crate 的多供应商 LLM provider。
//!
//! 替换原先手写 HTTP/SSE 的 OpenAiProvider 和 GeminiProvider。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatResponse, ChatStreamEvent, ToolCall as GenaiToolCall,
    ToolChoice,
};
use genai::ServiceTarget;
use genai::resolver::{AuthData, Endpoint};
use genai::Client as GenaiClient;
use reqwest::Client;

use crate::ai_service::llm::provider::{LlmProvider, LlmResponseWithTools};
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmConfig};
use crate::ai_service::types::{LlmMessage, ToolDefinition};

// ─── Provider ────────────────────────────────────────────────────

pub struct GenaiProvider {
    client: GenaiClient,
    model: String,
    temperature: Option<f64>,
    top_p: Option<f64>,
    enable_thinking: bool,
}

impl GenaiProvider {
    pub fn new(cfg: &LlmConfig) -> Result<Self> {
        let model = cfg.model.clone();
        let mut builder = GenaiClient::builder();

        match cfg.provider.to_lowercase().as_str() {
            "deepseek" => {
                let key = cfg.api_key.clone();
                let base = if cfg.base_url.is_empty() {
                    "https://api.deepseek.com/".to_string()
                } else {
                    cfg.base_url.clone()
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
                    let base = cfg.base_url.clone();
                    builder = builder.with_service_target_resolver_fn(move |mut t: ServiceTarget| {
                        t.endpoint = Endpoint::from_owned(base);
                        Ok(t)
                    });
                }
            }
            "lmstudio" => {
                builder = builder
                    .with_adapter_kind(AdapterKind::OpenAI)
                    .with_service_target_resolver_fn(|mut t: ServiceTarget| {
                        t.endpoint = Endpoint::from_owned("http://localhost:1234/v1".to_string());
                        Ok(t)
                    });
            }
            "gemini" => {
                let key = cfg.api_key.clone();
                builder = builder
                    .with_adapter_kind(AdapterKind::Gemini)
                    .with_auth_resolver_fn(move |_| Ok(Some(AuthData::from_single(key))));
                if !cfg.base_url.is_empty() {
                    let base = cfg.base_url.clone();
                    builder = builder.with_service_target_resolver_fn(move |mut t: ServiceTarget| {
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
            temperature: cfg.temperature,
            top_p: cfg.top_p,
            enable_thinking: cfg.enable_thinking,
        })
    }

    // ── 工具方法 ──────────────────────────────────────────────────

    fn build_chat_request(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> ChatRequest {
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
                    genai_messages.push(ChatMessage::user(&msg.content));
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
        req
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
}

// ─── LlmProvider 实现 ────────────────────────────────────────────

#[async_trait]
impl LlmProvider for GenaiProvider {
    async fn complete(&self, _http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let chat_req = self.build_chat_request(messages, None);
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

    async fn complete_stream(&self, _http: &Client, messages: &[LlmMessage]) -> Result<ChunkStream> {
        let chat_req = self.build_chat_request(messages, None);
        crate::utils::llm_request_logger::log_request_body(
            &self.model,
            &serde_json::to_value(&chat_req).unwrap_or_default(),
        );
        let opts = self.build_chat_options(None);

        let stream_resp = self
            .client
            .exec_chat_stream(&self.model, chat_req, Some(&opts))
            .await
            .map_err(|e| anyhow!("genai 流式请求失败: {e}"))?;

        let mut inner = stream_resp.stream;

        let output = async_stream::try_stream! {
            while let Some(event) = inner.next().await {
                match event.map_err(|e| anyhow!("genai 流式事件错误: {e}"))? {
                    ChatStreamEvent::Start => {}
                    ChatStreamEvent::Chunk(chunk) => {
                        if !chunk.content.is_empty() {
                            yield LlmChunk::Content(chunk.content);
                        }
                    }
                    ChatStreamEvent::ReasoningChunk(chunk) => {
                        if !chunk.content.is_empty() {
                            yield LlmChunk::Reasoning(chunk.content);
                        }
                    }
                    ChatStreamEvent::ThoughtSignatureChunk(_) => {}
                    ChatStreamEvent::ToolCallChunk(_) => {}
                    ChatStreamEvent::End(end) => {
                        if let Some(reasoning) = end.captured_reasoning_content {
                            if !reasoning.is_empty() {
                                yield LlmChunk::Reasoning(reasoning);
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(output))
    }

    async fn complete_with_tools(
        &self,
        _http: &Client,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<LlmResponseWithTools> {
        let chat_req = self.build_chat_request(messages, Some(tools));
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

        Ok(LlmResponseWithTools { content, tool_calls })
    }
}
