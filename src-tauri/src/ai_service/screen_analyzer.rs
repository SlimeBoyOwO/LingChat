//! 桌面截图分析器。
//! 独立的屏幕捕获与视觉语言模型(VLM)分析模块，可在多处复用（主动对话、脚本事件等）。
//!
//! 设计参考 Python 原版 `ling_chat_python/core/pic_analyzer.py` 的 DesktopAnalyzer。

use reqwest::Client;
use std::time::Instant;
use tauri::AppHandle;

use crate::ai_service::llm::provider_config::resolve_vision_provider;
use crate::ai_service::llm::vision::{self, VisionTarget};

/// 截屏分析的输出上限（token）。
const VISION_MAX_TOKENS: u32 = 512;
/// 视觉请求的 HTTP 读超时。
const ANALYZER_HTTP_TIMEOUT_SECS: u64 = 120;

/// 构造预配置的 reqwest Client（统一走 factory，复用 webpki-roots TLS 配置）。
fn build_analyzer_client() -> Client {
    match crate::ai_service::llm::factory::build_http_client(ANALYZER_HTTP_TIMEOUT_SECS) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("[ScreenAnalyzer] 构建 HTTP 客户端失败: {e}");
            Client::new()
        },
    }
}

/// 屏幕分析器的配置（从环境/Store 加载）。
#[derive(Clone, Debug)]
pub struct ScreenAnalyzerConfig {
    pub vd_api_key: String,
    pub vd_base_url: String,
    pub vd_model: String,
    /// 图片超过端点大小限制时是否自动压缩（全局开关 `llm.auto_compress_image`，默认 true）。
    pub auto_compress_oversize: bool,
}

impl Default for ScreenAnalyzerConfig {
    fn default() -> Self {
        Self {
            vd_api_key: String::new(),
            vd_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            // model 为空表示未配置任何视觉 provider，分析会被跳过
            vd_model: String::new(),
            auto_compress_oversize: true,
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
            vd_base_url: vision::vision_base_url(&provider),
            vd_model: provider.model.clone(),
            auto_compress_oversize: crate::config::app_config::AppConfig::load(app)
                .map(|c| c.auto_compress_image)
                .unwrap_or(true),
        }
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
            client: build_analyzer_client(),
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
        self.run_vision(prompt, &jpeg_bytes, "image/jpeg").await
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

        self.run_vision(prompt, image_bytes, "image/png").await
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
        let mime = if image_path.ends_with(".png") {
            "image/png"
        } else if image_path.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };

        self.run_vision(prompt, &bytes, mime).await
    }

    /// 调用视觉模型识别一张图片，记录耗时与用量后返回文本描述。
    async fn run_vision(&mut self, prompt: &str, image_bytes: &[u8], mime: &str) -> Option<String> {
        // reqwest::Client 内部为 Arc，克隆廉价；避免跨 await 借用 self.client。
        let client = self.client.clone();
        let target = VisionTarget {
            api_key: self.config.vd_api_key.clone(),
            base_url: self.config.vd_base_url.clone(),
            model: self.config.vd_model.clone(),
        };

        tracing::info!(
            "[ScreenAnalyzer] Sending image to VLM ({}) for analysis...",
            target.model
        );
        let start = Instant::now();
        let result = vision::analyze_image(
            &client,
            &target,
            prompt,
            image_bytes,
            mime,
            VISION_MAX_TOKENS,
            self.config.auto_compress_oversize,
        )
        .await;
        let elapsed = start.elapsed().as_secs_f64();

        match result {
            Ok(result) => {
                self.last_report = AnalysisReport {
                    response_time_secs: elapsed,
                    input_tokens: result.input_tokens,
                    output_tokens: result.output_tokens,
                };
                tracing::info!("[ScreenAnalyzer] Analysis success: {}", result.text);
                Some(result.text)
            },
            Err(e) => {
                self.last_report = AnalysisReport {
                    response_time_secs: elapsed,
                    ..Default::default()
                };
                tracing::error!("[ScreenAnalyzer] VLM analysis failed: {e}");
                None
            },
        }
    }
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
/// - 无法识别格式 / 尺寸非法返回 `None`（调用方自然回退到旁白转述）。
/// - `compress.enabled == false` 时**原图直发**：不解码重编码，仅识别格式并按原始字节
///   编码 base64，保留原分辨率与清晰度（用户默认偏好）。但图片超出端点硬限制
///   （`utils::image` 的 32 MiB / 8192px）且 `auto_compress` 开启时，仍强制压缩，避免
///   整条请求被服务端拒绝。`auto_compress`（全局开关 `llm.auto_compress_image`）关闭时
///   跳过该超限检查，超限图片按原样直发。
/// - `compress.enabled == true` 时统一转 JPEG、等比缩放到 `max_edge`（钳制到端点上限）、
///   透明通道压白底：既减小 base64 体积，也规避 WebP/PNG 在部分 OpenAI 兼容端点的兼容问题。
/// - **仅用于当轮请求**，不写入长期记忆（配合 `GeneratorDeps::transient_image`）。
pub fn image_bytes_to_native_data_url(
    image_bytes: &[u8],
    compress: NativeImageCompress,
    auto_compress: bool,
) -> Option<String> {
    use crate::utils::image::{MAX_INLINE_IMAGE_BYTES, MAX_INLINE_IMAGE_EDGE};

    // 一次头部探测同时拿到格式与尺寸：格式供原图直发推断 MIME，尺寸判断是否超限。
    let reader = image::ImageReader::new(std::io::Cursor::new(image_bytes))
        .with_guessed_format()
        .ok()?;
    let detected_format = reader.format();
    let (w, h) = reader.into_dimensions().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    // 超限检查仅在 `auto_compress` 开启时生效；关闭时按原样直发。
    let oversize = auto_compress
        && (image_bytes.len() > MAX_INLINE_IMAGE_BYTES || w.max(h) > MAX_INLINE_IMAGE_EDGE);

    // ─── 原图直发：未开压缩且未超端点限制时，保留原始格式与字节 ───
    if !compress.enabled && !oversize {
        // 根据识别出的真实格式推断 MIME；未知格式统一按 png 兜底。
        let mime = match detected_format {
            Some(image::ImageFormat::Jpeg) => "jpeg",
            Some(image::ImageFormat::WebP) => "webp",
            Some(_) | None => "png",
        };
        let b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, image_bytes);
        return Some(format!("data:image/{mime};base64,{b64}"));
    }
    if oversize && !compress.enabled {
        tracing::info!(
            "[Vision] 原图超出端点限制（{w}x{h}、{} 字节），强制压缩后直发对话模型。",
            image_bytes.len()
        );
    }

    // ─── 压缩路径：等比缩放到 max_edge（钳制到端点上限）、转 JPEG、透明压白 ───
    let target_edge = compress.max_edge.min(MAX_INLINE_IMAGE_EDGE);
    let bytes =
        crate::utils::image::recompress_jpeg(image_bytes, target_edge, compress.jpeg_quality)
            .ok()?;
    let b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &bytes);
    Some(format!("data:image/jpeg;base64,{b64}"))
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
