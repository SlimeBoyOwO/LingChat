//! 图像处理通用工具：原生识图 / 旁白转述 / 文件分析共享的解码、缩放、JPEG 编码。
//!
//! 集中内联图片的端点硬限制（见 [`MAX_INLINE_IMAGE_BYTES`] / [`MAX_INLINE_IMAGE_EDGE`]）
//! 判断与兜底压缩，避免多处各写一套 clamp 逻辑导致阈值漂移。

use std::borrow::Cow;

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView as _, ImageBuffer, ImageReader, Rgb};

/// 内联图片的端点硬限制（DeepSeek 等 OpenAI 兼容视觉端点）：
/// 单张内联图片解码后 ≤ 32 MiB、单边最长 8192 像素，超出会被服务端直接拒绝。
/// 原生识图当轮最多携带一张图，单边取 8192（单请求 ≥15 张时才降为 4096，不适用）。
pub const MAX_INLINE_IMAGE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_INLINE_IMAGE_EDGE: u32 = 8192;

/// 解码前的防御性上限：拦截病态超大图，避免 `image` 解码时内存爆炸。
const DECODE_GUARD_EDGE: u32 = 16_384;

/// 仅读图片头获取宽高，不解码像素。
pub fn probe_dimensions(image_bytes: &[u8]) -> Result<(u32, u32)> {
    ImageReader::new(std::io::Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| anyhow!("识别图片格式失败: {e}"))?
        .into_dimensions()
        .map_err(|e| anyhow!("读取图片尺寸失败: {e}"))
}

/// 解码图片（应用防御性尺寸上限）。
fn decode_guarded(image_bytes: &[u8]) -> Result<DynamicImage> {
    let mut reader = ImageReader::new(std::io::Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| anyhow!("识别图片格式失败: {e}"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(DECODE_GUARD_EDGE);
    limits.max_image_height = Some(DECODE_GUARD_EDGE);
    reader.limits(limits);
    reader.decode().map_err(|e| anyhow!("解码图片失败: {e}"))
}

/// 把 RGBA 压到白底返回 RGB（JPEG 无透明通道，避免半透明图转 JPEG 出黑底/花边）。
pub fn flatten_on_white(image: &DynamicImage) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let rgba = image.to_rgba8();
    ImageBuffer::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let alpha = u16::from(a);
        // 线性加权：alpha=255 → 原色，alpha=0 → 白
        let blend =
            |c: u8| -> u8 { (((u16::from(c) * alpha) + (255 * (255 - alpha))) / 255) as u8 };
        Rgb([blend(r), blend(g), blend(b)])
    })
}

/// 等比缩放到 `target_edge`（单边上限）以内并转 JPEG，透明通道压白。
pub fn recompress_jpeg(image_bytes: &[u8], target_edge: u32, jpeg_quality: u8) -> Result<Vec<u8>> {
    let img = decode_guarded(image_bytes)?;
    let (w, h) = img.dimensions();
    let target_edge = target_edge.max(1);
    let resized = if w.max(h) > target_edge {
        img.resize(
            target_edge,
            target_edge,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };
    let rgb = flatten_on_white(&resized);
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, jpeg_quality)
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .map_err(|e| anyhow!("JPEG 编码失败: {e}"))?;
    Ok(out.into_inner())
}

/// 是否超出内联端点硬限制（字节 或 单边像素）。探测尺寸失败时按「未超限」处理，
/// 交给请求端自行判定，避免在此因解析失败阻断正常图片。
pub fn is_over_inline_limits(image_bytes: &[u8]) -> bool {
    if image_bytes.len() > MAX_INLINE_IMAGE_BYTES {
        return true;
    }
    probe_dimensions(image_bytes)
        .map(|(w, h)| w.max(h) > MAX_INLINE_IMAGE_EDGE)
        .unwrap_or(false)
}

/// VLM 内联请求前的统一钳制：未超限原样返回（保留 `mime`）；超限则缩放重编码为 JPEG。
/// 返回 `(bytes, mime)`，未改动时借用原字节避免拷贝。
/// `auto_compress` 为全局开关（对应 `llm.auto_compress_image`）：关闭时跳过整个检查，
/// 原样返回、由调用方按原始字节发送（超限可能被服务端拒绝）。
pub fn clamp_to_inline_limits<'a>(
    image_bytes: &'a [u8],
    mime: &'a str,
    auto_compress: bool,
) -> Result<(Cow<'a, [u8]>, Cow<'a, str>)> {
    if !auto_compress || !is_over_inline_limits(image_bytes) {
        return Ok((Cow::Borrowed(image_bytes), Cow::Borrowed(mime)));
    }
    // 字节超限（如高清大图）时目标边长更保守（4096），确保重编码后必然落回 32 MiB 内；
    // 仅单边超限时按端点上限缩放即可。
    let target_edge = if image_bytes.len() > MAX_INLINE_IMAGE_BYTES {
        4096
    } else {
        MAX_INLINE_IMAGE_EDGE
    };
    let bytes = recompress_jpeg(image_bytes, target_edge, 85)?;
    tracing::info!(
        "[Image] 内联图片超出端点限制（{} 字节），已重编码为 JPEG {} 字节。",
        image_bytes.len(),
        bytes.len()
    );
    Ok((Cow::Owned(bytes), Cow::Borrowed("image/jpeg")))
}
