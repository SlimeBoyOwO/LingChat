//! 桌面截图分析器。
//! 独立的屏幕捕获与视觉语言模型(VLM)分析模块，可在多处复用（主动对话、脚本事件等）。
//!
//! 设计参考 Python 原版 `ling_chat_python/core/pic_analyzer.py` 的 DesktopAnalyzer。

use reqwest::Client;
use serde_json::Value;
use std::time::Instant;
use tauri::AppHandle;

use crate::ai_service::llm::provider_config::{LlmProviderConfig, resolve_vision_provider};

/// 构造预配置的 reqwest Client（TLS 见 crate::utils::tls::build_tls_config）。
fn build_vlm_client() -> Client {
    let tls_config = crate::utils::tls::build_tls_config().expect("TLS 配置失败");
    Client::builder()
        .tls_backend_preconfigured(tls_config)
        .build()
        .expect("reqwest client 构建失败")
}

/// 屏幕分析器的配置（从环境/Store 加载）。
#[derive(Clone, Debug)]
pub struct ScreenAnalyzerConfig {
    pub vd_api_key: String,
    pub vd_base_url: String,
    pub vd_model: String,
}

impl Default for ScreenAnalyzerConfig {
    fn default() -> Self {
        Self {
            vd_api_key: String::new(),
            vd_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            // model 为空表示未配置任何视觉 provider，分析会被跳过
            vd_model: String::new(),
        }
    }
}

impl ScreenAnalyzerConfig {
    /// 从大模型管理解析视觉分析配置：
    /// 优先使用「视觉模型」角色指定的 provider，缺省时跟随对话模型；
    /// 都没有可用配置时返回默认值（model 为空，分析会被跳过）。
    pub fn resolve(app: &AppHandle) -> Self {
        let Some(provider) = resolve_vision_provider(app) else {
            return Self::default();
        };

        // 截图分析固定走 OpenAI 兼容的 chat/completions + image_url 协议
        if !matches!(
            provider.provider.as_str(),
            "openai" | "deepseek" | "lmstudio" | "kimicode"
        ) {
            tracing::warn!(
                "[ScreenAnalyzer] Provider '{}' may not support the OpenAI-compatible vision API",
                provider.provider
            );
        }

        Self {
            vd_api_key: provider.api_key.clone(),
            vd_base_url: vision_base_url(&provider),
            vd_model: provider.model.clone(),
        }
    }
}

/// 将 provider 的 base_url 适配为视觉分析使用的 OpenAI 兼容端点前缀
/// （请求时拼接 `{base}/chat/completions`）。
/// Kimi Code 的聊天入口是 Anthropic 兼容协议，视觉请求需要改用
/// 官方提供的 OpenAI 兼容入口。
fn vision_base_url(provider: &LlmProviderConfig) -> String {
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
        },
        "openai" if base.is_empty() => "https://api.openai.com/v1".to_string(),
        "deepseek" if base.is_empty() => "https://api.deepseek.com".to_string(),
        _ => base.to_string(),
    }
}

/// 最后一次分析的性能与用量报告。
#[derive(Clone, Debug, Default)]
pub struct AnalysisReport {
    pub response_time_secs: f64,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

pub struct ScreenAnalyzer {
    config: ScreenAnalyzerConfig,
    client: Client,
    last_report: AnalysisReport,
}

impl ScreenAnalyzer {
    pub fn new(config: ScreenAnalyzerConfig) -> Self {
        Self {
            config,
            client: build_vlm_client(),
            last_report: AnalysisReport::default(),
        }
    }

    /// 允许运行时更新配置（例如用户修改了 Store 设置后）。
    pub fn update_config(&mut self, config: ScreenAnalyzerConfig) {
        self.config = config;
    }

    /// 获取最后一次分析的性能报告。
    pub fn get_report(&self) -> &AnalysisReport {
        &self.last_report
    }

    /// 核心方法：截屏 → 发送给 VLM 分析 → 返回文本描述。
    /// 这是策略分发器和主动对话系统的主要入口。
    pub async fn analyze_screen(&mut self, prompt: &str) -> Option<String> {
        if self.config.vd_model.is_empty() {
            tracing::warn!(
                "[ScreenAnalyzer] Vision provider model is not configured, skipping screenshot analysis."
            );
            return None;
        }

        let jpeg_bytes = capture_screen_as_jpeg()?;

        let (base64, mime) = encode_image_base64(&jpeg_bytes, "jpeg");
        self.call_vlm(prompt, &base64, &mime).await
    }

    /// 分析任意图片字节（支持 JPEG / PNG / WebP 等格式）。
    /// 供脚本事件、文件分析等外部调用方使用。
    pub async fn analyze_image(&mut self, image_bytes: &[u8], prompt: &str) -> Option<String> {
        if self.config.vd_model.is_empty() {
            tracing::warn!(
                "[ScreenAnalyzer] Vision provider model is not configured, skipping image analysis."
            );
            return None;
        }

        let (base64, mime) = encode_image_base64(image_bytes, "png");
        self.call_vlm(prompt, &base64, &mime).await
    }

    /// 分析本地图片文件路径。
    pub async fn analyze_image_file(&mut self, image_path: &str, prompt: &str) -> Option<String> {
        if self.config.vd_model.is_empty() {
            tracing::warn!(
                "[ScreenAnalyzer] Vision provider model is not configured, skipping image file analysis."
            );
            return None;
        }

        let bytes = std::fs::read(image_path).ok()?;

        // 根据扩展名推断 MIME
        let mime_type = if image_path.ends_with(".png") {
            "png"
        } else if image_path.ends_with(".webp") {
            "webp"
        } else {
            "jpeg"
        };

        let (base64, mime) = encode_image_base64(&bytes, mime_type);
        self.call_vlm(prompt, &base64, &mime).await
    }

    /// 调用视觉语言模型 API。
    async fn call_vlm(
        &mut self,
        prompt: &str,
        base64_image: &str,
        mime_type: &str,
    ) -> Option<String> {
        let image_url = format!("data:image/{};base64,{}", mime_type, base64_image);
        let model = &self.config.vd_model;

        let payload = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {"type": "image_url", "image_url": {"url": image_url}}
                    ]
                }
            ],
            "max_tokens": 512
        });

        tracing::info!(
            "[ScreenAnalyzer] Sending image to VLM ({}) for analysis...",
            model
        );

        let start = Instant::now();

        let api_key = &self.config.vd_api_key;
        let endpoint = format!("{}/chat/completions", self.config.vd_base_url);

        let res = self
            .client
            .post(&endpoint)
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .await;

        let elapsed = start.elapsed().as_secs_f64();

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(json_res) = response.json::<Value>().await {
                        let content = json_res["choices"][0]["message"]["content"]
                            .as_str()
                            .map(|s| s.to_string());

                        let usage = &json_res["usage"];
                        self.last_report = AnalysisReport {
                            response_time_secs: elapsed,
                            input_tokens: usage["prompt_tokens"].as_u64().map(|n| n as u32),
                            output_tokens: usage["completion_tokens"].as_u64().map(|n| n as u32),
                        };

                        if let Some(ref c) = content {
                            tracing::info!("[ScreenAnalyzer] Analysis success: {}", c);
                        }

                        return content;
                    }
                } else {
                    let err_text = response.text().await.unwrap_or_default();
                    tracing::error!(
                        "[ScreenAnalyzer] VLM API returned error status: {}",
                        err_text
                    );
                }
            },
            Err(e) => {
                tracing::error!("[ScreenAnalyzer] Failed to send request to VLM: {:?}", e);
            },
        }

        self.last_report = AnalysisReport {
            response_time_secs: elapsed,
            ..Default::default()
        };

        None
    }
}

/// 将图片字节编码为 Base64，返回 (base64_string, mime_type)。
fn encode_image_base64(bytes: &[u8], mime_type: &str) -> (String, String) {
    let b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, bytes);
    (b64, mime_type.to_string())
}

/// 原生识图发送给对话模型的图片处理参数。
/// 默认**不压缩、原图直发**；`enabled` 为 true 时才做缩放 + JPEG 压缩。
#[derive(Clone, Copy, Debug)]
pub struct NativeImageCompress {
    /// 是否开启压缩。false = 原图直发（保留原始格式与分辨率）。
    pub enabled: bool,
    /// 压缩时图片最大边长（像素），超宽图等比缩放。
    pub max_edge: u32,
    /// 压缩时 JPEG 编码质量（0-100）。
    pub jpeg_quality: u8,
}

impl Default for NativeImageCompress {
    fn default() -> Self {
        Self {
            enabled: false,
            max_edge: 2048,
            jpeg_quality: 85,
        }
    }
}

/// 把任意图片字节转换为适合原生多模态识图的 `data:image/...;base64,...` data URL。
///
/// - 解码失败 / 超限返回 `None`（调用方自然回退到旁白转述）。
/// - `compress.enabled == false` 时**原图直发**：不解码重现压缩，仅识别格式并
///   按原始字节编码 base64，保留原分辨率与清晰度（用户默认偏好）。
/// - `compress.enabled == true` 时统一转 JPEG 并等比缩放到 `max_edge`，透明通道
///   压到白底：既减小 base64 体积（token/缓存占用），也避免 WebP/PNG 在某些
///   OpenAI 兼容视觉端点上的兼容问题。
/// - **仅用于当轮请求**，不写入长期记忆（配合 `GeneratorDeps::transient_image`）。
pub fn image_bytes_to_native_data_url(
    image_bytes: &[u8],
    compress: NativeImageCompress,
) -> Option<String> {
    use image::imageops::FilterType;
    use image::{DynamicImage, GenericImageView, ImageReader};

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);

    // `ImageReader::limits` 就地修改接收者并返回 `()`，不能链式接 `.decode()`，
    // 因此先构造 reader、设置限制、再单独调用 decode（与 read_media_file.rs 一致）。
    // 注意：`decode` 会消费 reader（`self`），原图直发路径要用格式推断 MIME，
    // 所以必须在 decode 之前先取回 `format()`。
    let mut reader = ImageReader::new(std::io::Cursor::new(image_bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(limits);
    let detected_format = reader.format();
    let img = reader.decode().ok()?;

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }

    // ─── 原图直发：不缩放不重编码，保留原始格式字节 ───
    if !compress.enabled {
        // 根据识别出的真实格式推断 MIME；未知格式统一按 png 兜底。
        let mime = match detected_format {
            Some(image::ImageFormat::Jpeg) => "jpeg",
            Some(image::ImageFormat::WebP) => "webp",
            Some(_) | None => "png",
        };
        let b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, image_bytes);
        return Some(format!("data:image/{mime};base64,{b64}"));
    }

    // ─── 压缩路径：等比缩放到 max_edge，转 JPEG ───
    let max_edge = compress.max_edge.max(1);
    let resized = if w.max(h) > max_edge {
        img.resize(max_edge, max_edge, FilterType::Lanczos3)
    } else {
        img
    };

    // 透明通道压白，避免半透明图转 JPEG 后出现黑底/花边
    let rgb = flatten_on_white(&resized);

    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, compress.jpeg_quality)
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .ok()?;
    let bytes = out.into_inner();

    let b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &bytes);
    Some(format!("data:image/jpeg;base64,{b64}"))
}

/// 将 RGBA 图像压到白色背景上返回 RGB，供 JPEG 编码前去除透明通道。
fn flatten_on_white(image: &image::DynamicImage) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = image::ImageBuffer::new(w, h);
    for (x, y, px) in rgba.enumerate_pixels() {
        let [r, g, b, a] = px.0;
        // 线性加权：alpha=255 → 原色，alpha=0 → 白
        let blend = |c: u8| -> u8 {
            let c = c as u32;
            let a = a as u32;
            ((c * a + 255 * (255 - a)) / 255) as u8
        };
        out.put_pixel(x, y, image::Rgb([blend(r), blend(g), blend(b)]));
    }
    out
}

/// 捕获整个桌面并返回 JPEG 格式的字节。
/// Windows: 使用 GDI (BitBlt + GetDIBits) 捕获，然后压缩为 1024x768 JPEG。
/// 其他平台: 返回 None。
pub fn capture_screen_as_jpeg() -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{
            BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS,
            DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        unsafe {
            let hdc_screen = GetDC(None);
            if hdc_screen.is_invalid() {
                return None;
            }
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            if w <= 0 || h <= 0 {
                ReleaseDC(None, hdc_screen);
                return None;
            }

            let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
            let hbitmap = CreateCompatibleBitmap(hdc_screen, w, h);

            let old_obj = SelectObject(hdc_mem, hbitmap.into());
            let _ = BitBlt(hdc_mem, 0, 0, w, h, Some(hdc_screen), 0, 0, SRCCOPY);

            let mut bmi = windows::Win32::Graphics::Gdi::BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 24;
            bmi.bmiHeader.biCompression = 0;

            let mut buffer = vec![0u8; (w * h * 3) as usize];

            let lines = GetDIBits(
                hdc_screen,
                hbitmap,
                0,
                h as u32,
                Some(buffer.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc_mem, old_obj);
            let _ = DeleteObject(hbitmap.into());
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);

            if lines <= 0 {
                return None;
            }

            for chunk in buffer.chunks_exact_mut(3) {
                chunk.swap(0, 2);
            }

            let img =
                image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w as u32, h as u32, buffer)?;
            let dynamic_img = image::DynamicImage::ImageRgb8(img);
            let resized = dynamic_img.resize(1024, 768, image::imageops::FilterType::Triangle);

            let mut jpeg_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
            let _ = resized.write_to(&mut cursor, image::ImageFormat::Jpeg);

            Some(jpeg_bytes)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 捕获全分辨率桌面截图，不缩放。
/// 用于截图覆盖层的精确像素映射（覆盖层需要 1:1 像素坐标）。
#[cfg(desktop)]
pub fn capture_screen_raw_jpeg() -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{
            BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS,
            DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        unsafe {
            let hdc_screen = GetDC(None);
            if hdc_screen.is_invalid() {
                return None;
            }
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            if w <= 0 || h <= 0 {
                ReleaseDC(None, hdc_screen);
                return None;
            }

            let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
            let hbitmap = CreateCompatibleBitmap(hdc_screen, w, h);

            let old_obj = SelectObject(hdc_mem, hbitmap.into());
            let _ = BitBlt(hdc_mem, 0, 0, w, h, Some(hdc_screen), 0, 0, SRCCOPY);

            let mut bmi = windows::Win32::Graphics::Gdi::BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 24;
            bmi.bmiHeader.biCompression = 0;

            let mut buffer = vec![0u8; (w * h * 3) as usize];

            let lines = GetDIBits(
                hdc_screen,
                hbitmap,
                0,
                h as u32,
                Some(buffer.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc_mem, old_obj);
            let _ = DeleteObject(hbitmap.into());
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);

            if lines <= 0 {
                return None;
            }

            for chunk in buffer.chunks_exact_mut(3) {
                chunk.swap(0, 2);
            }

            let img =
                image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w as u32, h as u32, buffer)?;
            let dynamic_img = image::DynamicImage::ImageRgb8(img);

            let mut jpeg_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
            let _ = dynamic_img.write_to(&mut cursor, image::ImageFormat::Jpeg);

            Some(jpeg_bytes)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
