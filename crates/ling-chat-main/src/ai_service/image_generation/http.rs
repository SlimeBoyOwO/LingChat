use anyhow::{Context, Result, ensure};
use base64::Engine;
use futures_util::StreamExt;
use reqwest::{Client, Url};
use serde_json::Value;

pub(super) const MAX_RESPONSE_BYTES: usize = 48 * 1024 * 1024;

pub(super) fn validate_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).context("请填写完整的供应商 HTTP(S) 地址")?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "供应商地址必须是 HTTP(S)，凭据请填写到 API Key"
    );
    Ok(url)
}

pub(super) fn validate_base_url(raw: &str) -> Result<()> {
    let url = validate_url(raw)?;
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "接口基础地址不能包含查询参数或片段"
    );
    Ok(())
}

pub(super) async fn limited_body(response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    ensure!(
        response
            .content_length()
            .is_none_or(|size| size <= limit as u64),
        "供应商响应超过大小限制"
    );
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取供应商响应失败")?;
        ensure!(bytes.len() + chunk.len() <= limit, "供应商响应超过大小限制");
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) async fn read_openai_image(
    http: &Client,
    response: reqwest::Response,
    secret: &str,
) -> Result<String> {
    let status = response.status();
    let bytes = limited_body(
        response,
        if status.is_success() {
            MAX_RESPONSE_BYTES
        } else {
            64 * 1024
        },
    )
    .await?;
    let payload: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if !status.is_success() {
        let message = payload
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("供应商拒绝了图片请求");
        let message = if secret.is_empty() {
            message.to_owned()
        } else {
            message.replace(secret, "[redacted]")
        };
        anyhow::bail!(
            "图片生成失败（HTTP {status}）：{}",
            message.chars().take(500).collect::<String>()
        );
    }
    ensure!(payload.is_object(), "图片接口未返回有效 JSON");
    let image = payload
        .pointer("/data/0")
        .context("图片响应中没有 data 图像")?;
    if let Some(encoded) = image
        .get("b64_json")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Ok(encoded.to_owned());
    }
    let url = image
        .get("url")
        .and_then(Value::as_str)
        .context("图片响应中没有 Base64 或下载地址")?;
    validate_url(url)?;
    // 授权头仅设置在生成请求上，图片下载不携带供应商凭据。
    let response = http.get(url).send().await?.error_for_status()?;
    Ok(base64::engine::general_purpose::STANDARD
        .encode(limited_body(response, MAX_RESPONSE_BYTES).await?))
}
