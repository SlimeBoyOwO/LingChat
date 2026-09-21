//! WorkBuddy（腾讯 CodeBuddy）订阅 provider。
//!
//! 协议：OpenAI 兼容 Chat Completions（`POST {base}/v2/chat/completions`，强制 SSE 流式，
//! 非流式入口内部聚合）。凭据来自 `workbuddy::auth`（浏览器授权登录，自动刷新）；
//! 链路**直连不走代理**（上游按服务区域锁定，国内版走代理反而会失败）。
//!
//! 请求要点（参考 astrbot_plugin_workbuddy_provider / workbuddy2api 协议研究）：
//! - 头：完整设备/会话头族（`auth::account_headers`），`Accept: application/json, text/event-stream`
//! - 体：`stream:true` 必带；`tool_choice` 只发字符串；DeepSeek 系带
//!   `thinking:{type:enabled}`；`reasoning_effort` 按目录档位降级；带 `prompt_cache_key` 降费
//! - SSE：OpenAI chunk 帧（`choices[].delta.content/reasoning_content/tool_calls`）+ `[DONE]`

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use serde_json::{Value, json};

use crate::ai_service::llm::provider::{LlmModelInfo, LlmProvider, LlmResponseWithTools};
use crate::ai_service::llm::workbuddy::auth::{self, WorkBuddyCredential, account_headers};
use crate::ai_service::llm::workbuddy::models;
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmConfig, LlmUsage};
use crate::ai_service::types::{FunctionCall, LlmMessage, ToolCall, ToolDefinition};

const DEFAULT_MODEL: &str = "auto";
const MAX_RETRIES: usize = 3;

/// 上游可识别的推理档位全集（低→高）。
const EFFORT_RANK: [(&str, u8); 7] = [
    ("off", 0),
    ("minimal", 1),
    ("low", 2),
    ("medium", 3),
    ("high", 4),
    ("xhigh", 5),
    ("max", 6),
];

/// 按模型支持档位降级：取 ≤ 请求档位的最高支持档；全部高于请求档时取最低档。
fn downgrade_effort(requested: &str, supported: &[String]) -> String {
    if supported.is_empty() || supported.iter().any(|s| s == requested) {
        return requested.to_string();
    }
    let Some(&rank) = EFFORT_RANK
        .iter()
        .find(|(name, _)| *name == requested)
        .map(|(_, r)| r)
    else {
        return requested.to_string();
    };
    let mut ranked: Vec<(u8, &str)> = supported
        .iter()
        .filter_map(|name| {
            EFFORT_RANK
                .iter()
                .find(|(n, _)| *n == name.as_str())
                .map(|(_, r)| (*r, name.as_str()))
        })
        .collect();
    ranked.sort();
    if ranked.is_empty() {
        return requested.to_string();
    }
    let below: Vec<&(u8, &str)> = ranked.iter().filter(|(r, _)| *r <= rank).collect();
    match below.last() {
        Some(&&(_, name)) => name.to_string(),
        None => ranked[0].1.to_string(),
    }
}

pub struct WorkBuddyProvider {
    model: String,
    /// 登录区域（cn/global）：决定上游 base URL 与平台段 UA。
    /// 运行期实际区域以凭据为准（凭据里带 realm），此处为配置期推断值。
    #[allow(dead_code)]
    realm: String,
    reasoning_effort: Option<String>,
    /// 是否开启思考链（DeepSeek 系需要显式 thinking.type=enabled 才有思维链）。
    enable_thinking: bool,
    temperature: Option<f64>,
    top_p: Option<f64>,
    timeout_secs: u64,
    /// 模型目录缓存：(条目, 缓存时刻)；10 分钟内复用。
    catalog_cache: Mutex<Option<(Vec<LlmModelInfo>, std::time::Instant)>>,
}

impl WorkBuddyProvider {
    pub fn from_config(cfg: &LlmConfig) -> Result<Self> {
        let model = if cfg.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            cfg.model.trim().to_string()
        };
        let realm = auth::normalize_realm(&cfg.base_url);
        tracing::info!(
            "[WorkBuddy] from_config: model={}, realm={}, reasoning_effort={:?}, enable_thinking={}",
            model,
            realm,
            cfg.reasoning_effort,
            cfg.enable_thinking
        );
        Ok(Self {
            model,
            realm,
            reasoning_effort: cfg.reasoning_effort.clone(),
            enable_thinking: cfg.enable_thinking,
            temperature: cfg.temperature,
            top_p: cfg.top_p,
            timeout_secs: cfg.timeout_secs.max(30),
            catalog_cache: Mutex::new(None),
        })
    }

    /// 构建直连 client 并取有效凭据（自动刷新）。
    ///
    /// 直连：WorkBuddy 上游按服务区域锁定，国内版走代理反而会失败（与插件一致）。
    /// TLS 走项目统一的 webpki-roots 配置（Android 上缺此配置会 TLS panic）；
    /// 只设 read_timeout，不设全局 timeout，避免长回答的 SSE 流被整体掐断。
    async fn client_and_credential(&self) -> Result<(Client, WorkBuddyCredential)> {
        let tls = crate::utils::tls::build_tls_config().map_err(anyhow::Error::msg)?;
        let http = Client::builder()
            .read_timeout(Duration::from_secs(self.timeout_secs))
            .tls_backend_preconfigured(tls)
            .build()
            .context("创建 WorkBuddy HTTP 客户端失败")?;
        let cred = auth::get_valid_credential(&http)
            .await?
            .context("未登录 WorkBuddy：请先在「大模型管理」中完成订阅登录")?;
        Ok((http, cred))
    }

    /// 目录中该模型的推理档位（未登录/未命中时为空）。
    fn supported_efforts(&self, model: &str) -> Vec<String> {
        let Ok(guard) = self.catalog_cache.lock() else {
            return Vec::new();
        };
        let Some((entries, _)) = guard.as_ref() else {
            return Vec::new();
        };
        entries
            .iter()
            .find(|m| m.id.eq_ignore_ascii_case(model))
            .and_then(|m| m.think_efforts.as_ref())
            .map(|e| e.valid_efforts.clone())
            .unwrap_or_default()
    }

    /// 拉取模型目录（带 10 分钟缓存），供请求前的 effort 降级与启动时预热。
    async fn refresh_catalog(&self, http: &Client, cred: &WorkBuddyCredential) {
        {
            let guard = self.catalog_cache.lock().ok();
            let fresh = guard
                .as_ref()
                .and_then(|cached| cached.as_ref())
                // 空目录不视为有效缓存，允许下次重试
                .is_some_and(|(items, at)| {
                    !items.is_empty() && at.elapsed() < Duration::from_secs(600)
                });
            if fresh {
                return;
            }
        }
        if let Ok(items) = models::fetch_models(http, cred, &cred.access_token).await {
            if let Ok(mut guard) = self.catalog_cache.lock() {
                *guard = Some((items, std::time::Instant::now()));
            }
        }
    }

    /// DeepSeek 系必须显式开启 thinking 才有思维链；其他模型不带该字段。
    fn wants_thinking(&self, model: &str) -> bool {
        model.to_lowercase().starts_with("deepseek")
    }

    fn build_body(
        &self,
        cred: &WorkBuddyCredential,
        messages: &[LlmMessage],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&str>,
    ) -> Value {
        let mut msgs: Vec<Value> = Vec::new();
        for m in messages {
            match m.role.as_str() {
                // system / developer 都归一为 system（上游不认 developer）
                "system" | "developer" => {
                    msgs.push(json!({ "role": "system", "content": m.content }));
                },
                "assistant" => {
                    if !m.content.is_empty() {
                        msgs.push(json!({ "role": "assistant", "content": m.content }));
                    }
                    if let Some(calls) = &m.tool_calls {
                        msgs.push(json!({
                            "role": "assistant",
                            "tool_calls": calls.iter().map(|call| json!({
                                "id": call.id,
                                "type": "function",
                                "function": {
                                    "name": call.function.name,
                                    "arguments": call.function.arguments,
                                }
                            })).collect::<Vec<_>>(),
                            // content 允许为空字符串
                            "content": "",
                        }));
                    }
                },
                "tool" => {
                    msgs.push(json!({
                        "role": "tool",
                        "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                        "content": m.content,
                    }));
                },
                _ => {
                    msgs.push(json!({ "role": "user", "content": m.content }));
                },
            }
        }

        let mut body = json!({
            "model": self.model,
            "messages": msgs,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        if let Some(temperature) = self.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(top_p) = self.top_p {
            body["top_p"] = json!(top_p);
        }

        // 推理深度与思考链：
        // - DeepSeek 系必须显式 thinking.type=enabled 才回传思维链（与 effort 配对）；
        //   用户关闭「启用思考链」时不发这两个字段，避免上游按不支持处理。
        // - 目录声明了档位的其他模型（glm / kimi 等）本就默认推理，只按档位降级发送 effort。
        let is_deepseek = self.wants_thinking(&self.model);
        let thinking_on = !is_deepseek || self.enable_thinking;
        if thinking_on {
            let supported = self.supported_efforts(&self.model);
            if let Some(effort) = self
                .reasoning_effort
                .as_deref()
                .map(str::trim)
                .filter(|e| !e.is_empty() && !e.eq_ignore_ascii_case("off"))
            {
                let effective = downgrade_effort(effort, &supported);
                body["reasoning_effort"] = json!(effective);
            }
            if is_deepseek {
                body["thinking"] = json!({ "type": "enabled" });
            }
        }

        // prompt_cache_key：稳定派生，显著降低订阅额度消耗
        let uid_seed = if cred.uid.is_empty() {
            "anonymous".to_string()
        } else {
            cred.uid.chars().take(8).collect::<String>()
        };
        let digest = auth::sha256_hex(&format!("{}|{}", cred.uid, self.model));
        body["prompt_cache_key"] = json!(format!("lingchat-{uid_seed}-{}", &digest[..16]));

        if let Some(definitions) = tools {
            if !definitions.is_empty() {
                let converted: Vec<Value> = definitions
                    .iter()
                    .map(|d| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": d.function.name,
                                "description": d.function.description,
                                "parameters": d.function.parameters,
                            }
                        })
                    })
                    .collect();
                body["tools"] = json!(converted);
                // 上游 tool_choice 是字符串类型，对象形式会 400（code=11101）
                body["tool_choice"] = match tool_choice {
                    Some("none") => json!("none"),
                    Some("any") | Some("required") => json!("required"),
                    _ => json!("auto"),
                };
            }
        }
        body
    }

    fn chat_url(&self, cred: &WorkBuddyCredential) -> String {
        let (base, _, _) = auth::realm_endpoints(&cred.realm);
        format!("{base}/v2/chat/completions")
    }

    fn chat_headers(&self, cred: &WorkBuddyCredential) -> Result<HeaderMap> {
        let mut headers = account_headers(cred, &cred.access_token)?;
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        Ok(headers)
    }

    /// 发送请求（429/5xx 重试，最多 3 次，指数 backoff）。仅在流建立前重试。
    async fn send_with_retry(
        &self,
        http: &Client,
        cred: &WorkBuddyCredential,
        body: &Value,
    ) -> Result<reqwest::Response> {
        let headers = self.chat_headers(cred)?;
        let mut last_error = String::new();
        for attempt in 0..=MAX_RETRIES {
            let result = http
                .post(self.chat_url(cred))
                .headers(headers.clone())
                .json(body)
                .send()
                .await;
            let resp = match result {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = format!("请求发送失败: {e}");
                    if attempt < MAX_RETRIES && (e.is_connect() || e.is_timeout()) {
                        backoff_sleep(attempt).await;
                        continue;
                    }
                    return Err(anyhow!(last_error));
                },
            };
            let status = resp.status();
            if status.is_success() {
                return Ok(resp);
            }
            let text = resp.text().await.unwrap_or_default();
            last_error = format!("WorkBuddy 请求失败 ({status}): {text}");
            let retryable = matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504);
            if attempt < MAX_RETRIES && retryable {
                tracing::warn!("[WorkBuddy] {status} 第 {} 次重试", attempt + 1);
                backoff_sleep(attempt).await;
                continue;
            }
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(anyhow!(
                    "WorkBuddy 登录状态失效（{status}），请重新登录: {text}"
                ));
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
        let (http, cred) = self.client_and_credential().await?;
        self.refresh_catalog(&http, &cred).await;
        let body = self.build_body(&cred, messages, tools, tool_choice);
        crate::utils::llm_request_logger::log_request_body("workbuddy", &body);
        let resp = self.send_with_retry(&http, &cred, &body).await?;

        let stream = async_stream::try_stream! {
            // 按字节累积：SSE 分隔符 \n\n 是 ASCII，切点必落在字符边界
            let mut buffer: Vec<u8> = Vec::new();
            // 工具调用累积：index → (id, name, arguments 片段)
            let mut pending_calls: BTreeMap<u64, (String, String, String)> = BTreeMap::new();
            let mut finished_calls: Vec<ToolCall> = Vec::new();
            let mut usage: Option<LlmUsage> = None;
            let mut end_reason: Option<String> = None;
            let mut reasoning_text = String::new();
            let mut byte_stream = resp.bytes_stream();

            'outer: while let Some(item) = byte_stream.next().await {
                let bytes = item.context("读取 WorkBuddy 流失败")?;
                buffer.extend_from_slice(&bytes);
                while let Some(pos) = find_subslice(&buffer, b"\n\n") {
                    let raw_event = String::from_utf8_lossy(&buffer[..pos]).into_owned();
                    buffer.drain(..pos + 2);
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
                    if data.is_empty() {
                        continue;
                    }
                    if data == "[DONE]" {
                        break 'outer;
                    }
                    let Ok(event) = serde_json::from_str::<Value>(data) else {
                        continue;
                    };
                    // 流式错误帧原样透出
                    if let Some(err) = event.get("error") {
                        let message = err
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("未知错误");
                        Err(anyhow!("WorkBuddy 流式响应失败: {message}"))?;
                    }
                    // usage 帧：OpenAI 形态为 `choices: []` + `usage`，在 [DONE] 前到达。
                    // 必须先于 choices 判空处理，否则空数组会走进 choice 循环而漏掉用量。
                    if let Some(u) = parse_usage(&event) {
                        usage = Some(u);
                    }
                    let Some(choices) = event.get("choices").and_then(Value::as_array) else {
                        continue;
                    };
                    for choice in choices {
                        let Some(delta) = choice.get("delta") else {
                            continue;
                        };
                        if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
                            if !reasoning.is_empty() {
                                reasoning_text.push_str(reasoning);
                                yield LlmChunk::Reasoning(reasoning.to_string());
                            }
                        }
                        if let Some(content) = delta.get("content").and_then(Value::as_str) {
                            if !content.is_empty() {
                                yield LlmChunk::Content(content.to_string());
                            }
                        }
                        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
                            for call in calls {
                                let index = call.get("index").and_then(Value::as_u64).unwrap_or(0);
                                let entry = pending_calls.entry(index).or_default();
                                if let Some(id) = call.get("id").and_then(Value::as_str) {
                                    if !id.is_empty() {
                                        entry.0 = id.to_string();
                                    }
                                }
                                if let Some(function) = call.get("function") {
                                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                                        if !name.is_empty() {
                                            entry.1 = name.to_string();
                                        }
                                    }
                                    if let Some(args) = function.get("arguments").and_then(Value::as_str) {
                                        entry.2.push_str(args);
                                        yield LlmChunk::ToolCallProgress {
                                            name: entry.1.clone(),
                                            chars: entry.2.len(),
                                        };
                                    }
                                }
                            }
                        }
                        if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str) {
                            if !finish.is_empty() && finish != "null" {
                                end_reason = Some(
                                    match finish {
                                        "length" => "max_tokens".to_string(),
                                        "tool_calls" | "function_call" => "tool_calls".to_string(),
                                        other => other.to_string(),
                                    }
                                );
                            }
                        }
                    }
                }
            }

            // 汇总工具调用（finish_reason 为 tool_calls 或流自然结束都有效）
            for (_, (id, name, args)) in pending_calls {
                if name.is_empty() {
                    continue;
                }
                // arguments 为空时补合法 JSON
                let arguments = if args.trim().is_empty() {
                    "{}".to_string()
                } else {
                    args
                };
                finished_calls.push(ToolCall {
                    id,
                    type_: "function".to_string(),
                    function: FunctionCall { name, arguments },
                });
            }

            if !reasoning_text.is_empty() {
                tracing::info!("[WorkBuddy Thinking] {}", reasoning_text);
            }
            if let Some(u) = usage {
                tracing::info!(
                    "[WorkBuddy] usage: prompt={} completion={} cached={}",
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

    /// 非流式 = 流式收集拼接（上游只接受 stream:true）。
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
                _ => {},
            }
        }
        Ok((content, calls, usage))
    }
}

async fn backoff_sleep(attempt: usize) {
    let base = 1000u64 << attempt.min(5);
    let capped = base.min(30_000);
    tokio::time::sleep(Duration::from_millis(capped)).await;
}

/// 在字节序列中查找子序列首次出现的位置（防跨分块 UTF-8 截断，同 Codex provider）。
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// 解析 SSE 事件里的 token 用量。
///
/// 上游 usage 帧形如 `{"choices":[],"usage":{...}}`：空 `choices` 数组是正常形态，
/// 因此用量必须独立于 choices 解析。缓存命中字段用 `prompt_cache_hit_tokens`。
fn parse_usage(event: &Value) -> Option<LlmUsage> {
    let u = event.get("usage").filter(|value| !value.is_null())?;
    Some(LlmUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
        completion_tokens: u
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: u.get("total_tokens").and_then(Value::as_u64).unwrap_or(0),
        cached_tokens: u
            .get("prompt_cache_hit_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

#[async_trait]
impl LlmProvider for WorkBuddyProvider {
    async fn list_models(&self, _http: &Client) -> Result<Vec<LlmModelInfo>> {
        let (http, cred) = self.client_and_credential().await?;
        let items = models::fetch_models(&http, &cred, &cred.access_token).await?;
        if let Ok(mut guard) = self.catalog_cache.lock() {
            *guard = Some((items.clone(), std::time::Instant::now()));
        }
        Ok(items)
    }

    async fn complete(&self, _http: &Client, messages: &[LlmMessage]) -> Result<String> {
        let (content, _, _) = self.collect(messages, None, None).await?;
        Ok(content)
    }

    async fn complete_stream(
        &self,
        _http: &Client,
        messages: &[LlmMessage],
    ) -> Result<ChunkStream> {
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
            content: if content.is_empty() {
                None
            } else {
                Some(content)
            },
            tool_calls: if calls.is_empty() { None } else { Some(calls) },
            usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(model: &str) -> WorkBuddyProvider {
        WorkBuddyProvider::from_config(&LlmConfig {
            provider: "workbuddy".to_string(),
            model: model.to_string(),
            api_key: String::new(),
            base_url: String::new(),
            timeout_secs: 60,
            temperature: None,
            top_p: None,
            enable_thinking: false,
            reasoning_effort: None,
            fast_mode: false,
            support_vision: false,
        })
        .expect("provider")
    }

    #[test]
    fn effort_downgrade_picks_closest_supported() {
        let supported: Vec<String> = vec!["low".into(), "high".into(), "max".into()];
        assert_eq!(downgrade_effort("high", &supported), "high");
        assert_eq!(downgrade_effort("xhigh", &supported), "high");
        assert_eq!(downgrade_effort("max", &supported), "max");
        assert_eq!(downgrade_effort("minimal", &supported), "low");
        // 全部高于请求档 → 最低档
        let only_high: Vec<String> = vec!["high".into()];
        assert_eq!(downgrade_effort("low", &only_high), "high");
        // 未知档位原样透传
        assert_eq!(downgrade_effort("turbo", &only_high), "turbo");
        // 空支持列表原样
        assert_eq!(downgrade_effort("high", &[]), "high");
    }

    #[test]
    fn deepseek_models_enable_thinking() {
        let deepseek = provider("deepseek-v4.1-flash");
        assert!(deepseek.wants_thinking("deepseek-v4.1-flash"));
        assert!(!deepseek.wants_thinking("glm-5.2"));
        assert!(!provider("glm-5.2").wants_thinking("auto"));
    }

    #[test]
    fn body_is_stream_only_with_cache_key() {
        let p = provider("deepseek-v4.1-flash");
        let p = WorkBuddyProvider {
            reasoning_effort: Some("medium".to_string()),
            enable_thinking: true,
            ..p
        };
        let cred = workbuddy_cred();
        let messages = vec![LlmMessage::user("你好")];
        let body = p.build_body(&cred, &messages, None, None);
        assert_eq!(body["stream"], json!(true));
        assert_eq!(body["model"], json!("deepseek-v4.1-flash"));
        assert_eq!(body["thinking"], json!({ "type": "enabled" }));
        // prompt_cache_key: lingchat-<uid前8>-<hash16>
        let key = body["prompt_cache_key"].as_str().expect("cache key");
        assert!(key.starts_with("lingchat-testuid-"));
        assert_eq!(key.len(), "lingchat-".len() + 8 + 1 + 16);
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn deepseek_without_thinking_omits_reasoning_fields() {
        // 关闭「启用思考链」时不发 thinking / reasoning_effort，避免上游 400
        let p = provider("deepseek-v4.1-flash");
        let p = WorkBuddyProvider {
            reasoning_effort: Some("high".to_string()),
            ..p
        };
        let cred = workbuddy_cred();
        let body = p.build_body(&cred, &[LlmMessage::user("hi")], None, None);
        assert!(body.get("thinking").is_none());
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn non_deepseek_models_send_effort_without_thinking_field() {
        // 目录声明档位的非 DeepSeek 模型（glm / kimi 系）：只发 effort
        let p = provider("glm-5.2");
        let p = WorkBuddyProvider {
            reasoning_effort: Some("xhigh".to_string()),
            ..p
        };
        let cred = workbuddy_cred();
        let body = p.build_body(&cred, &[LlmMessage::user("hi")], None, None);
        assert_eq!(body["reasoning_effort"], json!("xhigh"));
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn tool_choice_is_string_not_object() {
        let p = provider("auto");
        let cred = workbuddy_cred();
        let messages = vec![LlmMessage::user("hi")];
        let definitions = vec![ToolDefinition::new(
            "search",
            "搜索",
            serde_json::json!({ "type": "object", "properties": {} }),
        )];
        let body = p.build_body(&cred, &messages, Some(&definitions), Some("auto"));
        assert_eq!(body["tool_choice"], json!("auto"));
        assert_eq!(body["tools"][0]["function"]["name"], json!("search"));

        let body = p.build_body(&cred, &messages, Some(&definitions), Some("any"));
        assert_eq!(body["tool_choice"], json!("required"));
    }

    #[test]
    fn messages_normalize_roles() {
        let p = provider("auto");
        let cred = workbuddy_cred();
        let messages = vec![LlmMessage::system("系统提示"), LlmMessage::user("问题")];
        let body = p.build_body(&cred, &messages, None, None);
        let msgs = body["messages"].as_array().expect("messages");
        assert_eq!(msgs[0]["role"], json!("system"));
        assert_eq!(msgs[1]["role"], json!("user"));
    }

    #[test]
    fn usage_frame_with_empty_choices_is_parsed() {
        // 上游 usage 帧形态：choices 为空数组 + usage（不能被空数组短路掉）
        let event = json!({
            "id": "chatcmpl-1",
            "choices": [],
            "usage": {
                "prompt_tokens": 7808,
                "completion_tokens": 20,
                "total_tokens": 7828,
                "prompt_cache_hit_tokens": 7808,
                "credit": 0.02
            }
        });
        let usage = parse_usage(&event).expect("usage");
        assert_eq!(usage.prompt_tokens, 7808);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 7828);
        assert_eq!(usage.cached_tokens, 7808);

        // 普通内容帧没有 usage
        assert!(parse_usage(&json!({ "choices": [{ "delta": { "content": "hi" } }] })).is_none());
        // usage 为 null 时不产生用量
        assert!(parse_usage(&json!({ "choices": [], "usage": null })).is_none());
    }

    // —— 测试辅助：直接构造凭据而非依赖登录 ——
    fn workbuddy_cred() -> WorkBuddyCredential {
        WorkBuddyCredential {
            access_token: "test".to_string(),
            refresh_token: String::new(),
            expires_at: 0,
            uid: "testuid-12345678".to_string(),
            nickname: String::new(),
            enterprise_id: String::new(),
            domain: String::new(),
            realm: "cn".to_string(),
        }
    }
}
