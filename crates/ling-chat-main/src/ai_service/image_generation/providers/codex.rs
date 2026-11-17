use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use base64::Engine;
use regex::Regex;
use reqwest::Client;
use serde_json::json;

use super::super::catalog::{cache_key, cached_catalog, fetch_api_catalog};
use super::super::http::{limited_body, read_openai_image, validate_url};
use super::super::provider::{ImageModelCatalog, ImageProvider};
use super::super::{GenerateImageRequest, ImageGenerationSettings};
use crate::ai_service::llm::codex_auth;

const CODEX_BASE: &str = "https://chatgpt.com/backend-api/codex";
const CODEX_CATALOG: &str = "https://developers.openai.com/api/docs/models";

pub struct CodexImageProvider {
    settings: ImageGenerationSettings,
}

impl CodexImageProvider {
    pub fn new(settings: &ImageGenerationSettings) -> Self {
        Self {
            settings: settings.clone(),
        }
    }
}

#[async_trait]
impl ImageProvider for CodexImageProvider {
    async fn list_models(&self, http: &Client, refresh: bool) -> Result<ImageModelCatalog> {
        let source = if self.settings.models_url.is_empty() {
            CODEX_CATALOG
        } else {
            &self.settings.models_url
        };
        validate_url(source)?;
        cached_catalog(cache_key("codex", source, ""), refresh, || async {
            // 模型目录不接收 Codex 订阅凭据。
            if !self.settings.models_url.is_empty() {
                return fetch_api_catalog(http, source, CODEX_BASE, "").await;
            }
            let response = http
                .get(source)
                .timeout(Duration::from_secs(30))
                .send()
                .await
                .context("请求图片模型目录失败")?;
            ensure!(
                response.status().is_success(),
                "模型目录返回 HTTP {}",
                response.status()
            );
            let bytes = limited_body(response, 4 * 1024 * 1024).await?;
            Ok(ImageModelCatalog {
                models: parse_official_catalog(std::str::from_utf8(&bytes)?)?,
                source: source.into(),
                warning: "公开图片目录；实际调用权限以订阅账号回执为准".into(),
                allow_automatic: true,
                novelai_metadata: None,
            })
        })
        .await
    }

    async fn generate(
        &self,
        http: &Client,
        model: &str,
        request: &GenerateImageRequest,
        reference: Option<&[u8]>,
    ) -> Result<String> {
        let credential = codex_auth::get_valid_credential(http)
            .await?
            .context("请先在大模型管理中登录 Codex 订阅")?;
        let route = if reference.is_some() {
            "images/edits"
        } else {
            "images/generations"
        };
        let mut body = json!({
            "prompt": request.prompt.trim(), "model": model,
            "background": "auto", "size": "auto", "quality": "auto"
        });
        for (field, value) in [
            ("size", &self.settings.size),
            ("quality", &self.settings.quality),
        ] {
            if !value.is_empty() {
                body[field] = json!(value);
            }
        }
        if let Some(image) = reference {
            body["images"] = json!([{"image_url": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(image))}]);
        }
        let response = http
            .post(format!("{CODEX_BASE}/{route}"))
            .bearer_auth(&credential.access)
            .header("chatgpt-account-id", credential.account_id)
            .header("originator", "codex_cli_rs")
            .json(&body)
            .send()
            .await
            .context("发送图片生成请求失败")?;
        read_openai_image(http, response, &credential.access).await
    }
}

fn parse_official_catalog(html: &str) -> Result<Vec<String>> {
    let pattern = Regex::new(r#"/api/docs/models/(gpt-image-[a-z0-9.-]+)"#)?;
    let dated = Regex::new(r"-\d{4}-\d{2}-\d{2}$")?;
    let mut seen = HashSet::new();
    let mut models: Vec<_> = pattern
        .captures_iter(html)
        .filter_map(|matched| {
            let id = matched[1].to_string();
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
