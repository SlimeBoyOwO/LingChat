//! 本地 HTTP 代理自动探测。
//!
//! Codex 后端（chatgpt.com / auth.openai.com）在境内网络必须走代理。
//! 本模块按「环境变量优先 → 常见本地端口探测」的顺序寻找可用代理，
//! 并对候选代理做真实 HTTPS 出网验证；结果缓存 10 分钟，避免每次
//! 请求都重新扫描端口。

use std::net::{SocketAddr, TcpStream};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use reqwest::Client;

use crate::utils::tls::build_tls_config;

/// 常见本地代理端口：10808/10809=v2ray，7890/7897=clash 系，
/// 10810=shadowsocks-windows，1080/8080/8888 通用兜底。
const CANDIDATE_PORTS: &[u16] = &[10808, 10809, 7890, 7897, 10810, 1080, 8080, 8888];

const CACHE_TTL: Duration = Duration::from_secs(600);

/// 探测结果缓存：Some((代理地址或 None, 探测时刻))。
static CACHE: Mutex<Option<(Option<String>, Instant)>> = Mutex::new(None);

fn proxy_from_env() -> Option<String> {
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn tcp_open(port: u16) -> bool {
    let addr: SocketAddr = match format!("127.0.0.1:{port}").parse() {
        Ok(addr) => addr,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// 用候选代理实际请求 chatgpt.com，验证它真的能出网（3 秒超时）。
async fn verify_proxy(url: &str) -> bool {
    let Ok(tls_config) = build_tls_config() else {
        return false;
    };
    let Ok(proxy) = reqwest::Proxy::all(url) else {
        return false;
    };
    let Ok(client) = Client::builder()
        .tls_backend_preconfigured(tls_config)
        .proxy(proxy)
        .timeout(Duration::from_secs(3))
        .build()
    else {
        return false;
    };
    match client.get("https://chatgpt.com/robots.txt").send().await {
        Ok(resp) => resp.status().is_success() || resp.status().is_redirection(),
        Err(_) => false,
    }
}

async fn detect_proxy_uncached() -> Option<String> {
    // 环境变量显式声明的优先信任（用户自己设置的，不再验证）
    if let Some(env_proxy) = proxy_from_env() {
        return Some(env_proxy);
    }
    for port in CANDIDATE_PORTS {
        if !tcp_open(*port) {
            continue;
        }
        let url = format!("http://127.0.0.1:{port}");
        if verify_proxy(&url).await {
            tracing::info!("[Proxy] 自动探测到可用本地代理: {url}");
            return Some(url);
        }
    }
    tracing::warn!("[Proxy] 未探测到可用本地代理，将直连");
    None
}

/// 探测可用代理地址（如 `http://127.0.0.1:10808`）；找不到返回 None（直连）。
/// 结果缓存 10 分钟。
pub async fn detect_proxy() -> Option<String> {
    {
        if let Ok(guard) = CACHE.lock() {
            if let Some((cached, at)) = &*guard {
                if at.elapsed() < CACHE_TTL {
                    return cached.clone();
                }
            }
        }
    }
    let found = detect_proxy_uncached().await;
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((found.clone(), Instant::now()));
    }
    found
}

/// 构建自动走代理的 reqwest Client（Codex 链路专用）。
///
/// 与 factory 的 `build_http_client` 相同使用 webpki-roots TLS，
/// 额外按探测结果注入 `reqwest::Proxy::all`。
pub async fn build_proxied_client(timeout_secs: u64) -> Result<Client> {
    let tls_config = build_tls_config().map_err(anyhow::Error::msg)?;
    let mut builder = Client::builder()
        .read_timeout(Duration::from_secs(timeout_secs))
        .tls_backend_preconfigured(tls_config);
    if let Some(proxy) = detect_proxy().await {
        builder = builder.proxy(reqwest::Proxy::all(&proxy).context("代理地址无效")?);
    }
    builder.build().context("创建带代理的 HTTP 客户端失败")
}
