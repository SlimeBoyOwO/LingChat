//! 应用配置结构体、默认值和 store 读写逻辑。
//!
//! 设计原则：每个配置项的默认值仅在 `AppConfig::default()` 中定义一次，
//! 其他所有位置（serde、load()、build_config_tree）均引用该实现。
//!
//! 注意：LLM 连接参数（provider/model/api_key/base_url/temperature/top_p/enable_thinking）
//! 和翻译参数已迁移到多供应商系统（`llm.providers`），不再作为全局配置项存在。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Wry};
use tauri_plugin_store::{Store, StoreExt};

use super::keys;
use super::tts::TtsConfig;

// ========== Serde 默认值函数 ==========

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

pub const DEFAULT_LLM_TIMEOUT_SECS: u64 = 120;
pub const MIN_LLM_TIMEOUT_SECS: u64 = 10;
pub const MAX_LLM_TIMEOUT_SECS: u64 = 3600;

fn default_llm_timeout_secs() -> u64 {
    DEFAULT_LLM_TIMEOUT_SECS
}

// ========== AppConfig 结构体 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // ---- LLM 高级选项 ----
    #[serde(default = "default_output_sec_lang")]
    pub llm_output_sec_lang: bool,
    #[serde(default = "default_consumers")]
    pub consumers: u32,
    #[serde(default)]
    pub no_emotion_limit_prompt: bool,
    #[serde(default = "default_llm_timeout_secs")]
    pub llm_timeout_secs: u64,

    // ---- 翻译 ----
    #[serde(default = "default_enable_translate")]
    pub enable_translate: bool,

    // ---- 对话增强 ----
    #[serde(default = "default_enable_time_sense")]
    pub enable_time_sense: bool,
    #[serde(default = "default_enable_emotion_classifier")]
    pub enable_emotion_classifier: bool,

    // ---- 功能开关（记忆系统） ----
    #[serde(default = "default_true")]
    pub use_persistent_memory: bool,
    #[serde(default = "default_memory_update_interval")]
    pub memory_update_interval: u32,
    #[serde(default = "default_memory_recent_window")]
    pub memory_recent_window: u32,

    /// TTS 引擎配置（适配器 URL、音频格式等）
    #[serde(default)]
    pub tts: TtsConfig,
}

// ========== Default 实现（单一真相源） ==========

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm_output_sec_lang: default_output_sec_lang(),
            consumers: default_consumers(),
            no_emotion_limit_prompt: false,
            llm_timeout_secs: default_llm_timeout_secs(),
            enable_translate: default_enable_translate(),
            enable_time_sense: default_enable_time_sense(),
            enable_emotion_classifier: default_enable_emotion_classifier(),
            use_persistent_memory: true,
            memory_update_interval: default_memory_update_interval(),
            memory_recent_window: default_memory_recent_window(),
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

fn get_u64_in_range(store: &Store<Wry>, key: &str, default: u64, min: u64, max: u64) -> u64 {
    store
        .get(key)
        .and_then(|v| v.as_u64())
        .filter(|value| (min..=max).contains(value))
        .unwrap_or(default)
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
            llm_timeout_secs: get_u64_in_range(
                &store,
                keys::LLM_TIMEOUT_SECS,
                default.llm_timeout_secs,
                MIN_LLM_TIMEOUT_SECS,
                MAX_LLM_TIMEOUT_SECS,
            ),
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
            tts: TtsConfig::from_store(Some(&store)),
        })
    }
}
