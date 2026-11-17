//! 背景页面与聊天工具共用的图片生成服务。
//! 通过供应商接口编排生成、图片校验、背景落盘及场景注册。

mod catalog;
mod factory;
mod http;
mod provider;
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
use crate::utils::proxy::build_proxied_client;

pub use factory::list_providers;
pub use provider::{ImageModelCatalog, ImagePromptHints, ImageProviderInfo};
pub use providers::NovelAiSettings;

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
    pub novelai: NovelAiSettings,
}

impl Default for ImageGenerationSettings {
    fn default() -> Self {
        Self {
            provider: factory::DEFAULT_PROVIDER.into(),
            model: String::new(),
            base_url: String::new(),
            api_key: String::new(),
            models_url: String::new(),
            size: String::new(),
            quality: String::new(),
            timeout_secs: 300,
            novelai: NovelAiSettings::default(),
        }
    }
}

impl ImageGenerationSettings {
    pub fn normalize(&mut self) {
        self.provider = self.provider.trim().to_string();
        if self.provider.is_empty() {
            self.provider = factory::DEFAULT_PROVIDER.into();
        }
        self.model = self.model.trim().to_string();
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        self.models_url = self.models_url.trim().to_string();
        self.size = self.size.trim().to_string();
        self.quality = self.quality.trim().to_string();
        self.timeout_secs = self.timeout_secs.clamp(30, 900);
        self.novelai.normalize();
    }

    pub fn validate(&self) -> Result<()> {
        factory::create_image_provider(self)?.validate()
    }
}

pub fn prompt_hints(settings: &ImageGenerationSettings) -> ImagePromptHints {
    factory::create_image_provider(settings)
        .map(|provider| provider.prompt_hints())
        .unwrap_or_default()
}

pub async fn list_models(
    _app: &AppHandle,
    settings: &ImageGenerationSettings,
    refresh: bool,
) -> Result<ImageModelCatalog> {
    let provider = factory::create_image_provider(settings)?;
    let http = build_proxied_client(30).await?;
    provider.list_models(&http, refresh).await
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateImageRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub apply_background: bool,
    #[serde(default)]
    pub reference_image: Option<String>,
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
    let provider = factory::create_image_provider(&settings)?;
    provider.validate()?;
    if let Some(negative) = request.negative_prompt.as_deref() {
        ensure!(
            provider.prompt_hints().negative_prompt.is_some(),
            "当前图片供应商不支持独立负向 tags"
        );
        ensure!(negative.chars().count() <= 32_000, "负向 tags 过长");
    }
    let reference = if let Some(encoded) = request.reference_image.as_deref() {
        let bytes = decode_image(encoded)?;
        Some(
            tokio::task::spawn_blocking(move || validate_image(&bytes))
                .await
                .context("参考图解码失败")??,
        )
    } else {
        None
    };
    let http = build_proxied_client(settings.timeout_secs).await?;
    let model = if settings.model.is_empty() {
        let catalog = provider
            .list_models(&http, false)
            .await
            .context("无法获取图片模型目录，请手动填写模型编号")?;
        ensure!(
            catalog.allow_automatic,
            "供应商没有确认图片输出能力，请先选择或填写生图模型编号"
        );
        catalog
            .models
            .into_iter()
            .next()
            .context("供应商未返回图片模型，请刷新模型列表或手动填写")?
    } else {
        settings.model.clone()
    };
    let payload = tokio::time::timeout(
        std::time::Duration::from_secs(settings.timeout_secs),
        provider.generate(&http, &model, &request, reference.as_deref()),
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
        encoded.len() <= http::MAX_RESPONSE_BYTES,
        "图片数据超过大小限制"
    );
    let encoded = encoded.trim();
    let encoded = if encoded.starts_with("data:") {
        let (header, data) = encoded.split_once(',').context("图片 data URL 无效")?;
        ensure!(
            header.starts_with("data:image/") && header.ends_with(";base64"),
            "图片必须使用 Base64 图像格式"
        );
        data
    } else {
        encoded
    };
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
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
        output.get_ref().len() <= http::MAX_RESPONSE_BYTES,
        "图片数据超过大小限制"
    );
    Ok(output.into_inner())
}
