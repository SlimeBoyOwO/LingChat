//! 网页搜索工具，两种后端模式：
//!
//! 1. 模型 API 内置联网（默认，`use_builtin = true`）：复用用户已配置的聊天模型
//!    API（Moonshot/Kimi 的 OpenAI 兼容端点），声明 `$web_search` 内置工具，
//!    由服务端执行搜索。协议（见 platform.moonshot.cn/docs/guide/use-web-search）：
//!    模型返回 tool_calls 后，客户端把参数原样回传为 tool 消息，服务端继续生成最终答案。
//!    无需单独的搜索 API Key。
//! 2. 独立搜索端点（`use_builtin = false`）：直接 POST Moonshot `/search` 端点，
//!    需要单独的 API Key。参考 kimi-code 的 WebSearch 设计（极简 query 参数、
//!    纯文本结果、错误分类成模型可读文本）。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use crate::ai_service::llm::provider_config::resolve_chat_provider;
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::settings::{SharedToolSettings, WebSearchSettings};

/// 独立端点模式的执行超时。
const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
/// 内置联网模式的执行超时（服务端要跑一轮 LLM 生成 + 搜索，更慢）。
const BUILTIN_TIMEOUT: Duration = Duration::from_secs(90);
/// 返回给模型的结果文本总量上限，避免把上下文塞爆。
const MAX_OUTPUT_CHARS: usize = 20_000;
/// 内置联网模式的最大 tool_calls 回显轮次。
const MAX_BUILTIN_ROUNDS: usize = 3;

/// 网页搜索内置工具。
pub struct WebSearchTool {
    settings: SharedToolSettings,
    /// 用于在内置联网模式下解析当前聊天模型配置。
    app: tauri::AppHandle,
}

impl WebSearchTool {
    pub fn new(settings: SharedToolSettings, app: tauri::AppHandle) -> Self {
        Self { settings, app }
    }

    /// 构建带统一 TLS 配置（webpki-roots，绕开 platform-verifier）的 HTTP 客户端。
    /// 与 `llm/factory.rs::build_http_client` 保持一致；按需叠加代理。
    fn build_client(cfg: &WebSearchSettings) -> Result<Client, ToolError> {
        let mut roots = rustls::RootCertStore::empty();
        roots.roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| ToolError::Execution(format!("rustls 协议版本配置失败: {e}")))?
        .with_root_certificates(Arc::new(roots))
        .with_no_client_auth();

        let mut builder = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .tls_backend_preconfigured(tls_config);

        // 显式配置的代理优先；未配置时回退到环境变量（与 TTS 适配器行为一致）
        let proxy_url = if cfg.proxy_enabled && !cfg.proxy_addr.trim().is_empty() {
            Some(cfg.proxy_addr.trim().to_string())
        } else if !cfg.proxy_enabled {
            std::env::var("HTTPS_PROXY")
                .or_else(|_| std::env::var("https_proxy"))
                .or_else(|_| std::env::var("HTTP_PROXY"))
                .or_else(|_| std::env::var("http_proxy"))
                .ok()
        } else {
            None
        };
        if let Some(url) = proxy_url {
            match reqwest::Proxy::all(&url) {
                Ok(proxy) => builder = builder.proxy(proxy),
                Err(e) => tracing::warn!("搜索代理地址无效，已忽略: {url} ({e})"),
            }
        }

        builder
            .build()
            .map_err(|e| ToolError::Execution(format!("创建搜索 HTTP 客户端失败: {e}")))
    }

    /// 把搜索结果渲染成模型友好的纯文本（独立端点模式）。
    /// `hide = true` 时不输出网址/来源名，并改为指示模型自然融入回答，
    /// 避免模型在对话里念出搜索结果列表。
    fn format_results(query: &str, results: &[Value], max_results: usize, hide: bool) -> String {
        let mut out = String::new();
        for item in results.iter().take(max_results) {
            let get = |key: &str| item.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string();
            let title = get("title");
            let url = get("url");
            if title.is_empty() && url.is_empty() {
                continue;
            }
            out.push_str(&format!("Title: {title}\n"));
            if !hide {
                let site = get("site_name");
                if !site.is_empty() {
                    out.push_str(&format!("Site: {site}\n"));
                }
            }
            let date = get("date");
            if !date.is_empty() {
                out.push_str(&format!("Date: {date}\n"));
            }
            if !hide {
                out.push_str(&format!("URL: {url}\n"));
            }
            // kimi coding /search 的结果 snippet 可能为空但 content 很长，兜底并截断
            let mut snippet = get("snippet");
            if snippet.is_empty() {
                snippet = get("content");
            }
            if snippet.chars().count() > 800 {
                snippet = snippet.chars().take(800).collect();
                snippet.push('…');
            }
            if !snippet.is_empty() {
                out.push_str(&format!("Snippet: {snippet}\n"));
            }
            out.push_str("\n---\n\n");
        }
        if out.is_empty() {
            return format!("No search results found for: {query}");
        }
        if hide {
            out.push_str(
                "以上是联网搜索到的信息。请把关键内容自然地融入你的回答，\
                 绝对不要在回复中输出来源名称、网址、链接列表或原始搜索结果。\n",
            );
        } else {
            out.push_str(
                "以上是联网搜索到的摘要。回答时请基于这些信息，并以 Markdown 链接形式标注来源，例如 [标题](URL)。\n",
            );
        }
        truncate_output(&mut out);
        out
    }

    /// 独立搜索端点模式：按 provider 分发（kimi / bocha）。
    async fn execute_search_endpoint(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        if cfg.api_key.trim().is_empty() {
            return Err(ToolError::Execution(
                "网页搜索未配置 API Key，请用户在「高级设置 → 工具配置」填写，或改用「模型 API 内置联网」模式".into(),
            ));
        }
        match cfg.provider.as_str() {
            "bocha" => self.execute_bocha_search(query, cfg).await,
            _ => self.execute_kimi_endpoint(query, cfg).await,
        }
    }

    /// 独立端点模式 · Kimi 系 /search（body 为 text_query）。
    async fn execute_kimi_endpoint(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        let base_url = if cfg.base_url.trim().is_empty() {
            "https://api.kimi.com/coding/v1/search"
        } else {
            cfg.base_url.trim()
        };
        let client = Self::build_client(cfg)?;
        let response = client
            .post(base_url)
            // kimi coding 搜索端点对 UA 有白名单；对其他服务无副作用
            .header(reqwest::header::USER_AGENT, "claude-code/2.0.0")
            .bearer_auth(cfg.api_key.trim())
            .json(&serde_json::json!({ "text_query": query }))
            .send()
            .await
            .map_err(classify_request_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Execution(http_error_message(status, response).await));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("搜索结果解析失败: {e}")))?;
        let results = payload
            .get("search_results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let text = Self::format_results(query, &results, cfg.max_results.max(1), cfg.hide_search_results);
        Ok(serde_json::json!({
            "ok": true,
            "query": query,
            "result_count": results.len().min(cfg.max_results.max(1)),
            "text": text,
        }))
    }

    /// 独立端点模式 · BoCha 博查（参考 AstrBot 的 web_search_bocha 实现）。
    async fn execute_bocha_search(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        let base_url = if cfg.base_url.trim().is_empty() {
            "https://api.bochaai.com/v1/web-search"
        } else {
            cfg.base_url.trim()
        };
        let client = Self::build_client(cfg)?;
        let response = client
            .post(base_url)
            .bearer_auth(cfg.api_key.trim())
            .json(&serde_json::json!({
                "query": query,
                "count": cfg.max_results.max(1),
                "summary": true,
            }))
            .send()
            .await
            .map_err(classify_request_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Execution(http_error_message(status, response).await));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("搜索结果解析失败: {e}")))?;
        let rows = payload
            .pointer("/data/webPages/value")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        // 统一成 format_results 认识的字段（summary 比 snippet 更完整，优先）
        let results: Vec<Value> = rows
            .iter()
            .map(|item| {
                let get = |key: &str| item.get(key).and_then(Value::as_str).unwrap_or("");
                serde_json::json!({
                    "title": get("name"),
                    "url": get("url"),
                    "site_name": get("siteName"),
                    "date": get("datePublished"),
                    "snippet": if get("summary").is_empty() { get("snippet") } else { get("summary") },
                })
            })
            .collect();

        let text = Self::format_results(query, &results, cfg.max_results.max(1), cfg.hide_search_results);
        Ok(serde_json::json!({
            "ok": true,
            "query": query,
            "result_count": results.len().min(cfg.max_results.max(1)),
            "text": text,
        }))
    }

    /// kimicode 模式的联网搜索：复用聊天配置里的 kimi key，
    /// 客户端直连 `{base_url}/v1/search`（与 Kimi Code CLI 相同的通道）。
    async fn execute_kimicode_search(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
        provider: &crate::ai_service::llm::provider_config::LlmProviderConfig,
    ) -> Result<ToolResult, ToolError> {
        let base = provider.base_url.trim().trim_end_matches('/');
        let base = if base.is_empty() {
            "https://api.kimi.com/coding"
        } else {
            base
        };
        let endpoint = if base.ends_with("/v1") {
            format!("{base}/search")
        } else {
            format!("{base}/v1/search")
        };

        let client = Self::build_client(cfg)?;
        let response = client
            .post(&endpoint)
            // coding 端点对 UA 有白名单，沿用 KimiCodeProvider 的伪装约定
            .header(reqwest::header::USER_AGENT, "claude-code/2.0.0")
            .bearer_auth(provider.api_key.trim())
            .json(&serde_json::json!({ "text_query": query }))
            .send()
            .await
            .map_err(classify_request_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Execution(http_error_message(status, response).await));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("搜索结果解析失败: {e}")))?;
        let results = payload
            .get("search_results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let text = Self::format_results(query, &results, cfg.max_results.max(1), cfg.hide_search_results);
        Ok(serde_json::json!({
            "ok": true,
            "query": query,
            "result_count": results.len().min(cfg.max_results.max(1)),
            "text": text,
        }))
    }

    /// 模型 API 内置联网模式：声明 `$web_search`，按协议回显 tool_calls 参数。
    async fn execute_builtin(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        let provider = resolve_chat_provider(&self.app).ok_or_else(|| {
            ToolError::Execution(
                "未找到可用的聊天模型配置，请先在「通用 → 文本」设置里配置 LLM".into(),
            )
        })?;
        if provider.provider.eq_ignore_ascii_case("kimicode") {
            // kimicode（Anthropic 协议）不支持 $web_search 内置工具，
            // 但 api.kimi.com/coding 提供独立的 /v1/search 端点（Kimi Code CLI 同款），
            // 复用聊天 Key 客户端直连即可。
            return self.execute_kimicode_search(query, cfg, &provider).await;
        }

        let base = if provider.base_url.trim().is_empty() {
            "https://api.moonshot.cn/v1".to_string()
        } else {
            provider.base_url.trim().trim_end_matches('/').to_string()
        };
        let endpoint = if base.ends_with("/chat/completions") {
            base
        } else {
            format!("{base}/chat/completions")
        };

        let client = Self::build_client(cfg)?;
        let system_prompt = if cfg.hide_search_results {
            "你是联网搜索助手。需要时使用 $web_search 工具获取信息，\
             然后把关键内容自然地融入回答，绝对不要输出来源名称、网址或链接列表。"
        } else {
            "你是联网搜索助手。需要时使用 $web_search 工具获取信息，\
             然后用简洁的中文总结搜索结果，保留关键事实与来源链接。"
        };
        let mut messages = serde_json::json!([
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": query }
        ]);
        let tools = serde_json::json!([
            { "type": "builtin_function", "function": { "name": "$web_search" } }
        ]);

        for round in 0..MAX_BUILTIN_ROUNDS {
            let body = serde_json::json!({
                "model": provider.model,
                "messages": messages,
                "tools": tools,
                // kimi-k2 系列官方建议值；对其他模型无副作用
                "temperature": 0.6,
                "stream": false,
            });
            let response = client
                .post(&endpoint)
                .bearer_auth(provider.api_key.trim())
                .json(&body)
                .send()
                .await
                .map_err(classify_request_error)?;

            let status = response.status();
            if !status.is_success() {
                return Err(ToolError::Execution(http_error_message(status, response).await));
            }

            let payload: Value = response
                .json()
                .await
                .map_err(|e| ToolError::Execution(format!("搜索响应解析失败: {e}")))?;
            let Some(message) = payload
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("message"))
            else {
                return Err(ToolError::Execution("搜索响应缺少 choices[0].message".into()));
            };

            let tool_calls = message.get("tool_calls").and_then(Value::as_array);
            if let Some(calls) = tool_calls.filter(|c| !c.is_empty()) {
                // $web_search 协议：服务端执行搜索，客户端只需把参数原样回传
                tracing::info!(round = round + 1, "内置联网：回显 $web_search tool_calls");
                let messages_arr = messages.as_array_mut().expect("messages 必须是数组");
                messages_arr.push(message.clone());
                for call in calls {
                    let id = call.get("id").and_then(Value::as_str).unwrap_or_default();
                    let arguments = call
                        .pointer("/function/arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}");
                    messages_arr.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": id,
                        "content": arguments,
                    }));
                }
                continue;
            }

            let mut content = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if content.is_empty() {
                return Err(ToolError::Execution("搜索服务未返回有效内容".into()));
            }
            truncate_output(&mut content);
            return Ok(serde_json::json!({
                "ok": true,
                "query": query,
                "text": content,
            }));
        }

        Err(ToolError::Execution(format!(
            "内置联网搜索超过 {MAX_BUILTIN_ROUNDS} 轮仍未返回结果"
        )))
    }
}

/// 限制返回给模型的文本长度。
fn truncate_output(text: &mut String) {
    if text.chars().count() > MAX_OUTPUT_CHARS {
        *text = text.chars().take(MAX_OUTPUT_CHARS).collect();
        text.push_str("\n[...结果过长已截断]");
    }
}

/// 把 reqwest 发送错误分类成模型可读的文本。
fn classify_request_error(e: reqwest::Error) -> ToolError {
    let msg = if e.is_timeout() {
        format!("搜索请求超时: {e}")
    } else if e.is_connect() {
        format!("无法连接搜索服务（如开启代理请检查代理是否在运行）: {e}")
    } else {
        format!("搜索请求失败: {e}")
    };
    ToolError::Execution(msg)
}

/// 把 HTTP 错误状态分类成模型可读的文本。
async fn http_error_message(status: reqwest::StatusCode, response: reqwest::Response) -> String {
    let body = response.text().await.unwrap_or_default();
    let body: String = body.chars().take(300).collect();
    match status.as_u16() {
        401 | 403 => format!("搜索服务认证失败，请检查 API Key 是否正确（HTTP {status}）"),
        429 => "搜索服务请求过于频繁，请稍后再试（HTTP 429）".to_string(),
        _ => format!("搜索服务返回错误（HTTP {status}）: {body}"),
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "web_search",
            "联网搜索网页信息。当用户询问新闻时事、你不确定的事实、或明确要求查资料时使用。\
             返回联网搜索得到的摘要，回答时必须以来源链接标注出处。",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索关键词"
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(if self.settings.get().web_search.use_builtin {
            BUILTIN_TIMEOUT
        } else {
            SEARCH_TIMEOUT
        })
    }

    async fn execute(&self, _: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let cfg = self.settings.get().web_search;
        if !cfg.enabled {
            return Err(ToolError::Execution(
                "网页搜索未启用，请用户在「日程 → 工具调用」设置页开启".into(),
            ));
        }

        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("缺少必填参数 query".into()))?
            .to_string();

        if cfg.use_builtin {
            self.execute_builtin(&query, &cfg).await
        } else {
            self.execute_search_endpoint(&query, &cfg).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证结果渲染格式与条数上限。
    #[test]
    fn formats_results_as_plain_text() {
        let results = vec![
            serde_json::json!({
                "title": "标题一",
                "url": "https://example.com/1",
                "site_name": "Example",
                "date": "2026-08-01",
                "snippet": "摘要一",
            }),
            serde_json::json!({
                "title": "标题二",
                "url": "https://example.com/2",
                "snippet": "摘要二",
            }),
            serde_json::json!({}),
        ];
        let text = WebSearchTool::format_results("测试", &results, 2, false);
        assert!(text.contains("Title: 标题一"));
        assert!(text.contains("Site: Example"));
        assert!(text.contains("URL: https://example.com/2"));
        assert!(text.contains("---"));
        assert!(text.contains("标注来源"));
    }

    /// 验证隐藏模式不含网址/来源名，且指示模型自然融入回答。
    #[test]
    fn hide_mode_strips_sources_and_urls() {
        let results = vec![serde_json::json!({
            "title": "标题一",
            "url": "https://example.com/1",
            "site_name": "Example",
            "date": "2026-08-01",
            "snippet": "摘要一",
        })];
        let text = WebSearchTool::format_results("测试", &results, 8, true);
        assert!(text.contains("Title: 标题一"));
        assert!(text.contains("Snippet: 摘要一"));
        assert!(!text.contains("example.com"));
        assert!(!text.contains("Site:"));
        assert!(text.contains("自然"));
        assert!(!text.contains("标注来源"));
    }

    /// 验证空结果返回明确提示。
    #[test]
    fn empty_results_yield_not_found_message() {
        let text = WebSearchTool::format_results("没有结果", &[], 8, false);
        assert!(text.contains("No search results found"));
    }

    /// 验证超长输出会被截断并标注。
    #[test]
    fn truncates_overlong_output() {
        let mut text = "一".repeat(MAX_OUTPUT_CHARS + 100);
        truncate_output(&mut text);
        assert!(text.contains("已截断"));
        assert!(text.chars().count() <= MAX_OUTPUT_CHARS + 20);
    }
}
