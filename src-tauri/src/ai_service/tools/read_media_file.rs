//! `ReadMediaFile`：读取本地图片或视频，并交给已配置的视觉模型识别。
//!
//! 路径规则与 Read/Write/Edit 共用：普通模式只能读取 data/ 沙箱，完全访问模式
//! 才允许任意路径。图片会按设置缩放、转为 JPEG 后发送；WebP/GIF 保留原格式。
//! 视频使用 OpenAI 兼容的 `video_url` 数据 URL，是否可用取决于视觉模型供应商。

use std::io::Cursor;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageReader, Rgb};
use reqwest::Client;
use serde_json::{json, Value};

use crate::ai_service::llm::provider_config::resolve_vision_provider;
use crate::ai_service::skill_agent::config::SkillAgentConfig;
use crate::ai_service::skill_agent::file_tools::FileTools;
use crate::ai_service::types::ToolDefinition;
use crate::utils::tls::build_tls_config;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::settings::{MediaFileSettings, SharedToolSettings};

const HARD_MAX_MEDIA_BYTES: u64 = 100 * 1024 * 1024;
const MAX_DECODE_DIMENSION: u32 = 32_768;
const MAX_DECODE_ALLOCATION: u64 = 384 * 1024 * 1024;
const MEDIA_TOOL_TIMEOUT: Duration = Duration::from_secs(240);

#[derive(Clone, Copy)]
enum MediaKind {
    Image(&'static str),
    Video(&'static str),
}

impl MediaKind {
    fn label(self) -> &'static str {
        match self {
            Self::Image(_) => "image",
            Self::Video(_) => "video",
        }
    }
}

#[derive(Clone, Copy)]
struct ImageRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

struct PreparedImage {
    bytes: Vec<u8>,
    mime: &'static str,
    original_width: Option<u32>,
    original_height: Option<u32>,
    delivered_width: Option<u32>,
    delivered_height: Option<u32>,
}

pub struct ReadMediaFileTool {
    settings: SharedToolSettings,
}

impl ReadMediaFileTool {
    pub fn new(settings: SharedToolSettings) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl Tool for ReadMediaFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "ReadMediaFile",
            "读取图片或视频文件并使用 LingChat 的视觉模型识别内容。支持按提示分析图片/视频；图片可用 region 查看局部细节，full_resolution=true 跳过默认缩放。相对路径基于 data/ 文件沙箱，完全访问模式可读取任意绝对路径。",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "图片或视频文件路径；相对路径基于 data/ 文件沙箱"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "希望视觉模型重点识别或回答的问题；省略时使用工具设置中的默认提示词"
                    },
                    "region": {
                        "type": "object",
                        "properties": {
                            "x": {"type": "integer", "description": "裁剪区域左上角 X 坐标"},
                            "y": {"type": "integer", "description": "裁剪区域左上角 Y 坐标"},
                            "width": {"type": "integer", "description": "裁剪宽度"},
                            "height": {"type": "integer", "description": "裁剪高度"}
                        },
                        "required": ["x", "y", "width", "height"],
                        "additionalProperties": false
                    },
                    "full_resolution": {
                        "type": "boolean",
                        "description": "图片专用：跳过默认最长边缩放；大图会增加请求体和视觉 token"
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(MEDIA_TOOL_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let settings = self.settings.get();
        let media_settings = settings.media_file.clone();
        let path = required_string(&arguments, "path")?;
        let prompt = arguments
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(media_settings.default_prompt.trim())
            .to_string();
        let region = parse_region(&arguments)?;
        let full_resolution = arguments
            .get("full_resolution")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let skill_config = SkillAgentConfig::load(&app);
        let files = FileTools {
            sandbox_dir: skill_config.resolve_sandbox_dir(),
            allow_any_path: settings.allows_any_path(),
        };
        let safe_path = files
            .sanitize(path)
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        if !safe_path.is_file() {
            return Err(ToolError::Execution(format!(
                "媒体文件不存在: {}",
                safe_path.display()
            )));
        }

        let metadata = tokio::fs::metadata(&safe_path)
            .await
            .map_err(|error| ToolError::Execution(format!("读取媒体文件信息失败: {error}")))?;
        if metadata.len() == 0 {
            return Err(ToolError::Execution("媒体文件为空".into()));
        }
        let configured_max = u64::from(media_settings.max_file_mb) * 1024 * 1024;
        let max_bytes = configured_max.min(HARD_MAX_MEDIA_BYTES);
        if metadata.len() > max_bytes {
            return Err(ToolError::Execution(format!(
                "媒体文件为 {} 字节，超过当前设置的 {} MB 上限",
                metadata.len(),
                media_settings.max_file_mb
            )));
        }

        let data = tokio::fs::read(&safe_path)
            .await
            .map_err(|error| ToolError::Execution(format!("读取媒体文件失败: {error}")))?;
        let media_kind = detect_media_kind(&data).ok_or_else(|| {
            ToolError::Execution(
                "不支持的媒体格式；图片支持 JPEG/PNG/WebP/GIF，视频支持 MP4/MOV/WebM/MKV/AVI/MPEG"
                    .into(),
            )
        })?;

        match media_kind {
            MediaKind::Image(_) if !media_settings.image_enabled => {
                return Err(ToolError::Execution("工具设置已关闭图片识别".into()));
            }
            MediaKind::Video(_) if !media_settings.video_enabled => {
                return Err(ToolError::Execution("工具设置已关闭视频识别".into()));
            }
            MediaKind::Video(_) if region.is_some() || full_resolution => {
                return Err(ToolError::InvalidArguments(
                    "region 和 full_resolution 仅适用于图片".into(),
                ));
            }
            _ => {}
        }

        let (media_bytes, mime, dimensions) = match media_kind {
            MediaKind::Image(mime) => {
                let config = media_settings.clone();
                let prepared = tokio::task::spawn_blocking(move || {
                    prepare_image(data, mime, &config, region, full_resolution)
                })
                .await
                .map_err(|error| ToolError::Execution(format!("图片处理任务异常: {error}")))??;
                let dimensions = json!({
                    "original_width": prepared.original_width,
                    "original_height": prepared.original_height,
                    "delivered_width": prepared.delivered_width,
                    "delivered_height": prepared.delivered_height,
                });
                (prepared.bytes, prepared.mime, dimensions)
            }
            MediaKind::Video(mime) => (data, mime, Value::Null),
        };

        let provider = resolve_vision_provider(&app).ok_or_else(|| {
            ToolError::Execution(
                "没有可用的视觉模型；请先在“高级设置 → 大模型管理”配置视觉模型".into(),
            )
        })?;
        let analysis = analyze_media(
            &provider,
            media_kind,
            &media_bytes,
            mime,
            &prompt,
            media_settings.max_output_tokens,
        )
        .await?;

        Ok(json!({
            "ok": true,
            "path": safe_path.display().to_string(),
            "kind": media_kind.label(),
            "mime_type": mime,
            "source_bytes": metadata.len(),
            "delivered_bytes": media_bytes.len(),
            "dimensions": dimensions,
            "vision_model": provider.model,
            "analysis": analysis,
        }))
    }
}

fn required_string<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ToolError::InvalidArguments(format!("缺少 {key} 参数")))
}

fn parse_region(arguments: &Value) -> Result<Option<ImageRegion>, ToolError> {
    let Some(region) = arguments.get("region") else {
        return Ok(None);
    };
    let read = |key: &str| -> Result<u32, ToolError> {
        let value = region
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| ToolError::InvalidArguments(format!("region.{key} 必须是非负整数")))?;
        u32::try_from(value)
            .map_err(|_| ToolError::InvalidArguments(format!("region.{key} 超出范围")))
    };
    let parsed = ImageRegion {
        x: read("x")?,
        y: read("y")?,
        width: read("width")?,
        height: read("height")?,
    };
    if parsed.width == 0 || parsed.height == 0 {
        return Err(ToolError::InvalidArguments(
            "region.width 和 region.height 必须大于 0".into(),
        ));
    }
    Ok(Some(parsed))
}

fn detect_media_kind(bytes: &[u8]) -> Option<MediaKind> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(MediaKind::Image("image/jpeg"));
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(MediaKind::Image("image/png"));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(MediaKind::Image("image/gif"));
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(MediaKind::Image("image/webp"));
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        let mime = if brand == b"qt  " {
            "video/quicktime"
        } else {
            "video/mp4"
        };
        return Some(MediaKind::Video(mime));
    }
    if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        let header = &bytes[..bytes.len().min(4096)];
        return Some(MediaKind::Video(
            if header.windows(4).any(|part| part == b"webm") {
                "video/webm"
            } else {
                "video/x-matroska"
            },
        ));
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"AVI " {
        return Some(MediaKind::Video("video/x-msvideo"));
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0xBA]) || bytes.starts_with(&[0x00, 0x00, 0x01, 0xB3])
    {
        return Some(MediaKind::Video("video/mpeg"));
    }
    None
}

pub(crate) fn is_supported_media_file(path: &Path) -> bool {
    use std::io::Read;

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 4096];
    let Ok(read) = file.read(&mut header) else {
        return false;
    };
    detect_media_kind(&header[..read]).is_some()
}

fn prepare_image(
    bytes: Vec<u8>,
    source_mime: &'static str,
    settings: &MediaFileSettings,
    region: Option<ImageRegion>,
    full_resolution: bool,
) -> Result<PreparedImage, ToolError> {
    if !matches!(source_mime, "image/jpeg" | "image/png") {
        if region.is_some() || full_resolution {
            return Err(ToolError::Execution(
                "WebP/GIF 当前仅支持默认整图识别；region/full_resolution 请先转换为 JPEG 或 PNG"
                    .into(),
            ));
        }
        return Ok(PreparedImage {
            bytes,
            mime: source_mime,
            original_width: None,
            original_height: None,
            delivered_width: None,
            delivered_height: None,
        });
    }

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| ToolError::Execution(format!("识别图片格式失败: {error}")))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DECODE_DIMENSION);
    limits.max_image_height = Some(MAX_DECODE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOCATION);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| ToolError::Execution(format!("解码图片失败: {error}")))?;
    let (original_width, original_height) = decoded.dimensions();
    let mut image = if let Some(region) = region {
        let right = region
            .x
            .checked_add(region.width)
            .ok_or_else(|| ToolError::InvalidArguments("region 横向范围溢出".into()))?;
        let bottom = region
            .y
            .checked_add(region.height)
            .ok_or_else(|| ToolError::InvalidArguments("region 纵向范围溢出".into()))?;
        if right > original_width || bottom > original_height {
            return Err(ToolError::InvalidArguments(format!(
                "region 超出原图范围（原图 {}x{}）",
                original_width, original_height
            )));
        }
        decoded.crop_imm(region.x, region.y, region.width, region.height)
    } else {
        decoded
    };

    if !full_resolution {
        let (width, height) = image.dimensions();
        let max_edge = settings.image_max_edge;
        if width.max(height) > max_edge {
            image = image.resize(max_edge, max_edge, FilterType::Lanczos3);
        }
    }
    let rgb = flatten_on_white(&image);
    let (delivered_width, delivered_height) = rgb.dimensions();
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, settings.jpeg_quality)
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .map_err(|error| ToolError::Execution(format!("压缩图片失败: {error}")))?;

    Ok(PreparedImage {
        bytes: output,
        mime: "image/jpeg",
        original_width: Some(original_width),
        original_height: Some(original_height),
        delivered_width: Some(delivered_width),
        delivered_height: Some(delivered_height),
    })
}

fn flatten_on_white(image: &DynamicImage) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let rgba = image.to_rgba8();
    ImageBuffer::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        let alpha = u16::from(pixel[3]);
        let blend = |channel: u8| -> u8 {
            (((u16::from(channel) * alpha) + (255 * (255 - alpha))) / 255) as u8
        };
        Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])])
    })
}

async fn analyze_media(
    provider: &crate::ai_service::llm::provider_config::LlmProviderConfig,
    kind: MediaKind,
    bytes: &[u8],
    mime: &str,
    prompt: &str,
    max_output_tokens: u32,
) -> Result<String, ToolError> {
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    let data_url = format!("data:{mime};base64,{data}");
    let media_part = match kind {
        MediaKind::Image(_) => json!({
            "type": "image_url",
            "image_url": {"url": data_url}
        }),
        MediaKind::Video(_) => json!({
            "type": "video_url",
            "video_url": {"url": data_url}
        }),
    };
    let payload = json!({
        "model": provider.model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                media_part
            ]
        }],
        "max_tokens": max_output_tokens,
    });
    let tls = build_tls_config().map_err(ToolError::Execution)?;
    let client = Client::builder()
        .tls_backend_preconfigured(tls)
        .timeout(Duration::from_secs(210))
        .build()
        .map_err(|error| ToolError::Execution(format!("创建视觉请求客户端失败: {error}")))?;
    let endpoint = format!(
        "{}/chat/completions",
        vision_base_url(provider).trim_end_matches('/')
    );
    let response = client
        .post(endpoint)
        .bearer_auth(&provider.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|error| ToolError::Execution(format!("视觉模型请求失败: {error}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ToolError::Execution(format!("读取视觉模型响应失败: {error}")))?;
    if !status.is_success() {
        let detail: String = body.chars().take(2000).collect();
        let compatibility = if matches!(kind, MediaKind::Video(_)) {
            "；当前视觉模型可能不支持 OpenAI 兼容的 video_url 输入，可关闭视频识别或改用支持视频的视觉模型"
        } else {
            ""
        };
        return Err(ToolError::Execution(format!(
            "视觉模型返回 HTTP {}: {}{}",
            status.as_u16(),
            detail,
            compatibility
        )));
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| ToolError::Execution(format!("解析视觉模型响应失败: {error}")))?;
    extract_response_text(&value)
        .ok_or_else(|| ToolError::Execution("视觉模型响应中没有可用的文本识别结果".into()))
}

fn vision_base_url(
    provider: &crate::ai_service::llm::provider_config::LlmProviderConfig,
) -> String {
    let base = provider.base_url.trim().trim_end_matches('/');
    match provider.provider.as_str() {
        "kimicode" => {
            if base.is_empty() {
                "https://api.kimi.com/coding/v1".to_string()
            } else if base.ends_with("/v1/messages") {
                base.trim_end_matches("/messages").to_string()
            } else if base.ends_with("/v1/chat/completions") {
                base.trim_end_matches("/chat/completions").to_string()
            } else if base.ends_with("/v1") {
                base.to_string()
            } else {
                format!("{base}/v1")
            }
        }
        "openai" if base.is_empty() => "https://api.openai.com/v1".to_string(),
        "deepseek" if base.is_empty() => "https://api.deepseek.com".to_string(),
        _ => base.to_string(),
    }
}

fn extract_response_text(value: &Value) -> Option<String> {
    let message = value.get("choices")?.get(0)?.get("message")?;
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        let trimmed = content.trim();
        return (!trimmed.is_empty()).then(|| trimmed.to_string());
    }
    let parts = message.get("content")?.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| {
            part.get("text")
                .and_then(Value::as_str)
                .or_else(|| part.get("content").and_then(Value::as_str))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
