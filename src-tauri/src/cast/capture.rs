//! 投屏窗口捕获与 JPEG 编码。
//!
//! Windows 下用 `tauri_plugin_screenshots` 的 `capture_own_window` 直接抓本进程
//! 的投屏窗口（cast）画面（复用 `api/save.rs` 的 `capture_main_window_screenshot`
//! 范式——xcap 会按 PID 过滤掉本进程自己的窗口，只有这条路能抓）。
//! 其它平台 v1 返回明确错误，串流端兜底为纯黑帧。

use image::RgbaImage;

/// 捕获投屏窗口（cast）的当前画面。
#[cfg(target_os = "windows")]
pub fn capture_cast_window(app: &tauri::AppHandle) -> Result<RgbaImage, String> {
    use tauri::Manager;

    let win = app
        .get_webview_window("cast")
        .ok_or_else(|| "投屏窗口未打开".to_string())?;
    let hwnd = win.hwnd().map_err(|e| format!("获取窗口句柄失败: {e}"))?;
    // HWND.0 → *mut c_void → usize → u32（Windows 句柄是 32 位值）
    let id = hwnd.0 as usize as u32;
    tauri_plugin_screenshots::windows::capture_own_window(id)
        .map_err(|e| format!("捕获投屏窗口失败: {e}"))
}

/// 非 Windows 平台的占位实现（命令始终可注册，但捕获返回错误）。
#[cfg(not(target_os = "windows"))]
pub fn capture_cast_window(_app: &tauri::AppHandle) -> Result<RgbaImage, String> {
    Err("投屏窗口捕获目前仅支持 Windows".to_string())
}

/// 等比缩放 + 居中贴到 w×h 黑底画布（letterbox）。
///
/// 与 `temp/cast_sender.py` 保持一致：绝不拉伸/裁剪，保持完整画面不变形，
/// 多余部分以黑色填充，适配不同宽高比的小屏设备（ESP32 / HoloCubic 等）。
pub fn resize_letterbox(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if img.width() == w && img.height() == h {
        return img.clone();
    }
    let src_w = img.width() as f32;
    let src_h = img.height() as f32;
    let scale = (w as f32 / src_w).min(h as f32 / src_h);
    let new_w = ((src_w * scale).round() as u32).max(1);
    let new_h = ((src_h * scale).round() as u32).max(1);
    let resized = image::imageops::resize(img, new_w, new_h, image::imageops::FilterType::Triangle);
    let mut canvas = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
    let x = (w - new_w) / 2;
    let y = (h - new_h) / 2;
    image::imageops::overlay(&mut canvas, &resized, x as i64, y as i64);
    canvas
}

/// 应用「vivid」色彩预设（复刻 `temp/cast_sender.py` 的 `--vivid`）：
/// 先按亮度插值提升饱和度 ×`saturation`，再提升对比度 ×`contrast`。
///
/// 饱和度与 PIL `ImageEnhance.Color` 同源：`out = lum + (color - lum) * sat`，
/// 亮度用 ITU-R 601 加权（0.299R + 0.587G + 0.114B）；对比度用 image crate 内建
/// `colorops::contrast`（`c * (x - 0.5) + 0.5`）。应在 resize 之后、JPEG 编码之前调用。
pub fn apply_vivid(img: &RgbaImage, saturation: f32, contrast: f32) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        let [r, g, b, a] = p.0;
        let (fr, fg, fb) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let lum = 0.299 * fr + 0.587 * fg + 0.114 * fb;
        let blend = |c: f32| (lum + (c - lum) * saturation).clamp(0.0, 1.0);
        p.0 = [
            (blend(fr) * 255.0).round() as u8,
            (blend(fg) * 255.0).round() as u8,
            (blend(fb) * 255.0).round() as u8,
            a,
        ];
    }
    image::imageops::colorops::contrast(&out, contrast)
}

/// 编码为 JPEG 字节（quality 1–100）。
pub fn encode_jpeg(img: &RgbaImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100));
    enc.encode_image(img)
        .map_err(|e| format!("JPEG 编码失败: {e}"))?;
    Ok(out)
}
