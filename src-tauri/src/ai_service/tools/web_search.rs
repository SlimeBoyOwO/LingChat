//! 网页搜索工具：直接 POST 独立搜索端点（需要单独配置 API Key）。
//! 支持 Kimi /search、BoCha、DeepSeek Responses API（服务端内置 `web_search`）、
//! Tavily 与自定义兼容端点。

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use crate::ai_service::llm::provider_config::{LlmProviderConfig, resolve_chat_provider};
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::settings::{SharedToolSettings, WebSearchSettings};

/// 独立端点模式的执行超时。
const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
/// DeepSeek Responses API 执行超时（服务端需要跑一轮模型 + 搜索，更慢）。
const DEEPSEEK_TIMEOUT: Duration = Duration::from_secs(45);
/// 返回给模型的结果文本总量上限，避免把上下文塞爆。
const MAX_OUTPUT_CHARS: usize = 20_000;
/// 搜索词长度上限，避免异常参数放大请求体、日志与第三方计费。
const MAX_QUERY_CHARS: usize = 500;

/// 网页搜索内置工具。
pub struct WebSearchTool {
    settings: SharedToolSettings,
}

impl WebSearchTool {
    pub fn new(settings: SharedToolSettings) -> Self {
        Self { settings }
    }

    fn tool_definition(cfg: &WebSearchSettings) -> ToolDefinition {
        let description = if cfg.hide_search_results {
            "联网搜索网页信息。当用户询问新闻时事、你不确定的事实、或明确要求查资料时使用。\
             返回内容已按用户设置隐藏来源与网址，请把事实自然融入回答，不要编造或输出链接。"
        } else {
            "联网搜索网页信息。当用户询问新闻时事、你不确定的事实、或明确要求查资料时使用。\
             返回联网搜索得到的摘要，回答时必须以来源链接标注出处。"
        };
        ToolDefinition::new(
            "web_search",
            description,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索关键词",
                        "maxLength": MAX_QUERY_CHARS
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        )
    }

    /// 构建带统一 TLS 配置（webpki-roots，绕开 platform-verifier）的 HTTP 客户端。
    /// 与 `llm/factory.rs::build_http_client` 保持一致；按需叠加代理。
    fn build_client(cfg: &WebSearchSettings) -> Result<Client, ToolError> {
        let mut roots = rustls::RootCertStore::empty();
        roots
            .roots
            .extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
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

    /// 解析执行时凭据。Codex 使用 OAuth；Kimi 可复用当前官方 Kimi Code
    /// 对话模型凭据，与 Kimi Code CLI 的 host WebSearch + `/search` 闭环一致。
    fn resolve_execution_settings(
        context: &ToolContext,
        cfg: &WebSearchSettings,
    ) -> Result<WebSearchSettings, ToolError> {
        let mut effective = cfg.clone();
        effective.provider = effective.provider.trim().to_ascii_lowercase();
        if effective.provider == "kimi" && effective.api_key.trim().is_empty() {
            let app = context.require_app()?;
            let chat = resolve_chat_provider(&app).ok_or_else(|| {
                ToolError::Execution(
                    "Kimi 搜索未填写独立 API Key，且当前没有可复用的对话模型凭据".into(),
                )
            })?;
            if !chat.provider.eq_ignore_ascii_case("kimicode") {
                return Err(ToolError::Execution(
                    "Kimi 搜索未填写独立 API Key；请把当前对话模型切换为官方 Kimi Code，或在工具设置中填写 API Key".into(),
                ));
            }
            if !is_official_kimi_code_base_url(&chat.base_url) {
                return Err(ToolError::Execution(
                    "为避免把对话凭据发送到不同服务，只有官方 api.kimi.com/coding 端点可复用 Kimi Code 凭据；自定义端点请单独填写 API Key".into(),
                ));
            }
            if chat.api_key.trim().is_empty() {
                return Err(ToolError::Execution(
                    "当前 Kimi Code 对话模型没有可复用的 API Key".into(),
                ));
            }
            effective.api_key = chat.api_key;
        }
        if effective.api_key.trim().is_empty() && effective.provider != "codex" {
            return Err(ToolError::Execution(
                "网页搜索未配置 API Key，请用户在「高级设置 → 工具配置」填写".into(),
            ));
        }
        Ok(effective)
    }

    /// 把搜索结果渲染成模型友好的纯文本（独立端点模式）。
    /// `hide = true` 时不输出网址/来源名，并改为指示模型自然融入回答，
    /// 避免模型在对话里念出搜索结果列表。
    fn format_results(query: &str, results: &[Value], max_results: usize, hide: bool) -> String {
        let mut out = String::new();
        for item in results.iter().take(max_results) {
            let get = |key: &str| {
                item.get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string()
            };
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

    /// 独立搜索端点模式：按 provider 分发（kimi / bocha / deepseek / tavily / codex / custom）。
    async fn execute_search_endpoint(
        &self,
        context: &ToolContext,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        match cfg.provider.as_str() {
            "bocha" => self.execute_bocha_search(query, cfg).await,
            "deepseek" => self.execute_deepseek_search(query, cfg).await,
            "tavily" => self.execute_tavily_search(query, cfg).await,
            "codex" => self.execute_codex_search(context, query, cfg).await,
            "kimi" | "custom" => self.execute_kimi_endpoint(query, cfg).await,
            provider => Err(ToolError::Execution(format!(
                "不支持的网页搜索提供商: {provider}"
            ))),
        }
    }

    /// 独立端点模式 · Kimi 系 /search（body 为 text_query）。
    /// "kimi" 固定用官方端点；"custom" 使用用户填写的 base_url。
    async fn execute_kimi_endpoint(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        let base_url = if cfg.provider == "custom" {
            let url = cfg.base_url.trim();
            if url.is_empty() {
                return Err(ToolError::Execution(
                    "自定义端点模式需要填写搜索服务地址".into(),
                ));
            }
            url
        } else {
            "https://api.kimi.com/coding/v1/search"
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
            return Err(ToolError::Execution(
                http_error_message(status, response).await,
            ));
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

        let text = Self::format_results(
            query,
            &results,
            cfg.max_results.max(1),
            cfg.hide_search_results,
        );
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
        let base_url = "https://api.bochaai.com/v1/web-search";
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
            return Err(ToolError::Execution(
                http_error_message(status, response).await,
            ));
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

        let text = Self::format_results(
            query,
            &results,
            cfg.max_results.max(1),
            cfg.hide_search_results,
        );
        Ok(serde_json::json!({
            "ok": true,
            "query": query,
            "result_count": results.len().min(cfg.max_results.max(1)),
            "text": text,
        }))
    }

    /// 独立端点模式 · Tavily（https://api.tavily.com/search）。
    ///
    /// Tavily 只认顶层的 `query` 字段。此前没有这个分支，选 Tavily 的配置会落进
    /// 上面 match 的 Kimi 兜底、发出 `text_query`，Tavily 便返回 422（issue #630）。
    /// 注意 `text_query` 是 Kimi 官方端点要求的格式，不能反过来去改那一侧。
    async fn execute_tavily_search(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        let base_url = "https://api.tavily.com/search";
        let client = Self::build_client(cfg)?;
        let response = client
            .post(base_url)
            .bearer_auth(cfg.api_key.trim())
            .json(&serde_json::json!({
                "query": query,
                "max_results": cfg.max_results.max(1),
            }))
            .send()
            .await
            .map_err(classify_request_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Execution(
                http_error_message(status, response).await,
            ));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("搜索结果解析失败: {e}")))?;
        let rows = payload
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        // 统一成 format_results 认识的字段：Tavily 的正文在 content，
        // 没有站点名和日期，留空即可（format_results 会跳过空字段）。
        let results: Vec<Value> = rows
            .iter()
            .map(|item| {
                let get = |key: &str| item.get(key).and_then(Value::as_str).unwrap_or("");
                serde_json::json!({
                    "title": get("title"),
                    "url": get("url"),
                    "snippet": get("content"),
                })
            })
            .collect();

        let text = Self::format_results(
            query,
            &results,
            cfg.max_results.max(1),
            cfg.hide_search_results,
        );
        Ok(serde_json::json!({
            "ok": true,
            "query": query,
            "result_count": results.len().min(cfg.max_results.max(1)),
            "text": text,
        }))
    }

    /// 独立端点模式 · OpenAI Codex 订阅联网搜索。
    ///
    /// 协议与 dsh-codex 的 standalone search 一致：
    /// `POST https://chatgpt.com/backend-api/codex/alpha/search`，复用
    /// `codex-auth.json` 的 OAuth 凭据（设备码登录、自动刷新），链路经
    /// `utils::proxy` 自动探测代理。响应 `output` 为模型综合答案，
    /// `results[]` 为 text_result（url/title/snippet）。
    async fn execute_codex_search(
        &self,
        context: &ToolContext,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        use crate::ai_service::llm::codex_auth;
        use crate::utils::proxy::build_proxied_client;

        const SEARCH_URL: &str = "https://chatgpt.com/backend-api/codex/alpha/search";

        let http = build_proxied_client(45)
            .await
            .map_err(|e| ToolError::Execution(format!("创建 Codex 搜索客户端失败: {e}")))?;
        let cred = codex_auth::get_valid_credential(&http)
            .await
            .map_err(|e| ToolError::Execution(format!("Codex 凭据读取失败: {e}")))?
            .ok_or_else(|| {
                ToolError::Execution(
                    "未登录 Codex：请先在「大模型管理」登录 ChatGPT 订阅，或改用其他搜索提供商"
                        .into(),
                )
            })?;

        let chat_provider = context.app.as_ref().and_then(resolve_chat_provider);
        let model = codex_search_model(chat_provider.as_ref());
        let body = serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "model": model,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": query }],
            }],
            "commands": { "search_query": [{ "q": query }] },
            "settings": {
                "search_context_size": "medium",
                "allowed_callers": ["direct"],
                // live：实时联网（搜索工具的语义就是查最新资料）
                "external_web_access": true,
            },
            "max_output_tokens": 10000,
        });

        let response = http
            .post(SEARCH_URL)
            .bearer_auth(&cred.access)
            .header("chatgpt-account-id", &cred.account_id)
            .header("originator", "lingchat")
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&body)
            .send()
            .await
            .map_err(classify_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let message = if matches!(status.as_u16(), 401 | 403) {
                format!(
                    "Codex 搜索认证失败，请在「大模型管理」重新登录 ChatGPT 订阅（HTTP {status}）"
                )
            } else {
                http_error_message(status, response).await
            };
            return Err(ToolError::Execution(message));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("Codex 搜索响应解析失败: {e}")))?;

        let answer = payload
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let rows = payload
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let results = normalize_codex_results(&rows);

        // 主答案（模型综合回答）+ 来源条目（hide 时省略并改为融入指示）
        let mut text = String::new();
        if !answer.is_empty() {
            text.push_str(&answer);
            text.push_str("\n\n");
        }
        if !cfg.hide_search_results {
            text.push_str(&Self::format_results(
                query,
                &results,
                cfg.max_results.max(1),
                false,
            ));
        } else {
            text.push_str(
                "以上是联网搜索到的信息。请把关键内容自然地融入你的回答，\
                 绝对不要在回复中输出来源名称、网址、链接列表或原始搜索结果。\n",
            );
        }
        truncate_output(&mut text);
        if text.trim().is_empty() {
            return Err(ToolError::Execution("Codex 搜索未返回有效结果".into()));
        }

        Ok(serde_json::json!({
            "ok": true,
            "query": query,
            "result_count": results.len().min(cfg.max_results.max(1)),
            "text": text,
        }))
    }

    /// DeepSeek Responses API 服务端联网搜索。
    ///
    /// 请求 `POST {base}/responses`，声明 `web_search` 工具并强制触发。
    /// 服务端会执行搜索并生成带引用的综合回答；这里解析 `output` 中的
    /// `web_search_call` 与 `final_answer` 消息。
    async fn execute_deepseek_search(
        &self,
        query: &str,
        cfg: &WebSearchSettings,
    ) -> Result<ToolResult, ToolError> {
        // DeepSeek Responses API 固定使用官方端点（与 bocha/kimi 一致，不读 base_url 配置）
        let endpoint = "https://api.deepseek.com/responses".to_string();
        let model = if cfg.model.trim().is_empty() {
            "deepseek-v4-flash"
        } else {
            cfg.model.trim()
        };
        // DeepSeek 的返回是模型生成的一段综合回答（内联引用），没有独立结构化条目，
        // 因此用 instructions 控制是否保留来源/链接，而不是事后剥离文本。
        let instructions = if cfg.hide_search_results {
            "你是联网搜索助手。搜索后把关键内容自然地融入回答，绝对不要输出来源名称、网址或链接列表。"
        } else {
            "你是联网搜索助手。搜索后请用简洁的中文总结搜索结果，保留关键事实与来源链接。"
        };
        let body = serde_json::json!({
            "model": model,
            "instructions": instructions,
            "input": query,
            "tools": [ { "type": "web_search" } ],
            "tool_choice": { "type": "web_search" },
            "max_output_tokens": 4096,
        });

        let client = Self::build_client(cfg)?;
        let response = client
            .post(&endpoint)
            .bearer_auth(cfg.api_key.trim())
            .json(&body)
            .send()
            .await
            .map_err(classify_request_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Execution(
                http_error_message(status, response).await,
            ));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("DeepSeek 搜索响应解析失败: {e}")))?;
        let output = payload
            .get("output")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let answer = extract_deepseek_answer(&output);
        if answer.trim().is_empty() {
            return Err(ToolError::Execution(
                "DeepSeek 搜索未返回有效结果（可能没有触发 web_search）".into(),
            ));
        }
        let mut text = answer.trim().to_string();
        truncate_output(&mut text);

        Ok(serde_json::json!({
            "ok": true,
            "query": query,
            "result_count": deepseek_search_action_count(&output),
            "text": text,
        }))
    }
}

fn normalize_codex_results(rows: &[Value]) -> Vec<Value> {
    let mut seen_urls = HashSet::new();
    rows.iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("text_result"))
        .filter_map(|item| {
            let get = |key: &str| item.get(key).and_then(Value::as_str).unwrap_or("");
            let parsed = reqwest::Url::parse(get("url").trim()).ok()?;
            if !matches!(parsed.scheme(), "http" | "https")
                || !seen_urls.insert(parsed.as_str().to_string())
            {
                return None;
            }
            Some(serde_json::json!({
                "title": get("title"),
                "url": parsed.as_str(),
                "snippet": get("snippet"),
            }))
        })
        .collect()
}

fn is_official_kimi_code_base_url(base_url: &str) -> bool {
    if base_url.trim().is_empty() {
        return true;
    }
    let Ok(url) = reqwest::Url::parse(base_url.trim()) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("api.kimi.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && (url.path().trim_end_matches('/') == "/coding" || url.path().starts_with("/coding/"))
}

fn codex_search_model(chat_provider: Option<&LlmProviderConfig>) -> String {
    if let Some(chat) =
        chat_provider.filter(|provider| provider.provider.eq_ignore_ascii_case("codex"))
    {
        if !chat.model.trim().is_empty() {
            return chat.model.trim().to_string();
        }
    }
    "gpt-5.6-sol".to_string()
}

/// 从 DeepSeek Responses API 的 `output` 中提取最终回答文本。
///
/// 只取 `phase == "final_answer"` 的 message 的 `output_text` 内容，
/// 跳过 commentary / reasoning 等非最终回答文本。
fn extract_deepseek_answer(output: &[Value]) -> String {
    let mut parts = Vec::new();
    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        if item.get("phase").and_then(Value::as_str) != Some("final_answer") {
            continue;
        }
        let Some(content) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in content {
            if part.get("type").and_then(Value::as_str) != Some("output_text") {
                continue;
            }
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                parts.push(text);
            }
        }
    }
    parts.join("")
}

/// 统计 DeepSeek Responses 响应中实际执行的搜索动作数量（用于测试/日志展示）。
fn deepseek_search_action_count(output: &[Value]) -> usize {
    output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("web_search_call"))
        .filter_map(|item| item.get("action").and_then(Value::as_object))
        .filter(|action| action.get("type").and_then(Value::as_str) == Some("search"))
        .count()
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
        Self::tool_definition(&self.settings.get().web_search)
    }

    fn timeout_hint(&self) -> Option<Duration> {
        let settings = self.settings.get().web_search;
        Some(if settings.provider == "deepseek" {
            DEEPSEEK_TIMEOUT
        } else if settings.provider == "codex" {
            // Codex 搜索 = 一轮模型生成 + 联网，与 DeepSeek 同级
            DEEPSEEK_TIMEOUT
        } else {
            SEARCH_TIMEOUT
        })
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let cfg = self.settings.get().web_search;
        if !cfg.enabled {
            return Err(ToolError::Execution(
                "网页搜索未启用，请用户在「高级设置 → 工具设置 → 网页搜索」开启".into(),
            ));
        }

        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("缺少必填参数 query".into()))?;
        let query = bounded_query(query);
        let cfg = Self::resolve_execution_settings(context, &cfg)?;

        self.execute_search_endpoint(context, &query, &cfg).await
    }
}

fn bounded_query(query: &str) -> String {
    query.chars().take(MAX_QUERY_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(kind: &str, model: &str) -> LlmProviderConfig {
        LlmProviderConfig {
            id: "test".into(),
            label: "test".into(),
            provider: kind.into(),
            model: model.into(),
            api_key: "secret".into(),
            base_url: String::new(),
            temperature: None,
            top_p: None,
            enable_thinking: false,
            reasoning_effort: None,
            fast_mode: false,
            support_vision: false,
            vision_compress: false,
            vision_max_edge: 2048,
            vision_jpeg_quality: 85,
        }
    }

    #[test]
    fn codex_search_uses_only_the_active_codex_chat_model() {
        let chat = provider("codex", "gpt-5.3-codex");
        assert_eq!(codex_search_model(Some(&chat)), "gpt-5.3-codex");
        assert_eq!(codex_search_model(None), "gpt-5.6-sol");
        assert_eq!(
            codex_search_model(Some(&provider("kimicode", "kimi-for-coding"))),
            "gpt-5.6-sol"
        );
        assert_eq!(
            codex_search_model(Some(&provider("CoDeX", "gpt-5.4"))),
            "gpt-5.4"
        );
    }

    #[test]
    fn kimi_chat_credential_reuse_is_restricted_to_official_origin() {
        assert!(is_official_kimi_code_base_url(""));
        assert!(is_official_kimi_code_base_url(
            "https://api.kimi.com/coding"
        ));
        assert!(is_official_kimi_code_base_url(
            "https://api.kimi.com/coding/v1"
        ));
        assert!(!is_official_kimi_code_base_url(
            "http://api.kimi.com/coding"
        ));
        assert!(!is_official_kimi_code_base_url(
            "https://api.kimi.com.evil.test/coding"
        ));
        assert!(!is_official_kimi_code_base_url(
            "https://api.kimi.com/other"
        ));
        assert!(!is_official_kimi_code_base_url(
            "https://user:password@api.kimi.com/coding"
        ));
    }

    #[tokio::test]
    async fn unknown_search_provider_fails_closed() {
        let tool = WebSearchTool::new(SharedToolSettings::new(
            crate::ai_service::tools::settings::ToolSettings::default(),
        ));
        let mut cfg = WebSearchSettings::default();
        cfg.provider = "future-provider".into();
        cfg.api_key = "must-not-be-forwarded".into();
        let error = tool
            .execute_search_endpoint(&ToolContext::default(), "query", &cfg)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("不支持的网页搜索提供商"));
    }

    #[test]
    fn hidden_results_do_not_expose_urls() {
        let results = vec![serde_json::json!({
            "title": "A",
            "url": "https://secret.test/path",
            "snippet": "summary"
        })];
        let text = WebSearchTool::format_results("query", &results, 1, true);
        assert!(!text.contains("secret.test"));
        assert!(text.contains("绝对不要在回复中输出"));
    }

    #[test]
    fn codex_results_keep_unique_http_sources_only() {
        let rows = vec![
            serde_json::json!({"type":"text_result","url":"https://a.test/x","title":"A","snippet":"one"}),
            serde_json::json!({"type":"text_result","url":"https://a.test/x","title":"duplicate"}),
            serde_json::json!({"type":"text_result","url":"javascript:alert(1)","title":"unsafe"}),
            serde_json::json!({"type":"other","url":"https://b.test/"}),
            serde_json::json!({"type":"text_result","url":"http://b.test/","title":"B"}),
        ];
        let results = normalize_codex_results(&rows);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["url"], "https://a.test/x");
        assert_eq!(results[1]["url"], "http://b.test/");
    }
}
