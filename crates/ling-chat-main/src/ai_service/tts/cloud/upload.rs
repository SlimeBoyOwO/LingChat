//! 本地音频上传到 DashScope 临时存储（免费、48h 有效、与模型绑定），换取 oss:// 临时 URL。
//!
//! 流程（参考已验证实现）：
//! ① GET /api/v1/uploads?action=getPolicy&model=... 取 OSS 上传凭证
//! ② multipart 直传 {upload_host}，key = {upload_dir}/{uuid8位}-{原文件名}（uuid 前缀防同名覆盖）
//! ③ 返回 oss://{key}，供 create_voice 的 url 字段使用（需 X-DashScope-OssResourceResolve 头）

use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::ai_service::tts::adapters::http_client;

const BASE_URL: &str = "https://dashscope.aliyuncs.com/api/v1";
const UPLOADS_PATH: &str = "/uploads";

/// getPolicy 响应字段（字段名来自已验证实现，与官方 Python SDK 一致）。
struct UploadPolicy {
    upload_host: String,
    upload_dir: String,
    policy: String,
    oss_access_key_id: String,
    signature: String,
    x_oss_object_acl: String,
    x_oss_forbid_overwrite: String,
}

fn parse_policy(data: &Value) -> Result<UploadPolicy> {
    let required = |key: &str| -> Result<String> {
        data[key]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("getPolicy 响应缺少 {key}: {data}"))
    };
    Ok(UploadPolicy {
        upload_host: required("upload_host")?,
        upload_dir: required("upload_dir")?,
        policy: required("policy")?,
        oss_access_key_id: required("oss_access_key_id")?,
        signature: required("signature")?,
        x_oss_object_acl: required("x_oss_object_acl")?,
        x_oss_forbid_overwrite: required("x_oss_forbid_overwrite")?,
    })
}

/// 上传本地音频，返回 oss:// 形式临时 URL。
pub async fn upload_audio(api_key: &str, model: &str, file_path: &Path) -> Result<String> {
    // ① 拿 OSS 上传凭证
    let resp = http_client()
        .get(format!("{BASE_URL}{UPLOADS_PATH}"))
        .query(&[("action", "getPolicy"), ("model", model)])
        .bearer_auth(api_key)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or_default();
        let body_str = body.to_string();
        let code = body["code"].as_str().unwrap_or("HTTP_ERROR");
        let message = body["message"].as_str().unwrap_or(&body_str);
        return Err(anyhow!(
            "获取上传凭证失败: {code}: {message} (HTTP {status})"
        ));
    }
    let v: Value = resp.json().await?;
    let policy = parse_policy(&v["data"])?;
    tracing::info!("CosyVoice 上传凭证已获取: upload_dir={}", policy.upload_dir);

    // ② 直传 OSS 临时空间
    let file_name = file_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sample.wav".to_string());
    // uuid 前 8 位前缀防同名文件覆盖（同一 upload_dir 下已有同名 key 会静默覆盖）
    let uuid_prefix = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let key = format!("{}/{}-{}", policy.upload_dir, uuid_prefix, file_name);
    tracing::debug!(
        "CosyVoice OSS 直传: host={} key={}",
        policy.upload_host,
        key
    );

    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| anyhow!("读取音频文件失败: {e}"))?;
    tracing::info!("CosyVoice 上传样本: {} ({} bytes)", file_name, bytes.len());

    let form = reqwest::multipart::Form::new()
        .text("OSSAccessKeyId", policy.oss_access_key_id.clone())
        // 注意：OSS POST 直传字段名——Signature 首字母大写、x-oss-* 用连字符
        // （与已验证实现一致；写错字段名 OSS 会直接 400）
        .text("Signature", policy.signature.clone())
        .text("policy", policy.policy.clone())
        .text("x-oss-object-acl", policy.x_oss_object_acl.clone())
        .text(
            "x-oss-forbid-overwrite",
            policy.x_oss_forbid_overwrite.clone(),
        )
        .text("key", key.clone())
        .text("success_action_status", "200")
        .part(
            "file",
            reqwest::multipart::Part::bytes(bytes)
                .file_name(file_name)
                .mime_str("application/octet-stream")?,
        );

    let upload_resp = http_client()
        .post(policy.upload_host.clone())
        .multipart(form)
        .send()
        .await?;
    if !upload_resp.status().is_success() {
        let status = upload_resp.status();
        let text = upload_resp.text().await.unwrap_or_default();
        return Err(anyhow!("OSS 直传失败: HTTP {status}: {text}"));
    }
    tracing::info!("CosyVoice 样本上传成功: oss://{key}");
    Ok(format!("oss://{}", key))
}
