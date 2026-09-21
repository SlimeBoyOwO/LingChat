//! WorkBuddy 账号级模型目录发现（`GET {base}/v3/config` → `data.models[]`）。
//!
//! 与 Codex 目录同一原则：模型 ID 与能力（推理档位）由上游目录供给，
//! 不在本地维护第二份易过时的模型清单。仅当请求失败时回退内置兜底清单。

use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderValue};
use serde_json::Value;

use crate::ai_service::llm::provider::{LlmModelInfo, ThinkEffortsInfo};
use crate::ai_service::llm::workbuddy::auth::{WorkBuddyCredential, account_headers};

/// 非对话模型的 ID 前缀（嵌入/补全/代码专用，选用会报 code=11102）。
const NON_CHAT_PREFIXES: [&str; 3] = ["nes-", "completion-", "codewise-"];
/// tiny 输出的非对话模型（maxOutputTokens ≤ 256）。
const NON_CHAT_MAX_OUTPUT_TOKENS: f64 = 256.0;
/// 图片生成/改图模型不走对话端点，同样过滤。
const IMAGE_TAGS: [&str; 2] = ["text-to-image", "image-to-image"];
/// 请求失败时的静态兜底清单（与官方 CLI 内置目录一致）。
const FALLBACK_CATALOG: [&str; 17] = [
    "auto",
    "hy3",
    "hy4-preview",
    "hy4-preview-f",
    "glm-5.3",
    "glm-5.3-flash",
    "glm-5.2",
    "glm-5.1",
    "glm-5v-turbo",
    "kimi-k3-2",
    "kimi-k2.8-preview",
    "kimi-k2.7",
    "kimi-k2.6",
    "minimax-m3-pay",
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "deepseek-v4.1-flash",
];

pub(super) async fn fetch_models(
    http: &Client,
    cred: &WorkBuddyCredential,
    token: &str,
) -> Result<Vec<LlmModelInfo>> {
    let (base, _, _) = crate::ai_service::llm::workbuddy::auth::realm_endpoints(&cred.realm);
    let mut header_map = account_headers(cred, token).context("构建 WorkBuddy 会话头失败")?;
    header_map.insert(ACCEPT, HeaderValue::from_static("application/json"));

    let response = http
        .get(format!("{base}/v3/config"))
        .headers(header_map)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .context("请求 WorkBuddy 模型目录失败")?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("获取 WorkBuddy 模型目录失败 ({status}): {}", short(&text));
    }
    let payload: Value = serde_json::from_str(&text).context("解析 WorkBuddy 模型目录失败")?;
    // 上游业务信封：code != 0 视为失败（HTTP 200 也可能带业务错误）
    if let Some(error) = envelope_error(&payload) {
        bail!("获取 WorkBuddy 模型目录失败: {error}");
    }
    let models = parse_models(&payload);
    if models.is_empty() {
        // 目录为空/格式不符：回退内置清单，保证模型选择器可用
        return Ok(fallback_models());
    }
    Ok(models)
}

/// 业务信封错误：`code != 0` 时返回可读文案。
///
/// 注意：令牌失效时上游返回 HTTP 200 且 `data.models = null`（code 仍为 0），
/// 这种情况由调用方按「空目录 → 兜底清单」处理。
fn envelope_error(payload: &Value) -> Option<String> {
    let code = payload.get("code").and_then(Value::as_i64)?;
    if code == 0 {
        return None;
    }
    let msg = payload
        .get("msg")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    Some(if msg.is_empty() {
        format!("code={code}")
    } else {
        format!("code={code} {msg}")
    })
}

/// 上游目录不可用时的静态兜底（无能力信息，仅 ID）。
pub(super) fn fallback_models() -> Vec<LlmModelInfo> {
    FALLBACK_CATALOG
        .iter()
        .map(|id| LlmModelInfo {
            id: (*id).to_string(),
            display_name: None,
            context_length: None,
            supports_reasoning: false,
            supports_thinking_type: None,
            think_efforts: None,
        })
        .collect()
}

fn short(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.len() > 200 {
        &trimmed[..200]
    } else {
        trimmed
    }
}

fn nonempty(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn is_non_chat_model(id: &str, max_output_tokens: Option<f64>, tags: &[String]) -> bool {
    let lowered = id.to_lowercase();
    if NON_CHAT_PREFIXES.iter().any(|p| lowered.starts_with(p)) {
        return true;
    }
    if let Some(max) = max_output_tokens {
        if max > 0.0 && max <= NON_CHAT_MAX_OUTPUT_TOKENS {
            return true;
        }
    }
    tags.iter().any(|tag| IMAGE_TAGS.contains(&tag.as_str()))
}

/// 从 `data.models[]` 提取对话模型与推理档位。
fn parse_models(payload: &Value) -> Vec<LlmModelInfo> {
    let Some(entries) = payload.pointer("/data/models").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    for entry in entries {
        if entry
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(id) = nonempty(entry.get("id")) else {
            continue;
        };
        let tags: Vec<String> = entry
            .get("tags")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let max_output = entry.get("maxOutputTokens").and_then(Value::as_f64);
        if is_non_chat_model(id, max_output, &tags) {
            continue;
        }
        if !seen.insert(id.to_owned()) {
            continue;
        }
        // 推理档位：entry.reasoning.supportedEfforts / defaultEffort
        let reasoning = entry.get("reasoning");
        let mut efforts: Vec<String> = reasoning
            .and_then(|r| r.get("supportedEfforts"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        efforts.dedup();
        let default_effort = nonempty(reasoning.and_then(|r| r.get("defaultEffort")))
            .filter(|effort| efforts.iter().any(|valid| valid == effort))
            .map(str::to_owned);
        models.push(LlmModelInfo {
            id: id.to_owned(),
            display_name: nonempty(entry.get("name")).map(str::to_owned),
            context_length: entry
                .get("maxInputTokens")
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
    models
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model_entry(id: &str, extra: Value) -> Value {
        let mut entry = json!({ "id": id, "name": id });
        if let (Some(obj), Some(map)) = (entry.as_object_mut(), extra.as_object()) {
            for (k, v) in map {
                obj.insert(k.clone(), v.clone());
            }
        }
        entry
    }

    #[test]
    fn parse_models_filters_non_chat_and_dedupes() {
        let payload = json!({
            "code": 0,
            "data": {
                "models": [
                    model_entry("glm-5.2", json!({})),
                    model_entry("glm-5.2", json!({})), // 重复
                    model_entry("nes-embed", json!({})), // 前缀过滤
                    model_entry("completion-tiny", json!({})),
                    model_entry("img-gen", json!({ "tags": ["text-to-image"] })),
                    model_entry("img-edit", json!({ "tags": ["chat", "image-to-image"] })),
                    model_entry("tiny", json!({ "maxOutputTokens": 128 })),
                    model_entry("edge", json!({ "maxOutputTokens": 256 })),
                    model_entry("big", json!({ "maxOutputTokens": 65536 })),
                    model_entry("disabled-model", json!({ "disabled": true })),
                ]
            }
        });
        let models = parse_models(&payload);
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        // maxOutputTokens 边界：≤256 一律视为非对话模型
        assert_eq!(ids, vec!["glm-5.2", "big"]);
    }

    #[test]
    fn parse_models_reads_reasoning_efforts() {
        let payload = json!({
            "data": {
                "models": [
                    model_entry(
                        "deepseek-v4.1-flash",
                        json!({
                            "name": "DeepSeek V4.1 Flash",
                            "maxInputTokens": 1000000,
                            "reasoning": {
                                "supportedEfforts": ["low", "high", "max"],
                                "defaultEffort": "high"
                            }
                        })
                    ),
                    model_entry("glm-5v-turbo", json!({ "reasoning": { "supportedEfforts": ["medium"] } })),
                ]
            }
        });
        let models = parse_models(&payload);
        assert_eq!(models.len(), 2);
        let deepseek = &models[0];
        assert_eq!(
            deepseek.display_name.as_deref(),
            Some("DeepSeek V4.1 Flash")
        );
        assert_eq!(deepseek.context_length, Some(1000000));
        assert!(deepseek.supports_reasoning);
        let efforts = deepseek.think_efforts.as_ref().expect("think_efforts");
        assert_eq!(efforts.valid_efforts, vec!["low", "high", "max"]);
        assert_eq!(efforts.default_effort.as_deref(), Some("high"));

        // 无 defaultEffort 时为 None
        let glm = &models[1];
        let efforts = glm.think_efforts.as_ref().expect("think_efforts");
        assert_eq!(efforts.valid_efforts, vec!["medium"]);
        assert!(efforts.default_effort.is_none());
    }

    #[test]
    fn parse_models_tolerates_missing_or_malformed_payload() {
        assert!(parse_models(&json!({})).is_empty());
        assert!(parse_models(&json!({ "data": {} })).is_empty());
        assert!(parse_models(&json!({ "data": { "models": "oops" } })).is_empty());
        assert!(parse_models(&json!({ "data": { "models": [1, null, "x"] } })).is_empty());
    }

    #[test]
    fn envelope_error_reports_business_failures_only() {
        assert!(envelope_error(&json!({ "code": 0, "msg": "OK" })).is_none());
        assert!(envelope_error(&json!({ "data": {} })).is_none());
        assert_eq!(
            envelope_error(&json!({ "code": 11217, "msg": "login ing..." })).as_deref(),
            Some("code=11217 login ing...")
        );
        assert_eq!(
            envelope_error(&json!({ "code": 14003 })).as_deref(),
            Some("code=14003")
        );
    }

    #[test]
    fn fallback_models_cover_core_catalog() {
        let models = fallback_models();
        assert!(models.iter().any(|m| m.id == "auto"));
        assert!(models.iter().any(|m| m.id == "deepseek-v4.1-flash"));
        assert!(models.iter().all(|m| m.think_efforts.is_none()));
    }
}
