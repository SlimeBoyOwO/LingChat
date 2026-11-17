use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use super::super::catalog::{cache_key, cached_catalog, fetch_api_catalog};
use super::super::http::{read_openai_image, validate_base_url, validate_url};
use super::super::provider::{ImageModelCatalog, ImageProvider};
use super::super::{GenerateImageRequest, ImageGenerationSettings};

pub struct OpenAiImageProvider {
    settings: ImageGenerationSettings,
}

impl OpenAiImageProvider {
    pub fn new(settings: &ImageGenerationSettings) -> Self {
        Self {
            settings: settings.clone(),
        }
    }
}

#[async_trait]
impl ImageProvider for OpenAiImageProvider {
    async fn list_models(&self, http: &Client, refresh: bool) -> Result<ImageModelCatalog> {
        let settings = &self.settings;
        validate_base_url(&settings.base_url)?;
        let source = if settings.models_url.is_empty() {
            format!("{}/models", settings.base_url)
        } else {
            settings.models_url.clone()
        };
        validate_url(&source)?;
        let key = cache_key(
            &format!("openai-compatible:{}", settings.base_url),
            &source,
            &settings.api_key,
        );
        cached_catalog(key, refresh, || {
            fetch_api_catalog(http, &source, &settings.base_url, &settings.api_key)
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
        let settings = &self.settings;
        validate_base_url(&settings.base_url)?;
        let route = if reference.is_some() {
            "images/edits"
        } else {
            "images/generations"
        };
        let mut http_request = http.post(format!("{}/{route}", settings.base_url));
        if !settings.api_key.is_empty() {
            http_request = http_request.bearer_auth(&settings.api_key);
        }
        let mut body = json!({"prompt": request.prompt.trim(), "model": model, "n": 1});
        for (field, value) in [("size", &settings.size), ("quality", &settings.quality)] {
            if !value.is_empty() {
                body[field] = json!(value);
            }
        }
        let http_request = if let Some(image) = reference {
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
            http_request.multipart(form)
        } else {
            http_request.json(&body)
        };
        let response = http_request.send().await.context("发送图片生成请求失败")?;
        read_openai_image(http, response, &settings.api_key).await
    }
}
