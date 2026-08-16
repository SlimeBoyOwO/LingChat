//! TTS 适配器实现集合。
//!
//! 每个后端对应 Python 的一个 `*_adapter.py`：
//! - [`sbv2`] — Style-Bert-Vits2 本地 HTTP (`/voice`)
//! - [`sbv2api`] — SBV2 API (`/synthesize`)
//! - [`bv2`] — Simple-Vits-API Bert-Vits2 (`/voice/bert-vits2`)
//! - [`vits`] — Simple-Vits-API VITS (`/voice/vits`)
//! - [`gsv`] — GPT-SoVITS (`/tts`)
//! - [`aivis`] — AIVIS Cloud API (`/v1/tts/synthesize`)
//! - [`fish_s2`] — Fish Audio S2 / s2.cpp (`/generate`)
//! - [`opentts`] — OpenAI TTS API (`/v1/audio/speech`)
//! - [`indextts`] — IndexTTS2 presets (`/voice/indextts/presets`)

pub mod aivis;
pub mod bv2;
pub mod fish_s2;
pub mod gsv;
pub mod indextts;
pub mod opentts;
pub mod sbv2;
pub mod sbv2api;
pub mod vits;

use once_cell::sync::Lazy;
use reqwest::Client;
use std::time::Duration;

/// 共享 HTTP 客户端（全局连接池 + 统一超时 + 环境变量代理）。
pub(crate) fn http_client() -> &'static Client {
    static CLIENT: Lazy<Client> = Lazy::new(|| {
        // TLS 用 webpki-roots（见 crate::utils::tls::build_tls_config），
        // 绕开 rustls-platform-verifier 在 Android 上的 TLS panic
        let tls_config = crate::utils::tls::build_tls_config().expect("TLS 配置失败");

        let mut builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .tls_backend_preconfigured(tls_config);

        // 兼容 Python openai/httpx 行为：读取 HTTP_PROXY / HTTPS_PROXY 环境变量
        if let Ok(proxy_url) = std::env::var("HTTPS_PROXY").or_else(|_| std::env::var("https_proxy")) {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
            }
        } else if let Ok(proxy_url) = std::env::var("HTTP_PROXY").or_else(|_| std::env::var("http_proxy")) {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
            }
        }

        builder.build().expect("reqwest client 构建失败")
    });
    &CLIENT
}
