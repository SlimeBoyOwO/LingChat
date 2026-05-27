//! 桌面截图分析器。
//! 独立的屏幕捕获与视觉语言模型(VLM)分析模块，可在多处复用（主动对话、脚本事件等）。
//!
//! 设计参考 Python 原版 `ling_chat_python/core/pic_analyzer.py` 的 DesktopAnalyzer。

use reqwest::Client;
use serde_json::Value;
use std::time::Instant;

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
            client: Client::new(),
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
        let api_key = &self.config.vd_api_key;
        if api_key.is_empty() {
            tracing::warn!("[ScreenAnalyzer] VD_API_KEY is empty, skipping screenshot analysis.");
            return None;
        }

        let jpeg_bytes = capture_screen_as_jpeg()?;

        let (base64, mime) = encode_image_base64(&jpeg_bytes, "jpeg");
        self.call_vlm(prompt, &base64, &mime).await
    }

    /// 分析任意图片字节（支持 JPEG / PNG / WebP 等格式）。
    /// 供脚本事件、文件分析等外部调用方使用。
    pub async fn analyze_image(
        &mut self,
        image_bytes: &[u8],
        prompt: &str,
    ) -> Option<String> {
        let api_key = &self.config.vd_api_key;
        if api_key.is_empty() {
            tracing::warn!("[ScreenAnalyzer] VD_API_KEY is empty, skipping image analysis.");
            return None;
        }

        let (base64, mime) = encode_image_base64(image_bytes, "png");
        self.call_vlm(prompt, &base64, &mime).await
    }

    /// 分析本地图片文件路径。
    pub async fn analyze_image_file(
        &mut self,
        image_path: &str,
        prompt: &str,
    ) -> Option<String> {
        let api_key = &self.config.vd_api_key;
        if api_key.is_empty() {
            tracing::warn!("[ScreenAnalyzer] VD_API_KEY is empty, skipping image file analysis.");
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
            }
            Err(e) => {
                tracing::error!("[ScreenAnalyzer] Failed to send request to VLM: {:?}", e);
            }
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

/// 捕获整个桌面并返回 JPEG 格式的字节。
/// Windows: 使用 GDI (BitBlt + GetDIBits) 捕获，然后压缩为 1024x768 JPEG。
/// 其他平台: 返回 None。
pub fn capture_screen_as_jpeg() -> Option<Vec<u8>> {
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
