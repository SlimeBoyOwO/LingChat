//! Account-scoped Codex model discovery. Keep model IDs and capabilities supplied
//! by the catalog instead of maintaining a second, quickly outdated model list.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use serde_json::Value;

use crate::ai_service::llm::provider::{LlmModelInfo, ThinkEffortsInfo};

const MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
// This endpoint gates catalogs by Codex client compatibility, not the LingChat
// application version. GPT-6 Astra's catalog minimum is 0.153.0.
const CATALOG_CLIENT_VERSION: &str = "0.153.4";

fn catalog_request(http: &Client, mut headers: HeaderMap) -> reqwest::RequestBuilder {
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    http.get(MODELS_URL)
        .query(&[("client_version", CATALOG_CLIENT_VERSION)])
        .headers(headers)
        .timeout(Duration::from_secs(30))
}

pub(super) async fn fetch_models(http: &Client, headers: HeaderMap) -> Result<Vec<LlmModelInfo>> {
    let response = catalog_request(http, headers)
        .send()
        .await
        .context("请求 Codex 模型目录失败")?
        .error_for_status()
        .context("获取 Codex 模型目录失败")?;
    let payload: Value = response.json().await.context("解析 Codex 模型目录失败")?;
    parse_models(&payload)
}

fn nonempty(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn parse_models(payload: &Value) -> Result<Vec<LlmModelInfo>> {
    let entries = payload
        .get("models")
        .and_then(Value::as_array)
        .context("Codex 模型目录格式无效：缺少 models 数组")?;
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    for entry in entries {
        // `supported_in_api` refers to API-key access. Subscription-only models
        // such as Spark must remain selectable when the picker marks them visible.
        if entry
            .get("visibility")
            .and_then(Value::as_str)
            .is_some_and(|v| v != "list")
        {
            continue;
        }
        let Some(id) = nonempty(entry.get("slug")) else {
            continue;
        };
        if !seen.insert(id.to_owned()) {
            continue;
        }
        let mut efforts = Vec::new();
        if let Some(levels) = entry
            .get("supported_reasoning_levels")
            .and_then(Value::as_array)
        {
            for level in levels {
                if let Some(effort) = nonempty(level.get("effort")) {
                    if !efforts.iter().any(|existing| existing == effort) {
                        efforts.push(effort.to_owned());
                    }
                }
            }
        }
        let default_effort = nonempty(entry.get("default_reasoning_level"))
            .filter(|effort| efforts.iter().any(|valid| valid == effort))
            .map(str::to_owned);
        models.push(LlmModelInfo {
            id: id.to_owned(),
            display_name: nonempty(entry.get("display_name")).map(str::to_owned),
            context_length: entry
                .get("context_window")
                .and_then(Value::as_u64)
                .filter(|v| *v > 0),
            supports_reasoning: !efforts.is_empty(),
            supports_thinking_type: None,
            think_efforts: if efforts.is_empty() {
                None
            } else {
                Some(ThinkEffortsInfo {
                    valid_efforts: efforts,
                    default_effort,
                })
            },
        });
    }
    ensure!(
        !models.is_empty(),
        "Codex 模型目录未返回可用模型，请检查账号权限后重试"
    );
    Ok(models)
}
