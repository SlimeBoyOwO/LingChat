//! 应用配置结构体、默认值和 store 读写逻辑。
//!
//! 设计原则：每个配置项的默认值仅在 `AppConfig::default()` 中定义一次，
//! 其他所有位置（serde、load()、build_config_tree）均引用该实现。
//!
//! 注意：LLM 连接参数（provider/model/api_key/base_url/temperature/top_p/enable_thinking）
//! 和翻译参数已迁移到多供应商系统（`llm.providers`），不再作为全局配置项存在。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
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
fn default_memory_top_k() -> u32 {
    20
}
fn default_memory_min_score() -> f64 {
    0.4
}
fn default_disable_splash_animation() -> bool {
    false
}
fn default_auto_save_enabled() -> bool {
    true
}
fn default_auto_save_interval_secs() -> u32 {
    300
}
fn default_auto_compress_image() -> bool {
    true
}

pub const DEFAULT_LLM_TIMEOUT_SECS: u64 = 120;
pub const MIN_LLM_TIMEOUT_SECS: u64 = 10;
pub const MAX_LLM_TIMEOUT_SECS: u64 = 3600;
pub const MIN_AUTO_SAVE_INTERVAL_SECS: u32 = 30;
pub const MAX_AUTO_SAVE_INTERVAL_SECS: u32 = 3600;
pub const MIN_MEMORY_TOP_K: u32 = 1;
pub const MAX_MEMORY_TOP_K: u32 = 200;
pub const MAX_MEMORY_MIN_SCORE: f64 = 1.0;

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
    /// 图片超过端点大小限制时是否自动压缩后再发送（作用于所有携带图片的 LLM 请求）。
    #[serde(default = "default_auto_compress_image")]
    pub auto_compress_image: bool,

    // ---- 翻译 ----
    #[serde(default = "default_enable_translate")]
    pub enable_translate: bool,

    // ---- 对话增强 ----
    #[serde(default = "default_enable_time_sense")]
    pub enable_time_sense: bool,
    #[serde(default = "default_enable_emotion_classifier")]
    pub enable_emotion_classifier: bool,
    /// 天气工具的手动城市（最高优先级；空 = 未配置）
    #[serde(default)]
    pub weather_city: String,
    /// 今日首次对话时是否注入天气提醒（剧本模式/编辑器试玩从不注入）。
    /// 默认关闭：平时天气只由模型按需调用 query_weather 感知，不做任何主动注入。
    #[serde(default)]
    pub weather_first_talk: bool,
    // ---- AI 自动切换（手动优先锁；用户手动设置后翻 false，设置页可改回） ----
    #[serde(default = "default_true")]
    pub ai_auto_clothes: bool,
    #[serde(default = "default_true")]
    pub ai_auto_scene: bool,
    #[serde(default = "default_true")]
    pub ai_auto_effect: bool,

    // ---- 功能开关（记忆系统） ----
    /// 记忆工具（memory_store / memory_delete / memory_summarize）总开关。
    #[serde(default = "default_true")]
    pub memory_enabled: bool,
    /// 记忆窗口内没有 user 消息时，是否在裁切后的首条 assistant 前注入一条 user「继续」。
    /// 默认开启：genai 不做消息规范化，Gemini 等 provider 要求首条为 user。
    #[serde(default = "default_true")]
    pub memory_inject_continue_user: bool,

    // ---- 记忆（向量嵌入） ----
    /// OpenAI 兼容 embeddings 接口地址（留空 = 未配置）。
    #[serde(default)]
    pub embedding_base_url: String,
    #[serde(default)]
    pub embedding_api_key: String,
    #[serde(default)]
    pub embedding_model: String,

    // ---- 记忆检索参数 ----
    #[serde(default = "default_memory_top_k")]
    pub memory_top_k: u32,
    #[serde(default = "default_memory_min_score")]
    pub memory_min_score: f64,

    // ---- 界面与显示 ----
    /// 是否关闭首次启动的开屏动画（LoadingTransition）。
    #[serde(default = "default_disable_splash_animation")]
    pub disable_splash_animation: bool,

    // ---- 自动存档 ----
    #[serde(default = "default_auto_save_enabled")]
    pub auto_save_enabled: bool,
    #[serde(default = "default_auto_save_interval_secs")]
    pub auto_save_interval_secs: u32,

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
            auto_compress_image: default_auto_compress_image(),
            enable_translate: default_enable_translate(),
            enable_time_sense: default_enable_time_sense(),
            enable_emotion_classifier: default_enable_emotion_classifier(),
            memory_enabled: true,
            weather_city: String::new(),
            weather_first_talk: false,
            ai_auto_clothes: true,
            ai_auto_scene: true,
            ai_auto_effect: true,
            memory_inject_continue_user: true,
            embedding_base_url: String::new(),
            embedding_api_key: String::new(),
            embedding_model: String::new(),
            memory_top_k: default_memory_top_k(),
            memory_min_score: default_memory_min_score(),
            disable_splash_animation: default_disable_splash_animation(),
            auto_save_enabled: default_auto_save_enabled(),
            auto_save_interval_secs: default_auto_save_interval_secs(),
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

fn get_u32_in_range(store: &Store<Wry>, key: &str, default: u32, min: u32, max: u32) -> u32 {
    store
        .get(key)
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| (min..=max).contains(value))
        .unwrap_or(default)
}

/// 读取 f64 设置；缺失或越界时回退默认值（同时兼容字符串形式的数字）。
fn get_f64_in_range(store: &Store<Wry>, key: &str, default: f64, min: f64, max: f64) -> f64 {
    store
        .get(key)
        .and_then(|v| match v {
            JsonValue::Number(n) => n.as_f64(),
            JsonValue::String(s) => s.parse::<f64>().ok(),
            _ => None,
        })
        .filter(|value| (min..=max).contains(value))
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
            auto_compress_image: get_bool(
                &store,
                keys::LLM_AUTO_COMPRESS_IMAGE,
                default.auto_compress_image,
            ),
            enable_translate: get_bool(&store, keys::TRANSLATE_ENABLE, default.enable_translate),
            enable_time_sense: get_bool(&store, keys::ENABLE_TIME_SENSE, default.enable_time_sense),
            enable_emotion_classifier: get_bool(
                &store,
                keys::ENABLE_EMOTION_CLASSIFIER,
                default.enable_emotion_classifier,
            ),
            memory_enabled: get_bool(&store, keys::MEMORY_ENABLED, default.memory_enabled),
            weather_city: get_string(&store, keys::WEATHER_CITY).unwrap_or_default(),
            weather_first_talk: get_bool(
                &store,
                keys::WEATHER_FIRST_TALK,
                default.weather_first_talk,
            ),
            ai_auto_clothes: get_bool(&store, keys::AI_AUTO_CLOTHES, default.ai_auto_clothes),
            ai_auto_scene: get_bool(&store, keys::AI_AUTO_SCENE, default.ai_auto_scene),
            ai_auto_effect: get_bool(&store, keys::AI_AUTO_EFFECT, default.ai_auto_effect),
            memory_inject_continue_user: get_bool(
                &store,
                keys::MEMORY_INJECT_CONTINUE_USER,
                default.memory_inject_continue_user,
            ),
            embedding_base_url: get_string(&store, keys::EMBEDDING_BASE_URL)
                .unwrap_or_else(|| default.embedding_base_url.clone()),
            embedding_api_key: get_string(&store, keys::EMBEDDING_API_KEY)
                .unwrap_or_else(|| default.embedding_api_key.clone()),
            embedding_model: get_string(&store, keys::EMBEDDING_MODEL)
                .unwrap_or_else(|| default.embedding_model.clone()),
            memory_top_k: get_u32_in_range(
                &store,
                keys::MEMORY_TOP_K,
                default.memory_top_k,
                MIN_MEMORY_TOP_K,
                MAX_MEMORY_TOP_K,
            ),
            memory_min_score: get_f64_in_range(
                &store,
                keys::MEMORY_MIN_SCORE,
                default.memory_min_score,
                0.0,
                MAX_MEMORY_MIN_SCORE,
            ),
            disable_splash_animation: get_bool(
                &store,
                keys::DISABLE_SPLASH_ANIMATION,
                default.disable_splash_animation,
            ),
            auto_save_enabled: get_bool(&store, keys::AUTO_SAVE_ENABLED, default.auto_save_enabled),
            auto_save_interval_secs: get_u32_in_range(
                &store,
                keys::AUTO_SAVE_INTERVAL_SECS,
                default.auto_save_interval_secs,
                MIN_AUTO_SAVE_INTERVAL_SECS,
                MAX_AUTO_SAVE_INTERVAL_SECS,
            ),
            tts: TtsConfig::from_store(Some(&store)),
        })
    }

    /// 手动优先落锁：用户手动设置了服装/场景/特效时，关闭对应的 AI 自动切换
    /// 开关（写入 settings.json，设置页可见、可重新打开）。
    pub fn lock_auto_switch(app: &AppHandle, key: &str) {
        if let Ok(store) = app.store(super::STORE_FILE) {
            store.set(key.to_string(), serde_json::Value::Bool(false));
            let _ = store.save();
        }
    }
}
