//! TTS 适配器实现集合。
//!
//! 每个后端对应 Python 的一个 `*_adapter.py`：
//! - [`sbv2`] — Style-Bert-Vits2 本地 HTTP (`/voice`)
//! - [`sbv2api`] — SBV2 API (`/synthesize`)
//! - [`bv2`] — Simple-Vits-API Bert-Vits2 (`/voice/bert-vits2`)
//! - [`vits`] — Simple-Vits-API VITS (`/voice/vits`)
//! - [`gsv`] — GPT-SoVITS (`/tts`)
//! - [`aivis`] — AIVIS Cloud API (`/v1/tts/synthesize`)
//! - [`indextts`] — IndexTTS2 presets (`/voice/indextts/presets`)

pub mod aivis;
pub mod bv2;
pub mod gsv;
pub mod indextts;
pub mod sbv2;
pub mod sbv2api;
pub mod vits;

use once_cell::sync::Lazy;
use reqwest::Client;
use std::time::Duration;

/// 共享 HTTP 客户端（全局连接池 + 统一超时）。
pub(crate) fn http_client() -> &'static Client {
    static CLIENT: Lazy<Client> = Lazy::new(|| {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client 构建失败")
    });
    &CLIENT
}
