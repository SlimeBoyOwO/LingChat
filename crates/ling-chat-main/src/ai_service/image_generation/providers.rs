use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use base64::Engine;
use futures_util::StreamExt;
use regex::Regex;
use reqwest::{Client, Url};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tokio::sync::Mutex;

use super::ImageGenerationSettings;
use crate::ai_service::llm::{codex_auth, provider_config::load_providers};
use crate::utils::proxy::build_proxied_client;

pub(super) const DEFAULT_PROVIDER: &str = "codex";
pub(super) const MAX_RESPONSE_BYTES: usize = 48 * 1024 * 1024;
const CODEX_BASE: &str = "https://chatgpt.com/backend-api/codex";
const CODEX_CATALOG: &str = "https://developers.openai.com/api/docs/models";

#[derive(Serialize)]
pub struct ImageProviderInfo {
    pub id: String,
    pub label: String,
    pub custom: bool,
    pub protocol: &'static str,
}

#[derive(Clone, Serialize)]
pub struct ImageModelCatalog {
    pub models: Vec<String>,
    pub source: String,
    pub warning: String,
    pub allow_automatic: bool,
    pub novelai_metadata: Option<super::novelai::NovelAiMetadata>,
}

static CATALOGS: LazyLock<Mutex<HashMap<String, (Instant, ImageModelCatalog)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn list_providers(app: &AppHandle) -> Vec<ImageProviderInfo> {
    let mut result = vec![
        ImageProviderInfo {
            id: DEFAULT_PROVIDER.into(),
            label: "Codex 订阅".into(),
            custom: false,
            protocol: "codex",
        },
        ImageProviderInfo {
            id: "openai-compatible".into(),
            label: "OpenAI compatible".into(),
            custom: true,
            protocol: "openai",
        },
        ImageProviderInfo {
            id: "novelai".into(),
            label: "NovelAI".into(),
            custom: false,
            protocol: "novelai",
        },
    ];
    result.extend(
        load_providers(app)
            .into_iter()
            .filter(|p| matches!(p.provider.as_str(), "openai" | "lmstudio"))
            .map(|p| ImageProviderInfo {
                id: format!("profile:{}", p.id),
                label: p.label,
                custom: false,
                protocol: "openai",
            }),
    );
    result
}

struct Connection {
    codex: bool,
    base: String,
    key: String,
}

fn connection(app: &AppHandle, settings: &ImageGenerationSettings) -> Result<Connection> {
    if settings.provider == DEFAULT_PROVIDER {
        return Ok(Connection {
            codex: true,
            base: CODEX_BASE.into(),
            key: String::new(),
        });
    }
    let (base, key) = if settings.provider == "openai-compatible" {
        (settings.base_url.clone(), settings.api_key.clone())
    } else {
        let id = settings
            .provider
            .strip_prefix("profile:")
            .context("未知的图片供应商")?;
        let profile = load_providers(app)
            .into_iter()
            .find(|p| p.id == id)
            .context("图片供应商配置已删除，请重新选择")?;
        ensure!(
            matches!(profile.provider.as_str(), "openai" | "lmstudio"),
            "该供应商协议尚不支持图片生成"
        );
        let base = if profile.base_url.trim().is_empty() && profile.provider == "openai" {
            "https://api.openai.com/v1".into()
        } else {
            profile.base_url
        };
        (base, profile.api_key)
    };
    let url = validate_url(&base)?;
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "接口基础地址不能包含查询参数或片段"
    );
    Ok(Connection {
        codex: false,
        base: base.trim_end_matches('/').into(),
        key,
    })
}

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

pub async fn list_models(
    app: &AppHandle,
    settings: &ImageGenerationSettings,
    refresh: bool,
) -> Result<ImageModelCatalog> {
    if settings.provider == "novelai" {
        return super::novelai::list_models(&settings.novelai, refresh).await;
    }
    let conn = connection(app, settings)?;
    let source = if !settings.models_url.is_empty() {
        settings.models_url.clone()
    } else if conn.codex {
        CODEX_CATALOG.into()
    } else {
        format!("{}/models", conn.base)
    };
    validate_url(&source)?;
    let cache_key = format!(
        "{}:{}:{:x}",
        settings.provider,
        source,
        Sha256::digest(conn.key.as_bytes())
    );
    let mut cache = CATALOGS.lock().await;
    if !refresh {
        if let Some((time, catalog)) = cache.get(&cache_key) {
            if time.elapsed() < Duration::from_secs(6 * 60 * 60) {
                return Ok(catalog.clone());
            }
        }
    }
    let fetched = async {
        let http = build_proxied_client(30).await?;
        let mut request = http.get(&source);
        // 公开目录不接收订阅凭据；其他供应商仅向其自身模型接口发送 API Key。
        if !conn.codex
            && !conn.key.is_empty()
            && validate_url(&source)?.origin() == validate_url(&conn.base)?.origin()
        {
            request = request.bearer_auth(&conn.key);
        }
        let response = request.send().await.context("请求图片模型目录失败")?;
        ensure!(
            response.status().is_success(),
            "模型目录返回 HTTP {}",
            response.status()
        );
        let bytes = limited_body(response, 4 * 1024 * 1024).await?;
        let (models, warning, allow_automatic) = if conn.codex && settings.models_url.is_empty() {
            (
                parse_official_catalog(std::str::from_utf8(&bytes)?)?,
                "公开图片目录；实际调用权限以订阅账号回执为准".to_string(),
                true,
            )
        } else {
            parse_api_catalog(&serde_json::from_slice(&bytes).context("模型接口未返回有效 JSON")?)?
        };
        Ok::<_, anyhow::Error>(ImageModelCatalog {
            models,
            source: source.clone(),
            warning,
            allow_automatic,
            novelai_metadata: None,
        })
    }
    .await;
    match fetched {
        Ok(catalog) => {
            cache.insert(cache_key, (Instant::now(), catalog.clone()));
            Ok(catalog)
        },
        Err(error) => match cache.get(&cache_key) {
            Some((_, previous)) => {
                let mut stale = previous.clone();
                stale.warning = format!("模型刷新失败，显示上次成功获取的目录：{error}");
                Ok(stale)
            },
            None => Err(error),
        },
    }
}

fn parse_official_catalog(html: &str) -> Result<Vec<String>> {
    let pattern = Regex::new(r#"/api/docs/models/(gpt-image-[a-z0-9.-]+)"#)?;
    let dated = Regex::new(r"-\d{4}-\d{2}-\d{2}$")?;
    let mut seen = HashSet::new();
    let mut models: Vec<_> = pattern
        .captures_iter(html)
        .filter_map(|m| {
            let id = m[1].to_string();
            (!dated.is_match(&id) && seen.insert(id.clone())).then_some(id)
        })
        .collect();
    let version = |id: &str| {
        id.trim_start_matches("gpt-image-")
            .split('-')
            .next()
            .unwrap_or("")
            .split('.')
            .filter_map(|part| part.parse::<u32>().ok())
            .collect::<Vec<_>>()
    };
    models.sort_by_key(|id| std::cmp::Reverse(version(id)));
    ensure!(
        !models.is_empty(),
        "官方目录未返回图片模型，请重试或手动填写；没有使用内置模型列表"
    );
    Ok(models)
}

fn parse_api_catalog(payload: &Value) -> Result<(Vec<String>, String, bool)> {
    let entries = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array)
        .context("模型目录缺少 data 或 models 数组")?;
    let mut all = Vec::new();
    let mut images = Vec::new();
    let mut candidates = Vec::new();
    for entry in entries {
        let id = entry.as_str().or_else(|| {
            entry
                .get("id")
                .or_else(|| entry.get("slug"))
                .and_then(Value::as_str)
        });
        let Some(id) = id.map(str::trim).filter(|id| !id.is_empty()) else {
            continue;
        };
        if all.iter().any(|existing| existing == id) {
            continue;
        }
        all.push(id.to_string());
        let image = entry
            .pointer("/capabilities/image_generation")
            .and_then(Value::as_bool)
            == Some(true)
            || entry.get("type").and_then(Value::as_str) == Some("image")
            || entry
                .get("output_modalities")
                .or_else(|| entry.pointer("/architecture/output_modalities"))
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().any(|v| v.as_str() == Some("image")));
        if image {
            images.push(id.to_string());
        } else if id.to_lowercase().contains("image") || id.to_lowercase().contains("diffusion") {
            candidates.push(id.to_string());
        }
    }
    ensure!(!all.is_empty(), "供应商没有返回可选模型");
    if images.is_empty() {
        Ok((
            candidates,
            "供应商未声明图片输出能力，按名称展示候选；请选择生图模型，获取不到时可手动填写模型编号".into(),
            false,
        ))
    } else {
        Ok((images, String::new(), true))
    }
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

pub(super) async fn generate(
    app: &AppHandle,
    settings: &ImageGenerationSettings,
    model: &str,
    prompt: &str,
    reference: Option<&[u8]>,
) -> Result<String> {
    if settings.provider == "novelai" {
        return super::novelai::generate(
            &settings.novelai,
            model,
            prompt,
            reference,
            settings.timeout_secs,
        )
        .await;
    }
    let conn = connection(app, settings)?;
    let http = build_proxied_client(settings.timeout_secs).await?;
    let route = if reference.is_some() {
        "images/edits"
    } else {
        "images/generations"
    };
    let mut request = http.post(format!("{}/{route}", conn.base));
    let mut secret = conn.key;
    if conn.codex {
        let credential = codex_auth::get_valid_credential(&http)
            .await?
            .context("请先在大模型管理中登录 Codex 订阅")?;
        secret = credential.access;
        request = request
            .header("chatgpt-account-id", credential.account_id)
            .header("originator", "codex_cli_rs");
    }
    if !secret.is_empty() {
        request = request.bearer_auth(&secret);
    }
    let mut body = json!({ "prompt": prompt, "model": model });
    if conn.codex {
        body["background"] = json!("auto");
        body["size"] = json!("auto");
        body["quality"] = json!("auto");
    }
    if !conn.codex {
        body["n"] = json!(1);
    }
    for (field, value) in [("size", &settings.size), ("quality", &settings.quality)] {
        if !value.is_empty() {
            body[field] = json!(value);
        }
    }
    let request = if let Some(image) = reference {
        if conn.codex {
            body["images"] = json!([{"image_url": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(image))}]);
            request.json(&body)
        } else {
            let mut form = reqwest::multipart::Form::new().part(
                "image",
                reqwest::multipart::Part::bytes(image.to_vec())
                    .file_name("reference.png")
                    .mime_str("image/png")?,
            );
            for (key, value) in body.as_object().unwrap() {
                form = form.text(
                    key.clone(),
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                );
            }
            request.multipart(form)
        }
    } else {
        request.json(&body)
    };
    let response = request.send().await.context("发送图片生成请求失败")?;
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
            message.to_string()
        } else {
            message.replace(&secret, "[redacted]")
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
        return Ok(encoded.to_string());
    }
    let url = image
        .get("url")
        .and_then(Value::as_str)
        .context("图片响应中没有 Base64 或下载地址")?;
    validate_url(url)?;
    // 下载图片时不转发生成接口的授权头。
    let download: Client = build_proxied_client(settings.timeout_secs).await?;
    let response = download.get(url).send().await?.error_for_status()?;
    Ok(base64::engine::general_purpose::STANDARD
        .encode(limited_body(response, MAX_RESPONSE_BYTES).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_catalog_is_dynamic_deduplicated_and_excludes_dated_variants() {
        let html = r#"<a href="/api/docs/models/gpt-image-81-fast">x</a><a href="/api/docs/models/gpt-image-82">y</a><a href="/api/docs/models/gpt-image-82-2099-01-01">z</a><a href="/api/docs/models/gpt-image-82">y</a>"#;
        assert_eq!(
            parse_official_catalog(html).unwrap(),
            ["gpt-image-82", "gpt-image-81-fast"]
        );
        assert!(parse_official_catalog("<html/>").is_err());
    }

    #[test]
    fn api_catalog_uses_output_capabilities_instead_of_model_names() {
        let (models, warning, automatic) = parse_api_catalog(&json!({"data": [
            {"id": "some-chat"}, {"id": "a-new-picture-model", "output_modalities": ["image"]}
        ]}))
        .unwrap();
        assert_eq!(models, ["a-new-picture-model"]);
        assert!(warning.is_empty());
        assert!(automatic);
        assert!(
            !parse_api_catalog(&json!({"data": [{"id": "unknown"}]}))
                .unwrap()
                .1
                .is_empty()
        );
    }

    #[test]
    fn credential_urls_are_rejected() {
        assert!(validate_url("https://secret@example.com/v1").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("http://127.0.0.1:8080/v1").is_ok());
    }
}
