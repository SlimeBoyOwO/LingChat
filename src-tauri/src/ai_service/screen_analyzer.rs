//! 桌面截图分析器。
//! 独立的屏幕捕获与视觉语言模型(VLM)分析模块，可在多处复用（主动对话、脚本事件等）。
//!
//! 设计参考 Python 原版 `ling_chat_python/core/pic_analyzer.py` 的 DesktopAnalyzer，
//! 并参考 N.E.K.O 与 Kimi Code 的截图清晰度、图片验证、窗口标题上下文等做法。

use reqwest::Client;
use serde_json::Value;
use std::time::Instant;

use crate::ai_service::llm::provider_config::resolve_chat_provider;
use crate::config::proactive::ProactiveConfig;

/// 安全限制：原始图片最大 10MB（base64 后约 13.3MB）。
const MAX_IMAGE_SIZE_BYTES: usize = 10 * 1024 * 1024;
const MAX_BASE64_SIZE: usize = MAX_IMAGE_SIZE_BYTES * 4 / 3 + 100;
/// 视觉模型输出 token 上限。
const VISION_MAX_TOKENS: u32 = 1024;
/// 图片像素数安全上限（避免超大图片耗尽内存）。
const MAX_IMAGE_PIXELS: u64 = 100_000_000;

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
            vd_model: "qwen3.5-plus".to_string(),
        }
    }
}

/// 图片压缩目标参数。
/// 参考 Kimi Code 的 `compressImageForModel`：在视觉可接受的前提下控制 token 消耗。
#[derive(Clone, Debug)]
pub struct CompressionTarget {
    /// 长边最大像素（默认 1024）。
    pub max_dimension: u32,
    /// JPEG 编码后目标最大字节数（默认 1.5 MB）。
    pub max_bytes: usize,
    /// 最低 JPEG 质量（默认 60）。
    pub min_quality: u8,
    /// 最高 JPEG 质量（默认 90）。
    pub max_quality: u8,
}

impl Default for CompressionTarget {
    fn default() -> Self {
        Self {
            max_dimension: 1024,
            max_bytes: 1536 * 1024, // 1.5 MiB
            min_quality: 60,
            max_quality: 90,
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
    compression: CompressionTarget,
}

impl ScreenAnalyzer {
    pub fn new(config: ScreenAnalyzerConfig) -> Self {
        Self {
            config,
            client: Client::new(),
            last_report: AnalysisReport::default(),
            compression: CompressionTarget::default(),
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
        let api_key = &self.config.vd_api_key;
        if api_key.is_empty() {
            tracing::warn!("[ScreenAnalyzer] VD_API_KEY is empty, skipping screenshot analysis.");
            return None;
        }

        let jpeg_bytes = capture_screen_as_jpeg_with_compression(&self.compression)?;
        tracing::info!(
            "[ScreenAnalyzer] Captured screenshot: {} bytes (quality-aware compression applied)",
            jpeg_bytes.len()
        );

        let window_title = get_active_window_title();
        let enriched_prompt = build_screen_prompt(prompt, window_title.as_deref());

        let (base64, mime) = encode_image_base64(&jpeg_bytes, "jpeg");
        self.call_vlm(&enriched_prompt, &base64, &mime).await
    }

    /// 分析任意图片字节（支持 JPEG / PNG / WebP 等格式）。
    /// 供脚本事件、文件分析等外部调用方使用。
    pub async fn analyze_image(&mut self, image_bytes: &[u8], prompt: &str) -> Option<String> {
        let api_key = &self.config.vd_api_key;
        if api_key.is_empty() {
            tracing::warn!("[ScreenAnalyzer] VD_API_KEY is empty, skipping image analysis.");
            return None;
        }

        if image_bytes.len() > MAX_IMAGE_SIZE_BYTES {
            tracing::error!(
                "[ScreenAnalyzer] Image too large: {} bytes > {}",
                image_bytes.len(),
                MAX_IMAGE_SIZE_BYTES
            );
            return None;
        }

        let compressed = compress_image_bytes(image_bytes, &self.compression).ok()?;
        let mime_type = if matches!(
            image::guess_format(&compressed).ok(),
            Some(image::ImageFormat::Png)
        ) {
            "png"
        } else {
            "jpeg"
        };

        let (base64, mime) = encode_image_base64(&compressed, mime_type);
        self.call_vlm(prompt, &base64, &mime).await
    }

    /// 分析本地图片文件路径。
    pub async fn analyze_image_file(&mut self, image_path: &str, prompt: &str) -> Option<String> {
        let api_key = &self.config.vd_api_key;
        if api_key.is_empty() {
            tracing::warn!("[ScreenAnalyzer] VD_API_KEY is empty, skipping image file analysis.");
            return None;
        }

        let bytes = std::fs::read(image_path).ok()?;
        self.analyze_image(&bytes, prompt).await
    }

    /// 调用视觉语言模型 API。
    async fn call_vlm(
        &mut self,
        prompt: &str,
        base64_image: &str,
        mime_type: &str,
    ) -> Option<String> {
        if base64_image.len() > MAX_BASE64_SIZE {
            tracing::error!(
                "[ScreenAnalyzer] Base64 image too large: {} bytes > {}",
                base64_image.len(),
                MAX_BASE64_SIZE
            );
            return None;
        }

        let image_url = format!("data:image/{};base64,{}", mime_type, base64_image);
        let model = &self.config.vd_model;

        let payload = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": VISION_SYSTEM_PROMPT
                },
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {"type": "image_url", "image_url": {"url": image_url}}
                    ]
                }
            ],
            "max_tokens": VISION_MAX_TOKENS
        });

        tracing::info!(
            "[ScreenAnalyzer] Sending image to VLM ({}) for analysis...",
            model
        );

        let start = Instant::now();

        let api_key = &self.config.vd_api_key;
        let endpoint = normalize_endpoint(&self.config.vd_base_url);

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
                let status = response.status();
                if status.is_success() {
                    match response.json::<Value>().await {
                        Ok(json_res) => {
                            let content = json_res["choices"][0]["message"]["content"]
                                .as_str()
                                .map(|s| s.to_string());

                            let usage = &json_res["usage"];
                            self.last_report = AnalysisReport {
                                response_time_secs: elapsed,
                                input_tokens: usage["prompt_tokens"].as_u64().map(|n| n as u32),
                                output_tokens: usage["completion_tokens"]
                                    .as_u64()
                                    .map(|n| n as u32),
                            };

                            if let Some(ref c) = content {
                                tracing::info!("[ScreenAnalyzer] Analysis success: {}", c);
                            } else {
                                tracing::warn!(
                                    "[ScreenAnalyzer] VLM response missing content: {:?}",
                                    json_res
                                );
                            }

                            return content;
                        }
                        Err(e) => {
                            tracing::error!(
                                "[ScreenAnalyzer] Failed to parse VLM JSON response: {:?}",
                                e
                            );
                        }
                    }
                } else {
                    let err_text = response.text().await.unwrap_or_default();
                    tracing::error!(
                        "[ScreenAnalyzer] VLM API returned error status {}: {}",
                        status,
                        err_text
                    );
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    tracing::error!(
                        "[ScreenAnalyzer] VLM request timed out (default 30s without explicit timeout): {:?}",
                        e
                    );
                } else if e.is_connect() {
                    tracing::error!(
                        "[ScreenAnalyzer] VLM connection failed, check base_url/network: {:?}",
                        e
                    );
                } else {
                    tracing::error!("[ScreenAnalyzer] Failed to send request to VLM: {:?}", e);
                }
            }
        }

        self.last_report = AnalysisReport {
            response_time_secs: elapsed,
            ..Default::default()
        };

        None
    }
}

/// VLM 系统提示：让模型知道自己在做什么，以及忽略聊天窗口。
const VISION_SYSTEM_PROMPT: &str = "你是一个桌面画面观察者。用户授权你查看他的屏幕截图，请用简洁的中文描述画面主体内容。\
如果截图里包含用户与 AI 的聊天窗口或角色立绘对话框，请不要描述这部分，只描述桌面上的其他内容。\
如果提供了当前窗口标题，可以结合标题理解用户正在做什么。";

/// 把用户提示与当前窗口标题结合，给 VLM 更丰富的上下文。
fn build_screen_prompt(base_prompt: &str, window_title: Option<&str>) -> String {
    match window_title {
        Some(title) if !title.is_empty() => {
            format!("{}\n\n[当前焦点窗口标题]：{}", base_prompt, title)
        }
        _ => base_prompt.to_string(),
    }
}

/// 根据 ProactiveConfig 构建 ScreenAnalyzerConfig。
/// 当 `VD_FOLLOW_CHAT_MODEL` 为 true 时，复用当前对话模型（chat provider）的 API Key、Base URL 和模型名；
/// 如果对话模型未配置或不可用，则回退到独立的视觉模型配置并记录警告。
pub fn build_screen_analyzer_config(
    app_handle: &tauri::AppHandle,
    config: &ProactiveConfig,
) -> ScreenAnalyzerConfig {
    if config.vd_follow_chat_model {
        if let Some(provider) = resolve_chat_provider(app_handle) {
            tracing::info!(
                "[ScreenAnalyzer] VD follows chat model: {} ({})",
                provider.label,
                provider.model
            );
            return ScreenAnalyzerConfig {
                vd_api_key: provider.api_key,
                vd_base_url: provider.base_url,
                vd_model: provider.model,
            };
        } else {
            tracing::warn!(
                "[ScreenAnalyzer] VD_FOLLOW_CHAT_MODEL is enabled but no usable chat provider found, falling back to dedicated VD config"
            );
        }
    }

    ScreenAnalyzerConfig {
        vd_api_key: config.vd_api_key.clone(),
        vd_base_url: config.vd_base_url.clone(),
        vd_model: config.vd_model.clone(),
    }
}

/// 将图片字节编码为 Base64，返回 (base64_string, mime_type)。
fn encode_image_base64(bytes: &[u8], mime_type: &str) -> (String, String) {
    let b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, bytes);
    (b64, mime_type.to_string())
}

/// 确保 Base URL 末尾没有斜杠，以便正确拼接 `/chat/completions`。
fn normalize_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.is_empty() {
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".to_string()
    } else {
        format!("{}/chat/completions", trimmed)
    }
}

/// 验证并加载图片，检查尺寸与完整性。
fn validate_image_bytes(bytes: &[u8]) -> anyhow::Result<image::DynamicImage> {
    if bytes.len() > MAX_IMAGE_SIZE_BYTES {
        anyhow::bail!(
            "image too large: {} bytes > {}",
            bytes.len(),
            MAX_IMAGE_SIZE_BYTES
        );
    }

    let img = image::load_from_memory(bytes).map_err(|e| anyhow::anyhow!("invalid image: {}", e))?;

    let (w, h) = (img.width() as u64, img.height() as u64);
    if w.saturating_mul(h) > MAX_IMAGE_PIXELS {
        anyhow::bail!(
            "image too large: {}x{} = {} pixels > {}",
            w,
            h,
            w * h,
            MAX_IMAGE_PIXELS
        );
    }

    Ok(img)
}

/// 压缩图片字节到目标尺寸与大小。
/// 先按 `max_dimension` 等比缩放，再用 JPEG 质量迭代控制文件大小。
fn compress_image_bytes(
    bytes: &[u8],
    target: &CompressionTarget,
) -> anyhow::Result<Vec<u8>> {
    let img = validate_image_bytes(bytes)?;
    compress_dynamic_image(&img, target)
}

/// 压缩 `DynamicImage` 到目标尺寸与大小。
fn compress_dynamic_image(
    img: &image::DynamicImage,
    target: &CompressionTarget,
) -> anyhow::Result<Vec<u8>> {
    // 统一转换为 RGB8，避免 RGBA/P 模式带来的兼容性问题
    let rgb_img = img.to_rgb8();
    let dynamic_rgb = image::DynamicImage::ImageRgb8(rgb_img);

    let (orig_w, orig_h) = (dynamic_rgb.width(), dynamic_rgb.height());
    let max_dim = target.max_dimension;

    let resized = if orig_w.max(orig_h) > max_dim {
        dynamic_rgb.resize(max_dim, max_dim, image::imageops::FilterType::Triangle)
    } else {
        dynamic_rgb
    };

    let (new_w, new_h) = (resized.width(), resized.height());

    // 二分查找合适的 JPEG 质量
    let mut low = target.min_quality;
    let mut high = target.max_quality;
    let mut best = Vec::new();

    while low <= high {
        let mid = (low + high) / 2;
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor,
            mid,
        );
        encoder.encode_image(&resized)?;

        if buf.len() <= target.max_bytes {
            best = buf;
            if high == mid {
                break;
            }
            low = mid + 1;
        } else {
            if low == mid {
                // 已经达到最低质量仍超大小，返回当前最佳（或最低质量结果）
                if best.is_empty() {
                    best = buf;
                }
                break;
            }
            high = mid - 1;
        }
    }

    if best.is_empty() {
        // 兜底：用最低质量写一次
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut cursor,
            target.min_quality,
        );
        encoder.encode_image(&resized)?;
        best = buf;
    }

    tracing::info!(
        "[ScreenAnalyzer] Compressed image: {}x{} -> {}x{}, {} bytes",
        orig_w,
        orig_h,
        new_w,
        new_h,
        best.len()
    );

    Ok(best)
}

/// 获取当前前台窗口标题（Windows）。
/// 用于给 VLM 提供“用户正在看什么窗口”的上下文。
#[cfg(target_os = "windows")]
fn get_active_window_title() -> Option<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == HWND(std::ptr::null_mut()) {
            return None;
        }

        // 先获取长度
        let len = GetWindowTextW(hwnd, &mut []);
        if len <= 0 {
            return None;
        }

        let mut buffer = vec![0u16; (len + 1) as usize];
        let written = GetWindowTextW(hwnd, &mut buffer);
        if written <= 0 {
            return None;
        }

        // 去掉末尾的 null
        buffer.truncate(written as usize);
        String::from_utf16(&buffer).ok()
    }
}

#[cfg(not(target_os = "windows"))]
fn get_active_window_title() -> Option<String> {
    None
}

/// 捕获整个桌面并返回经过压缩的 JPEG 字节。
/// Windows: 使用 GDI (BitBlt + GetDIBits) 捕获，然后压缩为 1024x768 JPEG。
/// 其他平台: 返回 None。
#[allow(dead_code)]
pub fn capture_screen_as_jpeg() -> Option<Vec<u8>> {
    capture_screen_as_jpeg_with_compression(&CompressionTarget::default())
}

fn capture_screen_as_jpeg_with_compression(target: &CompressionTarget) -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            GetDIBits, ReleaseDC, SelectObject, BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        unsafe {
            let hdc_screen = GetDC(None);
            if hdc_screen.is_invalid() {
                tracing::error!("[ScreenAnalyzer] Failed to get screen DC.");
                return None;
            }
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            if w <= 0 || h <= 0 {
                tracing::error!("[ScreenAnalyzer] Invalid screen size: {}x{}", w, h);
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
                tracing::error!("[ScreenAnalyzer] GetDIBits returned {} lines.", lines);
                return None;
            }

            for chunk in buffer.chunks_exact_mut(3) {
                chunk.swap(0, 2);
            }

            let img = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w as u32, h as u32, buffer)?;
            let dynamic_img = image::DynamicImage::ImageRgb8(img);

            match compress_dynamic_image(&dynamic_img, target) {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    tracing::error!("[ScreenAnalyzer] Failed to compress screenshot: {:?}", e);
                    None
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        tracing::warn!("[ScreenAnalyzer] Screen capture is only supported on Windows.");
        None
    }
}

/// 捕获全分辨率桌面截图，不缩放。
/// 用于截图覆盖层的精确像素映射（覆盖层需要 1:1 像素坐标）。
pub fn capture_screen_raw_jpeg() -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            GetDIBits, ReleaseDC, SelectObject, BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 冒烟测试：验证 Windows 下能成功截屏并压缩到目标大小以内。
    /// 该测试不需要调用任何外部 API，因此可以在没有 API Key 的情况下运行。
    #[test]
    fn test_capture_and_compress_smoke() {
        #[cfg(target_os = "windows")]
        {
            let target = CompressionTarget {
                max_dimension: 1024,
                max_bytes: 1536 * 1024,
                min_quality: 60,
                max_quality: 90,
            };
            let jpeg = capture_screen_as_jpeg_with_compression(&target)
                .expect("screen capture should succeed on Windows");
            assert!(!jpeg.is_empty(), "captured JPEG should not be empty");
            assert!(
                jpeg.len() <= target.max_bytes,
                "captured JPEG ({} bytes) should not exceed target size ({} bytes)",
                jpeg.len(),
                target.max_bytes
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            // 非 Windows 平台该功能直接返回 None，验证行为即可。
            assert!(capture_screen_as_jpeg().is_none());
        }
    }
}
