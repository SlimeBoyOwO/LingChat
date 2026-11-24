//! 图片模型目录的能力筛选与进程内缓存。
//! 按供应商、目录地址和凭据指纹隔离缓存，刷新失败时保留上次成功的目录并提示原因。

use std::collections::HashMap;
use std::future::Future;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use super::http::{limited_body, validate_url};
use super::provider::ImageModelCatalog;

static CATALOGS: LazyLock<Mutex<HashMap<String, (Instant, ImageModelCatalog)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn cache_key(provider: &str, source: &str, secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    let secret_hash = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("{provider}:{source}:{secret_hash}")
}

pub(super) async fn cached_catalog<F, Fut>(
    key: String,
    refresh: bool,
    fetch: F,
) -> Result<ImageModelCatalog>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<ImageModelCatalog>>,
{
    let previous = CATALOGS.lock().await.get(&key).cloned();
    if !refresh {
        if let Some((time, catalog)) = &previous {
            if time.elapsed() < Duration::from_secs(6 * 60 * 60) {
                return Ok(catalog.clone());
            }
        }
    }
    // 网络请求不持有目录锁，避免一个供应商阻塞其他供应商的刷新。
    match fetch().await {
        Ok(catalog) => {
            CATALOGS
                .lock()
                .await
                .insert(key, (Instant::now(), catalog.clone()));
            Ok(catalog)
        },
        Err(error) => match previous {
            Some((_, mut catalog)) => {
                catalog.warning = format!("模型刷新失败，显示上次成功获取的目录：{error}");
                Ok(catalog)
            },
            None => Err(error),
        },
    }
}

pub(super) async fn fetch_api_catalog(
    http: &Client,
    source: &str,
    base: &str,
    secret: &str,
) -> Result<ImageModelCatalog> {
    let mut request = http.get(source).timeout(Duration::from_secs(30));
    if !secret.is_empty() && validate_url(source)?.origin() == validate_url(base)?.origin() {
        request = request.bearer_auth(secret);
    }
    let response = request.send().await.context("请求图片模型目录失败")?;
    ensure!(
        response.status().is_success(),
        "模型目录返回 HTTP {}",
        response.status()
    );
    let bytes = limited_body(response, 4 * 1024 * 1024).await?;
    let payload: Value = serde_json::from_slice(&bytes).context("模型接口未返回有效 JSON")?;
    let entries = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array)
        .context("模型目录缺少 data 或 models 数组")?;
    let mut all = Vec::new();
    let mut images = Vec::new();
    let mut candidates = Vec::new();
    for entry in entries {
        let id = entry.as_str().or_else(|| {
            entry
                .get("id")
                .or_else(|| entry.get("slug"))
                .and_then(Value::as_str)
        });
        let Some(id) = id.map(str::trim).filter(|id| !id.is_empty()) else {
            continue;
        };
        if all.iter().any(|existing| existing == id) {
            continue;
        }
        all.push(id.to_owned());
        let image = entry
            .pointer("/capabilities/image_generation")
            .and_then(Value::as_bool)
            == Some(true)
            || entry.get("type").and_then(Value::as_str) == Some("image")
            || entry
                .get("output_modalities")
                .or_else(|| entry.pointer("/architecture/output_modalities"))
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("image")));
        if image {
            images.push(id.to_owned());
        } else if id.to_lowercase().contains("image") || id.to_lowercase().contains("diffusion") {
            candidates.push(id.to_owned());
        }
    }
    ensure!(!all.is_empty(), "供应商没有返回可选模型");
    let allow_automatic = !images.is_empty();
    Ok(ImageModelCatalog {
        models: if allow_automatic { images } else { candidates },
        source: source.into(),
        warning: if allow_automatic {
            String::new()
        } else {
            "供应商未声明图片输出能力，按名称展示候选；请选择生图模型，获取不到时可手动填写模型编号"
                .into()
        },
        allow_automatic,
        novelai_metadata: None,
    })
}
