//! NovelAI 图像生成客户端（直连官方端点，不依赖任何本地 Python 服务）。
//!
//! 协议对照 novelai-python / novelai-sdk 两个 Python SDK 的实际实现：
//! `POST {base}/ai/generate-image`，Bearer 认证，响应是一个 **ZIP**，里面装着 PNG。
//!
//! # 几个真机验证过、猜不出来的点
//!
//! - **响应是 ZIP 不是图片**：解不出 ZIP 时要退回「整个 body 当作单张图」再试一次，
//!   某些错误路径会直接回图。
//! - **V4 系模型的提示词走 `v4_prompt`**：此时 `parameters.prompt` 必须留空，
//!   但顶层 `input` 与 `parameters.negative_prompt` 照常填。V3 反过来。
//! - **null 字段要整个省掉**：SDK 用 `exclude_none=True` 序列化，
//!   显式送 null 会被服务端拒，所以这里全部 `skip_serializing_if`。
//! - **模型名有两个陷阱拼写**：`nai-diffusion-4-curated` 与 `nai-diffusion-3-furry`
//!   都不存在，正确的是 `-curated-preview` 与 `nai-diffusion-furry-3`。
//! - **NovelAI 有并发锁**：连续打会回 429，所以全局串行化（见 [`generation_lock`]）。

use std::io::{Cursor, Read};
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use reqwest::{Client, StatusCode};
use serde::Serialize;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::ai_service::tools::settings::ImageGenSettings;

/// 单次生成的网络超时。NovelAI 出图通常 10–30 秒，慢的时候更久。
const GENERATE_TIMEOUT: Duration = Duration::from_secs(180);

/// 品质标，与 SDK 的 `QUALITY_TAGS` 一致。
const QUALITY_TAGS: &str = ", very aesthetic, masterpiece, no text";

/// 负面预设文本，抄自 SDK 的 `UNDESIRED_CONTENT_PRESETS`。
/// 服务端只收 `ucPreset` 整数，这些文字是客户端自己拼进负面提示词的 —— 少了这步
/// 出图质量会和官方界面对不上。
fn uc_preset_text(preset: &str) -> &'static str {
    match preset {
        "strong" => ", lowres, artistic error, film grain, scan artifacts, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, dithering, halftone, screentone, multiple views, logo, too many watermarks, negative space, blank page, ",
        "furry_focus" => ", {worst quality}, distracting watermark, unfinished, bad quality, {widescreen}, upscale, {sequence}, {{grandfathered content}}, blurred foreground, chromatic aberration, sketch, everyone, [sketch background], simple, [flat colors], ych (character), outline, multiple scenes, [[horror (theme)]], comic, ",
        "human_focus" => ", lowres, artistic error, film grain, scan artifacts, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, dithering, halftone, screentone, multiple views, logo, too many watermarks, negative space, blank page, @_@, mismatched pupils, glowing eyes, bad anatomy, ",
        "none" => "",
        // 默认 light
        _ => ", lowres, artistic error, scan artifacts, worst quality, bad quality, jpeg artifacts, multiple views, very displeasing, too many watermarks, negative space, blank page, ",
    }
}

/// UC 预设 → 服务端接受的整数。API 只定义 0–3，超过会被拒。
/// furry_focus 与 human_focus 共用 2（服务端没有单独槽位），区别在上面的文本。
fn uc_preset_int(preset: &str) -> u8 {
    match preset {
        "strong" => 0,
        "furry_focus" | "human_focus" => 2,
        "none" => 3,
        _ => 1, // light
    }
}

/// 是否为 V4 系模型（提示词结构不同）。
fn is_v4_model(model: &str) -> bool {
    model.starts_with("nai-diffusion-4")
}

// ========== 请求体 ==========

#[derive(Debug, Serialize)]
struct V4CaptionPayload {
    base_caption: String,
    /// 背景图没有角色，但字段必须在（送空数组，不是省略）。
    char_captions: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct V4ConditionPayload {
    caption: V4CaptionPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    use_coords: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    use_order: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_uc: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ImageParameters {
    params_version: u8,
    width: u32,
    height: u32,
    scale: f32,
    sampler: String,
    steps: u32,
    n_samples: u32,
    seed: u64,
    noise_schedule: String,
    #[serde(rename = "ucPreset")]
    uc_preset: u8,
    #[serde(rename = "qualityToggle")]
    quality_toggle: bool,
    negative_prompt: String,
    /// V4 系模型必须留空（改用 v4_prompt），V3 系才填。
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    v4_prompt: Option<V4ConditionPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    v4_negative_prompt: Option<V4ConditionPayload>,
    cfg_rescale: f32,
    // 以下一组是官方界面的固定默认值。SDK 显式全部送出，这里照做 ——
    // 缺省值在服务端不同模型上并不一致，漏送会出现难查的画风漂移。
    legacy: bool,
    legacy_v3_extend: bool,
    legacy_uc: bool,
    deliberate_euler_ancestral_bug: bool,
    prefer_brownian: bool,
    #[serde(rename = "autoSmea")]
    auto_smea: bool,
    sm: bool,
    sm_dyn: bool,
    dynamic_thresholding: bool,
    use_coords: bool,
    normalize_reference_strength_multiple: bool,
    add_original_image: bool,
    /// 非 i2i 也要送 —— SDK 在没有源图时固定 0.7。
    strength: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_cfg_above_sigma: Option<f32>,
}

#[derive(Debug, Serialize)]
struct GenerateRequest {
    action: String,
    input: String,
    model: String,
    parameters: ImageParameters,
}

// ========== 错误 ==========

#[derive(Debug, thiserror::Error)]
pub enum ImageGenError {
    #[error("未配置 NovelAI Token")]
    MissingToken,
    #[error("{0}")]
    FreeTier(String),
    #[error("NovelAI 认证失败：Token 无效或已过期")]
    Unauthorized,
    #[error("NovelAI 拒绝请求：订阅不足或 Anlas 余额不够")]
    InsufficientCredits,
    #[error("NovelAI 请求参数无效：{0}")]
    InvalidRequest(String),
    #[error("NovelAI 正在处理其他生成任务（并发锁），请稍后再试")]
    Concurrent,
    #[error("NovelAI 限流（429），请稍后再试")]
    RateLimited,
    #[error("NovelAI 服务端错误 {0}：{1}")]
    Server(u16, String),
    #[error("网络错误：{0}")]
    Network(String),
    #[error("解析返回图片失败：{0}")]
    Decode(String),
}

// ========== 生成结果 ==========

pub struct GeneratedImage {
    /// PNG 字节。
    pub bytes: Vec<u8>,
    /// 实际使用的随机种子，写进文件名便于复现。
    pub seed: u64,
    /// 送给服务端的完整正向提示词（含风格前缀与品质标）。
    pub prompt: String,
}

/// 全局生成锁：NovelAI 有并发限制，连续并发请求会回 429。
/// 对话工具与手动生成按钮共用同一把锁。
static GENERATION_LOCK: std::sync::OnceLock<Arc<Mutex<()>>> = std::sync::OnceLock::new();

pub fn generation_lock() -> Arc<Mutex<()>> {
    GENERATION_LOCK
        .get_or_init(|| Arc::new(Mutex::new(())))
        .clone()
}

/// 尝试立刻取得生成锁；已被占用时返回 None（调用方据此回报「正忙」而不是排队等）。
pub fn try_acquire_generation_lock() -> Option<OwnedMutexGuard<()>> {
    generation_lock().try_lock_owned().ok()
}

/// 拼出最终送给服务端的正向提示词：风格前缀 + 场景描述 + 品质标。
pub fn build_prompt(cfg: &ImageGenSettings, scene_tags: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let style = cfg.style_prompt.trim();
    if !style.is_empty() {
        parts.push(style);
    }
    let tags = scene_tags.trim();
    if !tags.is_empty() {
        parts.push(tags);
    }
    let mut prompt = parts.join(", ");
    if cfg.quality_toggle {
        prompt.push_str(QUALITY_TAGS);
    }
    prompt
}

/// 拼出负面提示词：UC 预设文本 + 用户追加。
fn build_negative_prompt(cfg: &ImageGenSettings) -> String {
    let preset = uc_preset_text(&cfg.uc_preset);
    let extra = cfg.negative_prompt.trim();
    if extra.is_empty() {
        preset.to_string()
    } else if preset.is_empty() {
        extra.to_string()
    } else {
        format!("{preset}{extra}")
    }
}

fn build_client(cfg: &ImageGenSettings) -> Result<Client, ImageGenError> {
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(GENERATE_TIMEOUT);

    // 与 web_search 一致：显式代理优先，未开启时回退到环境变量。
    let proxy_url = if cfg.proxy_enabled && !cfg.proxy_addr.trim().is_empty() {
        Some(cfg.proxy_addr.trim().to_string())
    } else if !cfg.proxy_enabled {
        std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .or_else(|_| std::env::var("HTTP_PROXY"))
            .or_else(|_| std::env::var("http_proxy"))
            .ok()
    } else {
        None
    };
    if let Some(url) = proxy_url {
        match reqwest::Proxy::all(&url) {
            Ok(proxy) => builder = builder.proxy(proxy),
            Err(e) => tracing::warn!("NovelAI 代理地址无效，已忽略: {url} ({e})"),
        }
    }

    builder
        .build()
        .map_err(|e| ImageGenError::Network(format!("创建 HTTP 客户端失败: {e}")))
}

/// 组装请求体。抽出来单独可测：payload 形状错了服务端只回一句 validation error，
/// 从错误信息反推不出是哪个字段。
fn build_request(cfg: &ImageGenSettings, prompt: &str, seed: u64) -> GenerateRequest {
    let negative = build_negative_prompt(cfg);
    let v4 = is_v4_model(&cfg.model);

    let (prompt_field, v4_prompt, v4_negative) = if v4 {
        (
            None,
            Some(V4ConditionPayload {
                caption: V4CaptionPayload {
                    base_caption: prompt.to_string(),
                    char_captions: Vec::new(),
                },
                use_coords: Some(false),
                use_order: Some(true),
                legacy_uc: None,
            }),
            Some(V4ConditionPayload {
                caption: V4CaptionPayload {
                    base_caption: negative.clone(),
                    char_captions: Vec::new(),
                },
                use_coords: None,
                use_order: None,
                legacy_uc: Some(false),
            }),
        )
    } else {
        (Some(prompt.to_string()), None, None)
    };

    GenerateRequest {
        action: "generate".to_string(),
        input: prompt.to_string(),
        model: cfg.model.clone(),
        parameters: ImageParameters {
            params_version: 3,
            width: cfg.width,
            height: cfg.height,
            scale: cfg.scale,
            sampler: cfg.sampler.clone(),
            steps: cfg.steps,
            n_samples: 1,
            seed,
            noise_schedule: cfg.noise_schedule.clone(),
            uc_preset: uc_preset_int(&cfg.uc_preset),
            quality_toggle: cfg.quality_toggle,
            negative_prompt: negative,
            prompt: prompt_field,
            v4_prompt,
            v4_negative_prompt: v4_negative,
            cfg_rescale: 0.0,
            legacy: false,
            legacy_v3_extend: false,
            legacy_uc: false,
            deliberate_euler_ancestral_bug: false,
            prefer_brownian: true,
            auto_smea: false,
            sm: false,
            sm_dyn: false,
            dynamic_thresholding: false,
            use_coords: false,
            normalize_reference_strength_multiple: false,
            add_original_image: false,
            strength: 0.7,
            skip_cfg_above_sigma: None,
        },
    }
}

/// 从响应体里取出第一张图。响应正常是 ZIP；解不开时按「本身就是图片」再试一次。
fn extract_first_image(body: &[u8]) -> Result<Vec<u8>, ImageGenError> {
    match zip::ZipArchive::new(Cursor::new(body)) {
        Ok(mut archive) => {
            for i in 0..archive.len() {
                let mut entry = archive
                    .by_index(i)
                    .map_err(|e| ImageGenError::Decode(format!("读取 ZIP 条目失败: {e}")))?;
                let name = entry.name().to_lowercase();
                if name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg")
                    || name.ends_with(".webp")
                {
                    let mut buf = Vec::new();
                    entry
                        .read_to_end(&mut buf)
                        .map_err(|e| ImageGenError::Decode(format!("读取 ZIP 内图片失败: {e}")))?;
                    return Ok(buf);
                }
            }
            Err(ImageGenError::Decode("ZIP 中没有图片文件".to_string()))
        }
        Err(_) => {
            // 不是 ZIP：确认它至少像个 PNG/JPEG，避免把一段错误 JSON 当图片存下来。
            if body.starts_with(&[0x89, b'P', b'N', b'G']) || body.starts_with(&[0xFF, 0xD8]) {
                Ok(body.to_vec())
            } else {
                let preview = String::from_utf8_lossy(&body[..body.len().min(200)]).to_string();
                Err(ImageGenError::Decode(format!(
                    "响应既不是 ZIP 也不是图片: {preview}"
                )))
            }
        }
    }
}

/// 生成一张背景图。调用方需自行持有 [`generation_lock`]。
pub async fn generate_image(
    cfg: &ImageGenSettings,
    scene_tags: &str,
) -> Result<GeneratedImage, ImageGenError> {
    if cfg.api_token.trim().is_empty() {
        return Err(ImageGenError::MissingToken);
    }
    cfg.check_free_tier().map_err(ImageGenError::FreeTier)?;

    let prompt = build_prompt(cfg, scene_tags);
    // 服务端接受 0..2^32-1；自己生成而不是留空，便于复现与写进文件名。
    let seed: u64 = rand::thread_rng().gen_range(0..u32::MAX as u64);
    let request = build_request(cfg, &prompt, seed);

    let client = build_client(cfg)?;
    let url = format!("{}/ai/generate-image", cfg.base_url.trim_end_matches('/'));

    let response = client
        .post(&url)
        .bearer_auth(cfg.api_token.trim())
        .json(&request)
        .send()
        .await
        .map_err(|e| ImageGenError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        let truncated: String = text.chars().take(300).collect();
        return Err(match status {
            StatusCode::UNAUTHORIZED => ImageGenError::Unauthorized,
            StatusCode::PAYMENT_REQUIRED => ImageGenError::InsufficientCredits,
            StatusCode::BAD_REQUEST => ImageGenError::InvalidRequest(truncated),
            StatusCode::CONFLICT => ImageGenError::Concurrent,
            StatusCode::TOO_MANY_REQUESTS => ImageGenError::RateLimited,
            s => ImageGenError::Server(s.as_u16(), truncated),
        });
    }

    let body = response
        .bytes()
        .await
        .map_err(|e| ImageGenError::Network(format!("读取响应失败: {e}")))?;
    let bytes = extract_first_image(&body)?;

    Ok(GeneratedImage {
        bytes,
        seed,
        prompt,
    })
}

/// 账号信息，供设置页「测试连接」显示。
#[derive(Debug, serde::Serialize)]
pub struct SubscriptionInfo {
    /// 订阅等级：3 = Opus（有免费额度），数字越小额度越少。
    pub tier: u8,
    pub active: bool,
    /// 剩余 Anlas（免费额度内的生成不消耗它）。
    pub anlas: i64,
    /// 是否为 Opus —— 免费额度规则只对 Opus 成立。
    pub is_opus: bool,
}

/// 查询订阅状态，用于验证 Token 是否可用。
///
/// 注意端点：账号 API 要打 **image.novelai.net**，打 api.novelai.net 会回 400。
/// 这是 novelai_SDK 用真机验证过的坑（见其 CLAUDE.md 的 `ACCOUNT_API_BASE`）。
pub async fn fetch_subscription(cfg: &ImageGenSettings) -> Result<SubscriptionInfo, ImageGenError> {
    if cfg.api_token.trim().is_empty() {
        return Err(ImageGenError::MissingToken);
    }
    let client = build_client(cfg)?;
    let url = format!("{}/user/subscription", cfg.base_url.trim_end_matches('/'));
    let response = client
        .get(&url)
        .bearer_auth(cfg.api_token.trim())
        .send()
        .await
        .map_err(|e| ImageGenError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        let truncated: String = text.chars().take(300).collect();
        return Err(match status {
            StatusCode::UNAUTHORIZED => ImageGenError::Unauthorized,
            s => ImageGenError::Server(s.as_u16(), truncated),
        });
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ImageGenError::Decode(format!("解析订阅信息失败: {e}")))?;
    let tier = body.get("tier").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    Ok(SubscriptionInfo {
        tier,
        active: body.get("active").and_then(|v| v.as_bool()).unwrap_or(false),
        anlas: body
            .get("trainingStepsLeft")
            .and_then(|v| v.get("fixedTrainingStepsLeft"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        is_opus: tier >= 3,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4_settings() -> ImageGenSettings {
        ImageGenSettings {
            api_token: "test-token".to_string(),
            ..Default::default()
        }
    }

    /// V4 系模型的提示词必须走 v4_prompt，`parameters.prompt` 要整个缺席 ——
    /// 送了会和 v4_prompt 打架。
    #[test]
    fn v4_model_omits_flat_prompt_field() {
        let cfg = v4_settings();
        let req = build_request(&cfg, "seaside, sunset", 1234);
        let json = serde_json::to_value(&req).unwrap();

        assert!(json["parameters"].get("prompt").is_none());
        assert_eq!(
            json["parameters"]["v4_prompt"]["caption"]["base_caption"],
            "seaside, sunset"
        );
        assert_eq!(json["parameters"]["v4_prompt"]["use_order"], true);
        assert_eq!(json["parameters"]["v4_negative_prompt"]["legacy_uc"], false);
        // 顶层 input 与 negative_prompt 照常送。
        assert_eq!(json["input"], "seaside, sunset");
        assert!(json["parameters"]["negative_prompt"].is_string());
    }

    /// V3 系反过来：填平铺的 prompt，不带 v4_* 字段。
    #[test]
    fn v3_model_uses_flat_prompt_field() {
        let cfg = ImageGenSettings {
            model: "nai-diffusion-3".to_string(),
            ..v4_settings()
        };
        let req = build_request(&cfg, "forest path", 99);
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["parameters"]["prompt"], "forest path");
        assert!(json["parameters"].get("v4_prompt").is_none());
        assert!(json["parameters"].get("v4_negative_prompt").is_none());
    }

    /// 默认参数必须落在 Opus 免费额度内 —— 这是「不偷花钱」的底线。
    #[test]
    fn defaults_stay_within_free_tier() {
        let cfg = ImageGenSettings::default();
        assert!(cfg.check_free_tier().is_ok());
        assert!(cfg.width * cfg.height <= crate::ai_service::tools::settings::NAI_FREE_MAX_PIXELS);
        assert!(cfg.steps <= crate::ai_service::tools::settings::NAI_FREE_MAX_STEPS);
    }

    /// 超出免费额度且开着 free_tier_only 时，必须在发请求之前就拒绝。
    #[test]
    fn oversized_request_is_rejected_before_sending() {
        let cfg = ImageGenSettings {
            width: 1920,
            height: 1088,
            ..v4_settings()
        };
        let err = cfg.check_free_tier().unwrap_err();
        assert!(err.contains("超出免费额度"));

        // 关掉开关后放行（用户自己承担费用）。
        let cfg = ImageGenSettings {
            free_tier_only: false,
            ..cfg
        };
        assert!(cfg.check_free_tier().is_ok());
    }

    /// 风格前缀 + 场景标签 + 品质标的拼接顺序。
    #[test]
    fn prompt_composes_style_scene_and_quality() {
        let cfg = v4_settings();
        let prompt = build_prompt(&cfg, "beach, ocean");
        assert!(prompt.starts_with("no humans, scenery, detailed background, beach, ocean"));
        assert!(prompt.ends_with(QUALITY_TAGS));
    }

    /// UC 预设整数不能超过 3（服务端上限），且 none 映射到 3 而不是 4。
    #[test]
    fn uc_preset_ints_stay_within_api_range() {
        for preset in ["strong", "light", "furry_focus", "human_focus", "none"] {
            assert!(uc_preset_int(preset) <= 3, "{preset} 越界");
        }
        assert_eq!(uc_preset_int("none"), 3);
        assert_eq!(uc_preset_int("light"), 1);
    }

    /// 非 ZIP 且非图片的响应必须报错，不能把错误 JSON 当图片存进背景目录。
    #[test]
    fn non_image_response_is_rejected() {
        let err = extract_first_image(br#"{"error":"nope"}"#).unwrap_err();
        assert!(matches!(err, ImageGenError::Decode(_)));
    }

    /// 裸 PNG 响应（非 ZIP）要能直接接受。
    #[test]
    fn bare_png_response_is_accepted() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0u8; 32]);
        let out = extract_first_image(&png).unwrap();
        assert_eq!(out, png);
    }
}
