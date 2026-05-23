//! TTS 引擎配置（适配器 URL、音频格式、语言等），从 settings.json 统一加载。
//!
//! 对标 Python 侧通过 `os.environ` / `.env` 管理的 TTS 相关环境变量。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub mod keys {
    // TTS 适配器后端 URL
    pub const SIMPLE_VITS_API_URL: &str = "tts.simple_vits_api_url";
    pub const BV2_API_URL: &str = "tts.bv2_api_url";
    pub const GSV_API_URL: &str = "tts.gsv_api_url";
    pub const SBV2_API_URL: &str = "tts.sbv2_api_url";
    pub const SBV2API_API_URL: &str = "tts.sbv2api_api_url";
    pub const AIVIS_API_URL: &str = "tts.aivis_api_url";
    pub const AIVIS_API_KEY: &str = "tts.aivis_api_key";
    pub const INDEXTTS_API_URL: &str = "tts.indextts_api_url";

    // 音频参数
    pub const TTS_AUDIO_FORMAT: &str = "tts.audio_format";
    pub const VOICE_LANG: &str = "tts.voice_lang";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// Simple-Vits-API 服务地址（VITS 适配器）
    #[serde(default = "default_simple_vits_url")]
    pub simple_vits_api_url: String,
    /// Simple-Vits-API 服务地址（Bert-Vits2 适配器，可能与 VITS 共用同一服务）
    #[serde(default = "default_bv2_url")]
    pub bv2_api_url: String,
    /// GPT-SoVITS 服务地址
    #[serde(default = "default_gsv_url")]
    pub gsv_api_url: String,
    /// Style-Bert-Vits2 本地服务地址
    #[serde(default = "default_sbv2_url")]
    pub sbv2_api_url: String,
    /// SBV2 API 服务地址
    #[serde(default = "default_sbv2api_url")]
    pub sbv2api_api_url: String,
    /// AIVIS 云 API 地址
    #[serde(default = "default_aivis_url")]
    pub aivis_api_url: String,
    /// AIVIS API 密钥
    #[serde(default)]
    pub aivis_api_key: Option<String>,
    /// IndexTTS2 服务地址
    #[serde(default = "default_indextts_url")]
    pub indextts_api_url: String,

    /// TTS 音频文件格式（wav / mp3 / flac 等）
    #[serde(default = "default_audio_format")]
    pub audio_format: String,
    /// TTS 语音语言（ja / zh / auto）
    #[serde(default = "default_voice_lang")]
    pub voice_lang: String,
}

// ---- 默认值 ----
fn default_simple_vits_url() -> String {
    "http://127.0.0.1:23456".into()
}
fn default_bv2_url() -> String {
    "http://127.0.0.1:6006".into()
}
fn default_gsv_url() -> String {
    "http://127.0.0.1:9880".into()
}
fn default_sbv2_url() -> String {
    "http://127.0.0.1:5000".into()
}
fn default_sbv2api_url() -> String {
    "http://localhost:3000".into()
}
fn default_aivis_url() -> String {
    "https://api.aivis-project.com/v1".into()
}
fn default_indextts_url() -> String {
    "http://127.0.0.1:23467/voice/indextts/presets".into()
}
fn default_audio_format() -> String {
    "wav".into()
}
fn default_voice_lang() -> String {
    "ja".into()
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            simple_vits_api_url: default_simple_vits_url(),
            bv2_api_url: default_bv2_url(),
            gsv_api_url: default_gsv_url(),
            sbv2_api_url: default_sbv2_url(),
            sbv2api_api_url: default_sbv2api_url(),
            aivis_api_url: default_aivis_url(),
            aivis_api_key: None,
            indextts_api_url: default_indextts_url(),
            audio_format: default_audio_format(),
            voice_lang: default_voice_lang(),
        }
    }
}

impl TtsConfig {
    /// 从 `settings.json` 加载 TTS 配置，缺失项使用默认值。
    pub fn load(app: &AppHandle) -> Self {
        let store = app.store(super::STORE_FILE).ok();
        Self::from_store(store.as_deref())
    }

    /// 从已打开的 store 读取 TTS 配置（避免重复打开 store）。
    pub fn from_store(store: Option<&tauri_plugin_store::Store<tauri::Wry>>) -> Self {
        let get_string = |key: &str, default: &str| -> String {
            store
                .and_then(|s| s.get(key))
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| default.to_string())
        };

        Self {
            simple_vits_api_url: get_string(
                keys::SIMPLE_VITS_API_URL,
                &default_simple_vits_url(),
            ),
            bv2_api_url: get_string(keys::BV2_API_URL, &default_bv2_url()),
            gsv_api_url: get_string(keys::GSV_API_URL, &default_gsv_url()),
            sbv2_api_url: get_string(keys::SBV2_API_URL, &default_sbv2_url()),
            sbv2api_api_url: get_string(keys::SBV2API_API_URL, &default_sbv2api_url()),
            aivis_api_url: get_string(keys::AIVIS_API_URL, &default_aivis_url()),
            aivis_api_key: {
                let s = get_string(keys::AIVIS_API_KEY, "");
                if s.is_empty() { None } else { Some(s) }
            },
            indextts_api_url: get_string(keys::INDEXTTS_API_URL, &default_indextts_url()),
            audio_format: get_string(keys::TTS_AUDIO_FORMAT, &default_audio_format()),
            voice_lang: get_string(keys::VOICE_LANG, &default_voice_lang()),
        }
    }
}
