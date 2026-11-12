//! NovelAI 图片协议与官方前端模型发现；不维护模型编号列表。

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use base64::Engine;
use futures_util::{StreamExt, stream};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use super::providers::{ImageModelCatalog, MAX_RESPONSE_BYTES, limited_body, validate_url};
use crate::utils::proxy::build_proxied_client;

const API_BASE: &str = "https://image.novelai.net";
const FRONTEND: &str = "https://novelai.net/image";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct NovelAiSettings {
    pub api_key: String,
    pub base_url: String,
    pub models_url: String,
    pub frontend_url: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub scale: f64,
    pub sampler: String,
    pub seed: Option<u32>,
    pub negative_prompt: String,
    pub strength: f64,
    pub noise: f64,
    pub transport: String,
    pub parameters_json: String,
}

impl Default for NovelAiSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
            models_url: String::new(),
            frontend_url: String::new(),
            width: 1024,
            height: 1024,
            steps: 28,
            scale: 5.0,
            sampler: "k_euler_ancestral".into(),
            seed: None,
            negative_prompt: String::new(),
            strength: 0.7,
            noise: 0.0,
            transport: "auto".into(),
            parameters_json: "{}".into(),
        }
    }
}

impl NovelAiSettings {
    pub fn normalize(&mut self) {
        self.api_key = self.api_key.trim().into();
        self.base_url = self.base_url.trim().trim_end_matches('/').into();
        self.models_url = self.models_url.trim().into();
        self.frontend_url = self.frontend_url.trim().into();
        self.sampler = self.sampler.trim().into();
        self.transport = self.transport.trim().into();
        if self.parameters_json.trim().is_empty() {
            self.parameters_json = "{}".into();
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.width >= 64
                && self.height >= 64
                && self.width <= 4096
                && self.height <= 4096
                && self.width % 64 == 0
                && self.height % 64 == 0,
            "NovelAI 宽高须为 64 的倍数，范围 64–4096"
        );
        ensure!((1..=100).contains(&self.steps), "NovelAI 步数须为 1–100");
        ensure!(
            self.scale.is_finite() && self.scale > 0.0 && self.scale <= 100.0,
            "NovelAI 提示词引导值无效"
        );
        ensure!(
            self.strength.is_finite()
                && (0.0..=1.0).contains(&self.strength)
                && self.noise.is_finite()
                && (0.0..=1.0).contains(&self.noise),
            "NovelAI 参考图强度或噪声须为 0–1"
        );
        ensure!(
            matches!(self.transport.as_str(), "auto" | "json" | "sse"),
            "NovelAI 传输方式无效"
        );
        let extra: Value =
            serde_json::from_str(&self.parameters_json).context("NovelAI 附加参数不是有效 JSON")?;
        ensure!(extra.is_object(), "NovelAI 附加参数必须为 JSON 对象");
        ensure!(
            extra.get("n_samples").is_none_or(|v| v.as_u64() == Some(1)),
            "当前生图功能每次只生成一张图片"
        );
        Ok(())
    }

    fn base(&self) -> &str {
        if self.base_url.is_empty() {
            API_BASE
        } else {
            &self.base_url
        }
    }
}

#[derive(Clone, Default, Serialize)]
pub struct NovelAiMetadata {
    pub parameters_version: Option<u32>,
    pub streaming_models: Vec<String>,
}

static CATALOGS: LazyLock<Mutex<HashMap<String, (Instant, ImageModelCatalog)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn list_models(settings: &NovelAiSettings, refresh: bool) -> Result<ImageModelCatalog> {
    let page = if settings.frontend_url.is_empty() {
        FRONTEND
    } else {
        &settings.frontend_url
    };
    let source = if settings.models_url.is_empty() {
        page
    } else {
        &settings.models_url
    };
    validate_url(source)?;
    let key = format!(
        "{source}:{}:{:x}",
        settings.base(),
        Sha256::digest(settings.api_key.as_bytes())
    );
    let mut cache = CATALOGS.lock().await;
    if !refresh {
        if let Some((time, catalog)) = cache.get(&key) {
            if time.elapsed() < Duration::from_secs(6 * 60 * 60) {
                return Ok(catalog.clone());
            }
        }
    }
    let fetched = tokio::time::timeout(Duration::from_secs(45), async {
        let http = build_proxied_client(15).await?;
        if settings.models_url.is_empty() {
            discover_frontend(&http, page).await
        } else {
            let mut request = http.get(source);
            if validate_url(source)?.origin() == validate_url(settings.base())?.origin()
                && !settings.api_key.is_empty()
            {
                request = request.bearer_auth(&settings.api_key);
            }
            let response = request.send().await?.error_for_status()?;
            let payload: Value =
                serde_json::from_slice(&limited_body(response, 4 * 1024 * 1024).await?)?;
            let entries = payload
                .get("data")
                .or_else(|| payload.get("models"))
                .and_then(Value::as_array)
                .context("自定义 NovelAI 模型目录缺少 data/models 数组")?;
            let mut models = Vec::new();
            for entry in entries {
                if let Some(id) = entry
                    .as_str()
                    .or_else(|| entry.get("id").and_then(Value::as_str))
                {
                    if !id.trim().is_empty() && !models.iter().any(|s| s == id) {
                        models.push(id.to_string());
                    }
                }
            }
            ensure!(!models.is_empty(), "自定义 NovelAI 目录没有返回模型");
            Ok(ImageModelCatalog {
                models,
                source: source.into(),
                warning: String::new(),
                allow_automatic: true,
                novelai_metadata: Some(NovelAiMetadata {
                    parameters_version: payload
                        .get("parameters_version")
                        .and_then(Value::as_u64)
                        .and_then(|v| v.try_into().ok()),
                    streaming_models: payload
                        .get("streaming_models")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default(),
                }),
            })
        }
    })
    .await
    .context("NovelAI 模型发现超时，可手动填写模型编号")?;
    match fetched {
        Ok(catalog) => {
            cache.insert(key, (Instant::now(), catalog.clone()));
            Ok(catalog)
        },
        Err(error) => {
            if let Some((_, old)) = cache.get(&key) {
                let mut stale = old.clone();
                stale.warning = format!("在线刷新失败，显示上次真实获取的目录：{error}");
                Ok(stale)
            } else {
                Err(error)
            }
        },
    }
}

async fn discover_frontend(http: &reqwest::Client, page: &str) -> Result<ImageModelCatalog> {
    let response = http.get(page).send().await?.error_for_status()?;
    let html = String::from_utf8(limited_body(response, 4 * 1024 * 1024).await?)?;
    let base = validate_url(page)?;
    let script_pattern = Regex::new(r#"<script\b[^>]*\bsrc=["']([^"']+)["']"#)?;
    let mut seen = HashSet::new();
    let mut urls = Vec::new();
    for matched in script_pattern.captures_iter(&html) {
        let Ok(url) = base.join(&matched[1]) else {
            continue;
        };
        if url.origin() == base.origin()
            && url.path().ends_with(".js")
            && seen.insert(url.to_string())
        {
            urls.push(url);
        }
        if urls.len() >= 80 {
            break;
        }
    }
    let mut requests = stream::iter(urls)
        .map(|url| {
            let http = http.clone();
            async move {
                let response = http.get(url).send().await?.error_for_status()?;
                let bytes = limited_body(response, 4 * 1024 * 1024).await?;
                Ok::<_, anyhow::Error>(String::from_utf8(bytes)?)
            }
        })
        .buffer_unordered(6);
    let mut bytes = 0usize;
    while let Some(response) = requests.next().await {
        let Ok(script) = response else { continue };
        bytes += script.len();
        ensure!(bytes <= 24 * 1024 * 1024, "NovelAI 网页模型发现数据量超限");
        if let Ok((models, metadata)) = parse_frontend_models(&script) {
            return Ok(ImageModelCatalog {
                models,
                source: page.into(),
                allow_automatic: true,
                warning: "来自官方当前网页模型定义，实际可用性以账号及生成接口回执为准".into(),
                novelai_metadata: Some(metadata),
            });
        }
    }
    anyhow::bail!(
        "官方网页未返回可解析的图片模型定义；请刷新或手动填写模型编号，没有使用内置模型名单"
    )
}

fn parse_frontend_models(script: &str) -> Result<(Vec<String>, NovelAiMetadata)> {
    let enums = Regex::new(r#"\(\(([A-Za-z_$][\w$]*)=\{\}\)"#)?;
    let entries = Regex::new(r#"\.([\w$]+)\s*=\s*["']([a-zA-Z][a-zA-Z0-9._-]*)["']"#)?;
    let mut models = Vec::new();
    for start in enums.captures_iter(script) {
        let from = start.get(0).unwrap().end();
        let end = script[from..]
            .find(&format!(",{})", &start[1]))
            .map(|n| from + n);
        let Some(end) = end.filter(|end| end - from < 64 * 1024) else {
            continue;
        };
        let block = &script[from..end];
        if !block.contains("nai-diffusion") {
            continue;
        }
        models = entries
            .captures_iter(block)
            .filter(|c| {
                let key = c[1].to_lowercase();
                !key.contains("custom")
                    && !key.contains("test")
                    && !key.contains("unknown")
                    && !c[2].contains("inpainting")
            })
            .map(|c| c[2].to_string())
            .collect();
        if !models.is_empty() {
            break;
        }
    }
    ensure!(!models.is_empty(), "没有图片模型枚举");
    // 网页同时导出别名的双向转换，只排除扩展了 API 编号的显示别名。
    // 反向转换（API 编号 -> 显示别名）不能用于隐藏实际可调用的模型。
    let aliases = Regex::new(r#"return["']([^"']+)["']===\w+\?["']([^"']+)["']:void 0"#)?;
    let hidden: HashSet<_> = aliases
        .captures_iter(script)
        .filter(|c| c[1].starts_with(&format!("{}-", &c[2])) && models.iter().any(|id| id == &c[2]))
        .map(|c| c[1].to_string())
        .collect();
    models.retain(|id| !hidden.contains(id));
    let numbers = Regex::new(r"\d+")?;
    models.sort_by_key(|id| {
        std::cmp::Reverse(
            numbers
                .find_iter(id)
                .filter_map(|m| m.as_str().parse::<u32>().ok())
                .collect::<Vec<_>>(),
        )
    });
    let mut seen = HashSet::new();
    models.retain(|id| seen.insert(id.clone()));
    let versions = Regex::new(r"params_version\s*[:=]\s*(\d+)")?;
    let parameters_version = versions
        .captures_iter(script)
        .filter_map(|m| m[1].parse::<u32>().ok())
        .filter(|v| *v > 0 && *v <= 100)
        .max();
    let cases = Regex::new(r#"((?:case["'][^"']+["']:)+)return\{([^{}]*)\}"#)?;
    let labels = Regex::new(r#"case["']([^"']+)["']:"#)?;
    let streaming = Regex::new(r"streamedResponses\s*:\s*(?:!0|true)")?;
    let mut streaming_models = Vec::new();
    for body in cases.captures_iter(script) {
        if streaming.is_match(&body[2]) {
            streaming_models.extend(
                labels
                    .captures_iter(&body[1])
                    .map(|c| c[1].to_string())
                    .filter(|id| models.contains(id)),
            );
        }
    }
    Ok((
        models,
        NovelAiMetadata {
            parameters_version,
            streaming_models,
        },
    ))
}

fn payload(
    settings: &NovelAiSettings,
    model: &str,
    prompt: &str,
    reference: Option<&[u8]>,
    metadata: Option<&NovelAiMetadata>,
) -> Result<(Value, bool)> {
    settings.validate()?;
    let streaming = match settings.transport.as_str() {
        "sse" => true,
        "json" => false,
        _ => metadata.is_some_and(|m| m.streaming_models.iter().any(|id| id == model)),
    };
    let mut parameters = json!({
        "params_version": metadata.and_then(|m| m.parameters_version).unwrap_or(4),
        "width": settings.width, "height": settings.height, "steps": settings.steps,
        "scale": settings.scale, "sampler": settings.sampler, "seed": settings.seed.unwrap_or_else(rand::random),
        "n_samples": 1, "negative_prompt": settings.negative_prompt,
        "v4_prompt": {"caption":{"base_caption": prompt, "char_captions":[]}, "use_coords":false, "use_order":true},
        "v4_negative_prompt": {"caption":{"base_caption":settings.negative_prompt, "char_captions":[]}, "legacy_uc":false}
    });
    for (key, value) in serde_json::from_str::<Value>(&settings.parameters_json)?
        .as_object()
        .unwrap()
    {
        parameters[key] = value.clone();
    }
    parameters["n_samples"] = json!(1);
    let action = if let Some(image) = reference {
        parameters["image"] = json!(base64::engine::general_purpose::STANDARD.encode(image));
        parameters["strength"] = json!(settings.strength);
        parameters["noise"] = json!(settings.noise);
        "img2img"
    } else {
        "generate"
    };
    if streaming {
        parameters["stream"] = json!("sse");
    }
    Ok((
        json!({"input":prompt, "model":model, "action":action, "parameters":parameters}),
        streaming,
    ))
}

pub async fn generate(
    settings: &NovelAiSettings,
    model: &str,
    prompt: &str,
    reference: Option<&[u8]>,
    timeout: u64,
) -> Result<String> {
    ensure!(
        !settings.api_key.is_empty(),
        "请填写 NovelAI Persistent API Token"
    );
    validate_url(settings.base())?;
    let catalog = list_models(settings, false).await.ok();
    let metadata = catalog.as_ref().and_then(|c| c.novelai_metadata.as_ref());
    let (mut body, streaming) = payload(settings, model, prompt, reference, metadata)?;
    if let Some(reference) = reference {
        let width: u32 = body
            .pointer("/parameters/width")
            .and_then(Value::as_u64)
            .context("NovelAI 宽度无效")?
            .try_into()?;
        let height: u32 = body
            .pointer("/parameters/height")
            .and_then(Value::as_u64)
            .context("NovelAI 高度无效")?
            .try_into()?;
        ensure!(
            width > 0
                && height > 0
                && width <= 16_384
                && height <= 16_384
                && u64::from(width) * u64::from(height) <= 32_000_000,
            "NovelAI 参考图目标尺寸过大"
        );
        let reference = reference.to_vec();
        let image =
            tokio::task::spawn_blocking(move || resize_reference(&reference, width, height))
                .await
                .context("处理 NovelAI 参考图失败")??;
        body["parameters"]["image"] =
            json!(base64::engine::general_purpose::STANDARD.encode(image));
    }
    let route = if streaming {
        "ai/generate-image-stream"
    } else {
        "ai/generate-image"
    };
    let http = build_proxied_client(timeout).await?;
    let response = http
        .post(format!("{}/{route}", settings.base().trim_end_matches('/')))
        .bearer_auth(&settings.api_key)
        .header(
            "accept",
            if streaming {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .json(&body)
        .send()
        .await
        .context("发送 NovelAI 图片请求失败")?;
    let status = response.status();
    let bytes = limited_body(
        response,
        if status.is_success() {
            MAX_RESPONSE_BYTES
        } else {
            64 * 1024
        },
    )
    .await?;
    if !status.is_success() {
        let error: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let detail = error
            .get("message")
            .or_else(|| error.pointer("/error/message"))
            .and_then(Value::as_str)
            .unwrap_or("请检查令牌、模型权限及生成参数");
        anyhow::bail!(
            "NovelAI 返回 HTTP {status}：{}",
            detail
                .replace(&settings.api_key, "[redacted]")
                .chars()
                .take(500)
                .collect::<String>()
        );
    }
    parse_response(&bytes, &settings.api_key)
}

fn resize_reference(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let image = image::load_from_memory(bytes)?.resize_exact(
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png)?;
    ensure!(
        output.get_ref().len() <= MAX_RESPONSE_BYTES,
        "NovelAI 参考图大小超限"
    );
    Ok(output.into_inner())
}

fn image_from_json(value: &Value) -> Option<String> {
    [
        "/images/0/image",
        "/images/0",
        "/image",
        "/data/0/b64_json",
        "/data/image",
        "/data/image_data",
        "/image_data",
    ]
    .iter()
    .find_map(|path| {
        value
            .pointer(path)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    })
}

fn parse_response(bytes: &[u8], secret: &str) -> Result<String> {
    if bytes.starts_with(b"PK\x03\x04") {
        let mut archive =
            zip::ZipArchive::new(Cursor::new(bytes)).context("NovelAI 返回的 ZIP 无效")?;
        ensure!(archive.len() <= 16, "NovelAI ZIP 文件数过多");
        for index in 0..archive.len() {
            let mut file = archive.by_index(index)?;
            let name = file.name()?.to_lowercase();
            if file.is_dir()
                || !["png", "webp", "jpg", "jpeg"]
                    .iter()
                    .any(|ext| name.ends_with(&format!(".{ext}")))
            {
                continue;
            }
            ensure!(
                file.size() <= MAX_RESPONSE_BYTES as u64,
                "NovelAI 图片解压大小超限"
            );
            let mut image = Vec::new();
            (&mut file)
                .take(MAX_RESPONSE_BYTES as u64 + 1)
                .read_to_end(&mut image)?;
            ensure!(
                image.len() <= MAX_RESPONSE_BYTES,
                "NovelAI 图片解压大小超限"
            );
            return Ok(base64::engine::general_purpose::STANDARD.encode(image));
        }
        anyhow::bail!("NovelAI ZIP 中没有图片");
    }
    if let Ok(json) = serde_json::from_slice::<Value>(bytes) {
        return image_from_json(&json).context("NovelAI JSON 响应中没有图片");
    }
    let text = std::str::from_utf8(bytes)
        .context("NovelAI 未返回 JSON、SSE 或 ZIP 图片")?
        .replace("\r\n", "\n");
    let mut result = None;
    let mut final_seen = false;
    for event in text.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let kind = value
            .get("event_type")
            .or_else(|| value.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if kind.contains("error") || event.lines().any(|line| line.trim() == "event: error") {
            let message = value
                .get("message")
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .unwrap_or("生成流报告失败");
            let message = if secret.is_empty() {
                message.to_owned()
            } else {
                message.replace(secret, "[redacted]")
            };
            anyhow::bail!(
                "NovelAI 图片生成失败：{}",
                message.chars().take(500).collect::<String>()
            );
        }
        let final_event = kind.to_lowercase().contains("final")
            || event.lines().any(|line| {
                line.trim().to_lowercase().starts_with("event:")
                    && line.to_lowercase().contains("final")
            });
        if final_event {
            final_seen = true;
            if let Some(image) = image_from_json(&value) {
                result = Some(image);
            }
        }
    }
    ensure!(final_seen, "NovelAI 生成流提前结束，没有最终图片");
    result.context("NovelAI 生成流中没有最终图片")
}
