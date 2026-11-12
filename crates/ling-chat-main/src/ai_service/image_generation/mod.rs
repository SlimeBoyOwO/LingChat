//! 背景页面与聊天工具共用的图片生成服务。

mod providers;

use std::io::Cursor;
use std::sync::LazyLock;

use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::AppState;
use crate::ai_service::game_system::world_actions;
use crate::ai_service::tools::atomic_replace;
use crate::api::scene::{CreateSceneRequest, SceneInfo, create_scene};

pub use providers::{ImageModelCatalog, ImageProviderInfo, list_models, list_providers};

static GENERATION: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageGenerationSettings {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub models_url: String,
    pub size: String,
    pub quality: String,
    pub timeout_secs: u64,
}

impl Default for ImageGenerationSettings {
    fn default() -> Self {
        Self {
            provider: providers::DEFAULT_PROVIDER.into(),
            model: String::new(),
            base_url: String::new(),
            api_key: String::new(),
            models_url: String::new(),
            size: String::new(),
            quality: String::new(),
            timeout_secs: 300,
        }
    }
}

impl ImageGenerationSettings {
    pub fn normalize(&mut self) {
        self.provider = self.provider.trim().to_string();
        if self.provider.is_empty() {
            self.provider = providers::DEFAULT_PROVIDER.into();
        }
        self.model = self.model.trim().to_string();
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        self.models_url = self.models_url.trim().to_string();
        self.size = self.size.trim().to_string();
        self.quality = self.quality.trim().to_string();
        self.timeout_secs = self.timeout_secs.clamp(30, 900);
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateImageRequest {
    pub prompt: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub apply_background: bool,
}

#[derive(Clone, Serialize)]
pub struct GeneratedImage {
    pub path: String,
    pub model: String,
    pub provider: String,
    pub scene: SceneInfo,
}

pub async fn generate(app: &AppHandle, request: GenerateImageRequest) -> Result<GeneratedImage> {
    ensure!(!request.prompt.trim().is_empty(), "请输入图片描述");
    ensure!(request.prompt.chars().count() <= 32_000, "图片描述过长");
    let _guard = GENERATION
        .try_lock()
        .map_err(|_| anyhow::anyhow!("已有图片正在生成，请稍后重试"))?;
    let mut settings = app.state::<AppState>().tool_settings.get().image_generation;
    settings.normalize();
    let model = if settings.model.is_empty() {
        list_models(app, &settings, false)
            .await?
            .models
            .into_iter()
            .next()
            .context("供应商未返回图片模型，请刷新模型列表或手动填写")?
    } else {
        settings.model.clone()
    };
    let payload = tokio::time::timeout(
        std::time::Duration::from_secs(settings.timeout_secs),
        providers::generate(app, &settings, &model, request.prompt.trim()),
    )
    .await
    .context("图片生成超时，请稍后检查供应商状态")??;
    let bytes = decode_image(&payload)?;
    let bytes = tokio::task::spawn_blocking(move || validate_image(&bytes))
        .await
        .context("图片解码任务失败")??;
    let file = format!("{}.png", Uuid::new_v4());
    let path = crate::api::backgrounds_dir().join(&file);
    atomic_replace(&path, &bytes).map_err(anyhow::Error::msg)?;
    let name = if request.name.trim().is_empty() {
        request.prompt.trim().chars().take(60).collect()
    } else {
        request.name.trim().chars().take(128).collect()
    };
    let scene = match create_scene(
        app.clone(),
        CreateSceneRequest {
            scene_name: name,
            scene_description: request.prompt,
            background: file,
            lighting: None,
        },
    )
    .await
    {
        Ok(scene) => scene,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(anyhow::anyhow!("注册生成图片失败：{error}"));
        },
    };
    if request.apply_background {
        world_actions::switch_scene(app, Some(scene.id.clone())).await;
    }
    let result = GeneratedImage {
        path: path.to_string_lossy().into_owned(),
        model,
        provider: settings.provider,
        scene,
    };
    let _ = app.emit("image-generation:completed", &result);
    Ok(result)
}

fn decode_image(encoded: &str) -> Result<Vec<u8>> {
    ensure!(
        encoded.len() <= providers::MAX_RESPONSE_BYTES,
        "图片数据超过大小限制"
    );
    base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .context("图片接口返回了无效 Base64 数据")
}

fn validate_image(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().context("供应商返回的内容不是有效图片")?;
    ensure!(
        u64::from(decoded.width()) * u64::from(decoded.height()) <= 32_000_000,
        "图片分辨率过大"
    );
    let mut output = Cursor::new(Vec::new());
    decoded.write_to(&mut output, image::ImageFormat::Png)?;
    ensure!(
        output.get_ref().len() <= providers::MAX_RESPONSE_BYTES,
        "图片数据超过大小限制"
    );
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_model_and_path_overrides() {
        assert!(
            serde_json::from_value::<GenerateImageRequest>(
                serde_json::json!({"prompt": "picture", "model": "override"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<GenerateImageRequest>(
                serde_json::json!({"prompt": "picture", "path": "../../outside.png"})
            )
            .is_err()
        );
    }

    #[test]
    fn provider_output_must_decode_as_an_image() {
        assert!(decode_image("!invalid base64").is_err());
        assert!(validate_image(b"<html>not an image</html>").is_err());
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        assert!(
            validate_image(png.get_ref())
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
    }
}
