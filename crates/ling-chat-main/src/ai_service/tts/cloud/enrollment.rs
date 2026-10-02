//! CosyVoice 音色注册/查询/列表/删除 HTTP 客户端。
//!
//! 官方 HTTP API：`POST /api/v1/services/audio/tts/customization`，
//! 通过 `input.action` 区分操作（create_voice / query_voice / list_voice / delete_voice）。
//! 认证：Bearer API Key。

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::ai_service::tts::adapters::http_client;

const BASE_URL: &str = "https://dashscope.aliyuncs.com/api/v1";
const ENROLLMENT_PATH: &str = "/services/audio/tts/customization";

/// 构造 create_voice 请求体（纯函数，便于测试）。
pub fn create_voice_body(
    model: &str,
    prefix: &str,
    url: &str,
    language_hints: Option<&[&str]>,
) -> Value {
    json!({
        "model": "voice-enrollment",
        "input": {
            "action": "create_voice",
            "target_model": model,
            "prefix": prefix,
            "url": url,
            "language_hints": language_hints.unwrap_or(&["zh"]),
        }
    })
}

/// 从 create_voice 响应提取 voice_id。
pub fn parse_voice_id(resp: &Value) -> Result<String> {
    resp["output"]["voice_id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("create_voice 响应缺少 voice_id: {resp}"))
}

/// 从 query_voice 响应提取 status。
pub fn parse_voice_status(resp: &Value) -> Result<String> {
    resp["output"]["status"]
        .as_str()
        .map(|s| s.to_lowercase())
        .ok_or_else(|| anyhow!("query_voice 响应缺少 status: {resp}"))
}

async fn post_customization(api_key: &str, body: Value) -> Result<Value> {
    let resp = http_client()
        .post(format!("{BASE_URL}{ENROLLMENT_PATH}"))
        .bearer_auth(api_key)
        // oss:// 临时 URL 需要此头解析；https:// URL 传此头无害
        .header("X-DashScope-OssResourceResolve", "enable")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();
        let body_str = body.to_string();
        let code = body["code"].as_str().unwrap_or("HTTP_ERROR");
        let message = body["message"].as_str().unwrap_or(&body_str);
        return Err(anyhow!(
            "CosyVoice 请求失败: {code}: {message} (HTTP {status})"
        ));
    }
    Ok(resp.json().await?)
}

pub async fn create_voice(
    api_key: &str,
    model: &str,
    prefix: &str,
    url: &str,
    language_hints: Option<&[&str]>,
) -> Result<String> {
    let body = create_voice_body(model, prefix, url, language_hints);
    let resp = post_customization(api_key, body).await?;
    parse_voice_id(&resp)
}

pub async fn query_voice(api_key: &str, voice_id: &str) -> Result<String> {
    let resp = post_customization(
        api_key,
        json!({
            "model": "voice-enrollment",
            "input": {"action": "query_voice", "voice_id": voice_id}
        }),
    )
    .await?;
    parse_voice_status(&resp)
}

pub async fn delete_voice(api_key: &str, voice_id: &str) -> Result<()> {
    let resp = post_customization(
        api_key,
        json!({
            "model": "voice-enrollment",
            "input": {"action": "delete_voice", "voice_id": voice_id}
        }),
    )
    .await?;
    // 删除成功：output 可为空或 message；HTTP 2xx 即视为成功
    let _ = resp;
    Ok(())
}
