//! 应用配置结构体、默认值和 store 读写逻辑。
//!
//! 设计原则：每个配置项的默认值仅在 `AppConfig::default()` 中定义一次，
//! 其他所有位置（serde、load()、build_config_tree）均引用该实现。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Wry};
use tauri_plugin_store::{Store, StoreExt};

use super::keys;
use super::tts::TtsConfig;

// ========== 单一真相源：默认值常量 ==========

/// serde `#[serde(default)]` 在 bool 上默认返回 false，但语义上需要 true。
/// 此函数供 serde 注解使用，确保与 `Default` 实现一致。
fn default_true() -> bool {
    true
}

fn default_output_sec_lang() -> bool {
    true
}
fn default_consumers() -> u32 {
    3
}
fn default_enable_translate() -> bool {
    true
}
fn default_enable_time_sense() -> bool {
    true
}
fn default_enable_emotion_classifier() -> bool {
    true
}
fn default_memory_update_interval() -> u32 {
    250
}
fn default_memory_recent_window() -> u32 {
    30
}

// ========== AppConfig 结构体 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // ---- LLM 连接 ----
    #[serde(default)]
    pub llm_provider: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,
    #[serde(default)]
    pub llm_api_key: Option<String>,
    #[serde(default)]
    pub llm_base_url: Option<String>,

    // ---- LLM 生成参数 ----
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub enable_thinking: bool,

    // ---- LLM 高级选项 ----
    #[serde(default = "default_output_sec_lang")]
    pub llm_output_sec_lang: bool,
    #[serde(default = "default_consumers")]
    pub consumers: u32,
    #[serde(default)]
    pub no_emotion_limit_prompt: bool,

    // ---- 翻译 ----
    #[serde(default)]
    pub translate_provider: Option<String>,
    #[serde(default)]
    pub translate_model: Option<String>,
    #[serde(default)]
    pub translate_api_key: Option<String>,
    #[serde(default)]
    pub translate_base_url: Option<String>,
    #[serde(default = "default_enable_translate")]
    pub enable_translate: bool,

    // ---- 对话增强 ----
    #[serde(default = "default_enable_time_sense")]
    pub enable_time_sense: bool,
    #[serde(default = "default_enable_emotion_classifier")]
    pub enable_emotion_classifier: bool,

    // ---- 功能开关（记忆系统） ----
    /// 修复：使用 `#[serde(default = "default_true")]` 代替裸 `#[serde(default)]`，
    /// 确保 serde 反序列化时也返回 true，与 `Default` 实现和 `load()` 一致。
    #[serde(default = "default_true")]
    pub use_persistent_memory: bool,
    #[serde(default = "default_memory_update_interval")]
    pub memory_update_interval: u32,
    #[serde(default = "default_memory_recent_window")]
    pub memory_recent_window: u32,

    // ---- TTS ----
    #[serde(default)]
    pub auto_start_tts_software: bool,
    #[serde(default)]
    pub tts_software_path: Option<String>,
    #[serde(default)]
    pub voice_check: bool,

    /// TTS 引擎配置（适配器 URL、音频格式等）
    #[serde(default)]
    pub tts: TtsConfig,
}

// ========== Default 实现（单一真相源） ==========

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm_provider: None,
            llm_model: None,
            llm_api_key: None,
            llm_base_url: None,
            temperature: None,
            top_p: None,
            enable_thinking: false,
            llm_output_sec_lang: default_output_sec_lang(),
            consumers: default_consumers(),
            no_emotion_limit_prompt: false,
            translate_provider: None,
            translate_model: None,
            translate_api_key: None,
            translate_base_url: None,
            enable_translate: default_enable_translate(),
            enable_time_sense: default_enable_time_sense(),
            enable_emotion_classifier: default_enable_emotion_classifier(),
            use_persistent_memory: default_true(),
            memory_update_interval: default_memory_update_interval(),
            memory_recent_window: default_memory_recent_window(),
            auto_start_tts_software: false,
            tts_software_path: None,
            voice_check: false,
            tts: TtsConfig::default(),
        }
    }
}

// ========== Store 读写辅助函数 ==========

fn get_string(store: &Store<Wry>, key: &str) -> Option<String> {
    store
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// 从 settings store 读取字符串值（公开接口，供外部模块使用）。
pub fn get_setting_string(app: &AppHandle, key: &str) -> Option<String> {
    super::settings_store(app)
        .ok()
        .and_then(|store| get_string(&store, key))
}

fn get_bool(store: &Store<Wry>, key: &str, default: bool) -> bool {
    store.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn get_u32(store: &Store<Wry>, key: &str, default: u32) -> u32 {
    store
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(default)
}

fn get_f64(store: &Store<Wry>, key: &str) -> Option<f64> {
    store.get(key).and_then(|v| v.as_f64())
}

// ========== AppConfig 方法 ==========

impl AppConfig {
    /// 从 settings.json 加载配置，缺失项回退到 `Self::default()`。
    pub fn load(app: &AppHandle) -> Result<Self> {
        let store = app
            .store(super::STORE_FILE)
            .context("Failed to open settings store")?;

        let default = Self::default();

        Ok(Self {
            llm_provider: get_string(&store, keys::LLM_PROVIDER),
            llm_model: get_string(&store, keys::LLM_MODEL),
            llm_api_key: get_string(&store, keys::LLM_API_KEY),
            llm_base_url: get_string(&store, keys::LLM_BASE_URL),
            temperature: get_f64(&store, keys::LLM_TEMPERATURE),
            top_p: get_f64(&store, keys::LLM_TOP_P),
            enable_thinking: get_bool(&store, keys::LLM_ENABLE_THINKING, default.enable_thinking),
            llm_output_sec_lang: get_bool(
                &store,
                keys::LLM_OUTPUT_SEC_LANG,
                default.llm_output_sec_lang,
            ),
            consumers: get_u32(&store, keys::CONSUMERS, default.consumers),
            no_emotion_limit_prompt: get_bool(
                &store,
                keys::LLM_NO_EMOTION_LIMIT,
                default.no_emotion_limit_prompt,
            ),
            translate_provider: get_string(&store, keys::TRANSLATE_PROVIDER),
            translate_model: get_string(&store, keys::TRANSLATE_MODEL),
            translate_api_key: get_string(&store, keys::TRANSLATE_API_KEY),
            translate_base_url: get_string(&store, keys::TRANSLATE_BASE_URL),
            enable_translate: get_bool(
                &store,
                keys::TRANSLATE_ENABLE,
                default.enable_translate,
            ),
            enable_time_sense: get_bool(
                &store,
                keys::ENABLE_TIME_SENSE,
                default.enable_time_sense,
            ),
            enable_emotion_classifier: get_bool(
                &store,
                keys::ENABLE_EMOTION_CLASSIFIER,
                default.enable_emotion_classifier,
            ),
            use_persistent_memory: get_bool(
                &store,
                keys::USE_PERSISTENT_MEMORY,
                default.use_persistent_memory,
            ),
            memory_update_interval: get_u32(
                &store,
                keys::MEMORY_UPDATE_INTERVAL,
                default.memory_update_interval,
            ),
            memory_recent_window: get_u32(
                &store,
                keys::MEMORY_RECENT_WINDOW,
                default.memory_recent_window,
            ),
            auto_start_tts_software: get_bool(
                &store,
                keys::AUTO_START_TTS_SOFTWARE,
                default.auto_start_tts_software,
            ),
            tts_software_path: get_string(&store, keys::TTS_SOFTWARE_PATH),
            voice_check: get_bool(&store, keys::VOICE_CHECK, default.voice_check),
            tts: TtsConfig::from_store(Some(&store)),
        })
    }
}
