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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requests_json_catalog_with_subscription_headers_and_client_compatibility() {
        use reqwest::header::{AUTHORIZATION, HeaderValue};
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer test-token"));
        headers.insert(
            "chatgpt-account-id",
            HeaderValue::from_static("test-account"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        let request = catalog_request(&Client::new(), headers).build().unwrap();
        assert_eq!(request.method(), reqwest::Method::GET);
        assert_eq!(request.url().host_str(), Some("chatgpt.com"));
        assert_eq!(request.url().path(), "/backend-api/codex/models");
        assert_eq!(request.url().query(), Some("client_version=0.153.4"));
        assert_eq!(request.headers()[ACCEPT], "application/json");
        assert_eq!(request.headers().get_all(ACCEPT).iter().count(), 1);
        assert_eq!(request.headers()[AUTHORIZATION], "Bearer test-token");
        assert_eq!(request.headers()["chatgpt-account-id"], "test-account");
        assert_eq!(request.timeout(), Some(&Duration::from_secs(30)));
    }

    #[test]
    fn discovers_astra_and_preserves_catalog_capabilities() {
        let result = parse_models(&json!({"models": [{
            "slug": "gpt-6-astra", "display_name": "GPT-6-Astra", "visibility": "list",
            "context_window": 272000, "default_reasoning_level": "medium",
            "supported_reasoning_levels": [{"effort":"low"}, {"effort":"medium"}, {"effort":"ultra"}],
            "new_catalog_field": "ignored"
        }]})).unwrap();
        let astra = &result[0];
        assert_eq!(astra.id, "gpt-6-astra");
        assert_eq!(astra.display_name.as_deref(), Some("GPT-6-Astra"));
        assert_eq!(astra.context_length, Some(272000));
        assert!(astra.supports_reasoning);
        let efforts = astra.think_efforts.as_ref().unwrap();
        assert_eq!(efforts.valid_efforts, ["low", "medium", "ultra"]);
        assert_eq!(efforts.default_effort.as_deref(), Some("medium"));
    }

    #[test]
    fn keeps_subscription_models_and_future_ids_without_a_whitelist() {
        let result = parse_models(&json!({"models": [
            {"slug":"gpt-future", "visibility":"list"},
            {"slug":"internal-model", "visibility":"hide"},
            {"slug":"gpt-5.3-codex-spark", "visibility":"list", "supported_in_api":false},
            {"slug":"gpt-future", "display_name":"duplicate"},
            {"slug":"   "}, {"slug":42}, null
        ]}))
        .unwrap();
        assert_eq!(
            result.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["gpt-future", "gpt-5.3-codex-spark"]
        );
        assert!(!result[0].supports_reasoning);
        assert!(result[0].think_efforts.is_none());
    }

    #[test]
    fn normalizes_optional_metadata_without_inventing_reasoning_levels() {
        let result = parse_models(&json!({"models": [{
            "slug":" custom-model ", "display_name":" ", "context_window":-1,
            "default_reasoning_level":"invalid",
            "supported_reasoning_levels":[{"effort":" high "}, {"effort":"high"}, {}, {"effort":42}]
        }]}))
        .unwrap();
        let model = &result[0];
        assert_eq!(model.id, "custom-model");
        assert!(model.display_name.is_none());
        assert!(model.context_length.is_none());
        assert_eq!(
            model.think_efforts.as_ref().unwrap().valid_efforts,
            ["high"]
        );
        assert!(
            model
                .think_efforts
                .as_ref()
                .unwrap()
                .default_effort
                .is_none()
        );
    }

    #[test]
    fn rejects_invalid_or_empty_catalogs_instead_of_reporting_fake_success() {
        for value in [
            json!({}),
            json!({"models":null}),
            json!({"models":[]}),
            json!({"models":[{"slug":"hidden", "visibility":"hide"}]}),
        ] {
            assert!(parse_models(&value).is_err());
        }
    }
}
