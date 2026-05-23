use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::{AppHandle, Wry};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_store::{Store, StoreExt};

pub const STORE_FILE: &str = "settings.json";

pub mod proactive;
pub mod tts;

use crate::config::tts::TtsConfig;

// ========== 字段键（对标 Python .env） ==========
pub mod keys {
    // LLM 连接（对应 LLM_PROVIDER / MODEL_TYPE / CHAT_API_KEY / CHAT_BASE_URL）
    pub const LLM_PROVIDER: &str = "llm.provider";
    pub const LLM_MODEL: &str = "llm.model";
    pub const LLM_API_KEY: &str = "llm.api_key";
    pub const LLM_BASE_URL: &str = "llm.base_url";

    // LLM 生成参数（对应 TEMPERATURE / TOP_P / ENABLE_THINKING）
    pub const LLM_TEMPERATURE: &str = "llm.temperature";
    pub const LLM_TOP_P: &str = "llm.top_p";
    pub const LLM_ENABLE_THINKING: &str = "llm.enable_thinking";

    // LLM 高级选项
    pub const LLM_OUTPUT_SEC_LANG: &str = "llm.output_sec_lang";
    pub const CONSUMERS: &str = "llm.consumers";
    pub const LLM_NO_EMOTION_LIMIT: &str = "llm.no_emotion_limit_prompt";

    // 翻译（对应 TRANSLATE_LLM_PROVIDER / TRANSLATE_MODEL / TRANSLATE_API_KEY / TRANSLATE_BASE_URL）
    pub const TRANSLATE_PROVIDER: &str = "translate.provider";
    pub const TRANSLATE_MODEL: &str = "translate.model";
    pub const TRANSLATE_API_KEY: &str = "translate.api_key";
    pub const TRANSLATE_BASE_URL: &str = "translate.base_url";
    pub const TRANSLATE_ENABLE: &str = "translate.enable";

    // 对话增强
    pub const ENABLE_TIME_SENSE: &str = "features.enable_time_sense";
    pub const ENABLE_EMOTION_CLASSIFIER: &str = "features.enable_emotion_classifier";

    // 功能开关
    pub const USE_RAG: &str = "features.use_rag";
    pub const USE_PERSISTENT_MEMORY: &str = "features.use_persistent_memory";
    pub const MEMORY_UPDATE_INTERVAL: &str = "features.memory_update_interval";
    pub const MEMORY_RECENT_WINDOW: &str = "features.memory_recent_window";

    // TTS
    pub const AUTO_START_TTS_SOFTWARE: &str = "tts.auto_start";
    pub const TTS_SOFTWARE_PATH: &str = "tts.software_path";
    pub const VOICE_CHECK: &str = "tts.voice_check";

    // 其他
    pub const COMMUNITY_URL: &str = "community.url";
    /// 上次游玩的角色 ID（启动时自动恢复）
    pub const LAST_CHARACTER_ID: &str = "game.last_character_id";
}

// ========== 类型化配置 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // LLM 连接
    #[serde(default)]
    pub llm_provider: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,
    #[serde(default)]
    pub llm_api_key: Option<String>,
    #[serde(default)]
    pub llm_base_url: Option<String>,

    // LLM 生成参数（对应 Python TEMPERATURE / TOP_P / ENABLE_THINKING）
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub enable_thinking: bool,

    // LLM 高级选项
    #[serde(default = "default_output_sec_lang")]
    pub llm_output_sec_lang: bool,
    #[serde(default = "default_consumers")]
    pub consumers: u32,
    #[serde(default)]
    pub no_emotion_limit_prompt: bool,

    // 翻译
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

    // 对话增强
    #[serde(default = "default_enable_time_sense")]
    pub enable_time_sense: bool,
    #[serde(default = "default_enable_emotion_classifier")]
    pub enable_emotion_classifier: bool,

    // 功能开关
    #[serde(default)]
    pub use_rag: bool,
    #[serde(default)]
    pub use_persistent_memory: bool,
    #[serde(default = "default_memory_update_interval")]
    pub memory_update_interval: u32,
    #[serde(default = "default_memory_recent_window")]
    pub memory_recent_window: u32,

    // TTS
    #[serde(default)]
    pub auto_start_tts_software: bool,
    #[serde(default)]
    pub tts_software_path: Option<String>,
    #[serde(default)]
    pub voice_check: bool,

    // 其他
    #[serde(default)]
    pub community_url: Option<String>,

    /// TTS 引擎配置（适配器 URL、音频格式等）
    #[serde(default)]
    pub tts: TtsConfig,
}

fn default_output_sec_lang() -> bool { true }
fn default_consumers() -> u32 { 3 }
fn default_enable_translate() -> bool { true }
fn default_enable_time_sense() -> bool { true }
fn default_enable_emotion_classifier() -> bool { true }
fn default_memory_update_interval() -> u32 { 50 }
fn default_memory_recent_window() -> u32 { 15 }

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
            llm_output_sec_lang: true,
            consumers: 3,
            no_emotion_limit_prompt: false,
            translate_provider: None,
            translate_model: None,
            translate_api_key: None,
            translate_base_url: None,
            enable_translate: true,
            enable_time_sense: true,
            enable_emotion_classifier: true,
            use_rag: false,
            use_persistent_memory: false,
            memory_update_interval: 50,
            memory_recent_window: 15,
            auto_start_tts_software: false,
            tts_software_path: None,
            voice_check: false,
            community_url: None,
            tts: TtsConfig::default(),
        }
    }
}

fn get_string(store: &Store<Wry>, key: &str) -> Option<String> {
    store.get(key).and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn get_bool(store: &Store<Wry>, key: &str, default: bool) -> bool {
    store
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
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

impl AppConfig {
    pub fn load(app: &AppHandle) -> Result<Self> {
        let store = app
            .store(STORE_FILE)
            .context("Failed to open settings store")?;

        Ok(Self {
            llm_provider: get_string(&store, keys::LLM_PROVIDER),
            llm_model: get_string(&store, keys::LLM_MODEL),
            llm_api_key: get_string(&store, keys::LLM_API_KEY),
            llm_base_url: get_string(&store, keys::LLM_BASE_URL),
            temperature: get_f64(&store, keys::LLM_TEMPERATURE),
            top_p: get_f64(&store, keys::LLM_TOP_P),
            enable_thinking: get_bool(&store, keys::LLM_ENABLE_THINKING, false),
            llm_output_sec_lang: get_bool(&store, keys::LLM_OUTPUT_SEC_LANG, true),
            consumers: get_u32(&store, keys::CONSUMERS, 3),
            no_emotion_limit_prompt: get_bool(&store, keys::LLM_NO_EMOTION_LIMIT, false),
            translate_provider: get_string(&store, keys::TRANSLATE_PROVIDER),
            translate_model: get_string(&store, keys::TRANSLATE_MODEL),
            translate_api_key: get_string(&store, keys::TRANSLATE_API_KEY),
            translate_base_url: get_string(&store, keys::TRANSLATE_BASE_URL),
            enable_translate: get_bool(&store, keys::TRANSLATE_ENABLE, true),
            enable_time_sense: get_bool(&store, keys::ENABLE_TIME_SENSE, true),
            enable_emotion_classifier: get_bool(&store, keys::ENABLE_EMOTION_CLASSIFIER, true),
            use_rag: get_bool(&store, keys::USE_RAG, false),
            use_persistent_memory: get_bool(&store, keys::USE_PERSISTENT_MEMORY, false),
            memory_update_interval: get_u32(&store, keys::MEMORY_UPDATE_INTERVAL, 50),
            memory_recent_window: get_u32(&store, keys::MEMORY_RECENT_WINDOW, 15),
            auto_start_tts_software: get_bool(&store, keys::AUTO_START_TTS_SOFTWARE, false),
            tts_software_path: get_string(&store, keys::TTS_SOFTWARE_PATH),
            voice_check: get_bool(&store, keys::VOICE_CHECK, false),
            community_url: get_string(&store, keys::COMMUNITY_URL),
            tts: TtsConfig::from_store(Some(&store)),
        })
    }
}

pub fn settings_store(app: &AppHandle) -> Result<Arc<Store<Wry>>> {
    app.store(STORE_FILE).context("Failed to open settings store")
}

// ========== 结构化配置树（前端"高级设置"页面使用） ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSetting {
    pub key: String,
    pub value: String,
    pub description: String,
    #[serde(rename = "type")]
    pub setting_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subcategory {
    pub description: String,
    pub settings: Vec<ConfigSetting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub subcategories: BTreeMap<String, Subcategory>,
}

pub type ConfigTree = BTreeMap<String, Category>;

fn read_setting(app: &AppHandle, key: &str, default: &str) -> String {
    settings_store(app)
        .ok()
        .and_then(|store| {
            store.get(key).map(|v| match v {
                JsonValue::String(s) => s.clone(),
                JsonValue::Bool(b) => b.to_string(),
                JsonValue::Number(n) => n.to_string(),
                _ => default.to_string(),
            })
        })
        .unwrap_or_else(|| default.to_string())
}

/// 构建前端"高级设置"页面所需的完整配置树。
/// 分类对标 Python .env 的逻辑分组。
pub fn build_config_tree(app: &AppHandle) -> ConfigTree {
    let mut tree = BTreeMap::new();

    // ===== LLM 配置 =====
    {
        let mut llm_subs = BTreeMap::new();

        // 连接设置（对应 LLM_PROVIDER / MODEL_TYPE / CHAT_API_KEY / CHAT_BASE_URL）
        llm_subs.insert(
            "连接设置".to_string(),
            Subcategory {
                description: "对应 ENV: LLM_PROVIDER / MODEL_TYPE / CHAT_API_KEY / CHAT_BASE_URL".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::LLM_PROVIDER.to_string(),
                        value: read_setting(app, keys::LLM_PROVIDER, ""),
                        description: "LLM 服务提供商（openai / deepseek / gemini / ollama）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_MODEL.to_string(),
                        value: read_setting(app, keys::LLM_MODEL, ""),
                        description: "模型名称（如 deepseek-chat / gpt-4o）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_API_KEY.to_string(),
                        value: read_setting(app, keys::LLM_API_KEY, ""),
                        description: "API 密钥".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_BASE_URL.to_string(),
                        value: read_setting(app, keys::LLM_BASE_URL, ""),
                        description: "API 基础 URL（留空使用默认，如 DeepSeek 官方 https://api.deepseek.com/v1）".to_string(),
                        setting_type: "text".to_string(),
                    },
                ],
            },
        );

        // 生成参数（对应 TEMPERATURE / TOP_P / ENABLE_THINKING）
        llm_subs.insert(
            "生成参数".to_string(),
            Subcategory {
                description: "对应 ENV: TEMPERATURE / TOP_P / ENABLE_THINKING".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::LLM_TEMPERATURE.to_string(),
                        value: read_setting(app, keys::LLM_TEMPERATURE, ""),
                        description: "生成温度 TEMPERATURE（越高越随机，留空使用默认 1.3）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_TOP_P.to_string(),
                        value: read_setting(app, keys::LLM_TOP_P, ""),
                        description: "核采样 TOP_P（留空使用默认 0.9）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_ENABLE_THINKING.to_string(),
                        value: read_setting(app, keys::LLM_ENABLE_THINKING, "false"),
                        description: "ENABLE_THINKING — 启用思考链（部分模型支持）".to_string(),
                        setting_type: "bool".to_string(),
                    },
                ],
            },
        );

        // 高级选项
        llm_subs.insert(
            "高级选项".to_string(),
            Subcategory {
                description: "调优 AI 对话行为的高级参数".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::LLM_OUTPUT_SEC_LANG.to_string(),
                        value: read_setting(app, keys::LLM_OUTPUT_SEC_LANG, "true"),
                        description: "LLM_OUTPUT_SEC_LANG — 是否允许输出第二语言（关闭后仅输出中文）".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::CONSUMERS.to_string(),
                        value: read_setting(app, keys::CONSUMERS, "3"),
                        description: "COMSUMERS — 并发消费者数量（增大可加速流式输出，默认 3）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_NO_EMOTION_LIMIT.to_string(),
                        value: read_setting(app, keys::LLM_NO_EMOTION_LIMIT, "false"),
                        description: "NO_EMOTION_LIMIT_PROMPT — 解除 emotion 数量限制（可能增加 token 消耗）".to_string(),
                        setting_type: "bool".to_string(),
                    },
                ],
            },
        );

        tree.insert("LLM 配置".to_string(), Category { subcategories: llm_subs });
    }

    // ===== 翻译配置 =====
    {
        let mut trans_subs = BTreeMap::new();

        trans_subs.insert(
            "连接设置".to_string(),
            Subcategory {
                description: "对应 ENV: TRANSLATE_LLM_PROVIDER / TRANSLATE_MODEL / TRANSLATE_API_KEY / TRANSLATE_BASE_URL".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::TRANSLATE_PROVIDER.to_string(),
                        value: read_setting(app, keys::TRANSLATE_PROVIDER, ""),
                        description: "翻译服务提供商（留空则复用主 LLM 配置）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::TRANSLATE_MODEL.to_string(),
                        value: read_setting(app, keys::TRANSLATE_MODEL, ""),
                        description: "翻译模型名称（如 qwen-mt-plus）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::TRANSLATE_API_KEY.to_string(),
                        value: read_setting(app, keys::TRANSLATE_API_KEY, ""),
                        description: "翻译 API 密钥".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::TRANSLATE_BASE_URL.to_string(),
                        value: read_setting(app, keys::TRANSLATE_BASE_URL, ""),
                        description: "翻译 API 基础 URL".to_string(),
                        setting_type: "text".to_string(),
                    },
                ],
            },
        );

        trans_subs.insert(
            "功能选项".to_string(),
            Subcategory {
                description: "翻译功能的开关与行为控制".to_string(),
                settings: vec![ConfigSetting {
                    key: keys::TRANSLATE_ENABLE.to_string(),
                    value: read_setting(app, keys::TRANSLATE_ENABLE, "true"),
                    description: "ENABLE_TRANSLATE — 启用 AI 翻译（将中文对话翻译为第二语言）".to_string(),
                    setting_type: "bool".to_string(),
                }],
            },
        );

        tree.insert("翻译配置".to_string(), Category { subcategories: trans_subs });
    }

    // ===== 功能设置 =====
    {
        let mut feat_subs = BTreeMap::new();

        // 对话增强
        feat_subs.insert(
            "对话增强".to_string(),
            Subcategory {
                description: "对应 ENV: USE_TIME_SENSE / ENABLE_EMOTION_CLASSIFIER".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::ENABLE_TIME_SENSE.to_string(),
                        value: read_setting(app, keys::ENABLE_TIME_SENSE, "true"),
                        description: "USE_TIME_SENSE — 启用时间感知（根据上下文时间添加系统提醒）".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::ENABLE_EMOTION_CLASSIFIER.to_string(),
                        value: read_setting(app, keys::ENABLE_EMOTION_CLASSIFIER, "true"),
                        description: "ENABLE_EMOTION_CLASSIFIER — 启用情感分类器（ONNX 模型，用于自动标注对话 emotion）".to_string(),
                        setting_type: "bool".to_string(),
                    },
                ],
            },
        );

        // 记忆系统
        feat_subs.insert(
            "记忆系统".to_string(),
            Subcategory {
                description: "对应 ENV: MEMORY_UPDATE_INTERVAL / MEMORY_RECENT_WINDOW / USE_PERSISTENT_MEMORY".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::USE_PERSISTENT_MEMORY.to_string(),
                        value: read_setting(app, keys::USE_PERSISTENT_MEMORY, "false"),
                        description: "USE_PERSISTENT_MEMORY — 开启后记忆会自动压缩，减少 token 消耗".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::MEMORY_UPDATE_INTERVAL.to_string(),
                        value: read_setting(app, keys::MEMORY_UPDATE_INTERVAL, "50"),
                        description: "MEMORY_UPDATE_INTERVAL — 触发记忆摘要的新消息数（默认 50）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::MEMORY_RECENT_WINDOW.to_string(),
                        value: read_setting(app, keys::MEMORY_RECENT_WINDOW, "15"),
                        description: "MEMORY_RECENT_WINDOW — 摘要时保留的最近消息数（默认 15）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::USE_RAG.to_string(),
                        value: read_setting(app, keys::USE_RAG, "false"),
                        description: "USE_RAG — 启用 RAG 检索增强生成（需要额外配置向量数据库）".to_string(),
                        setting_type: "bool".to_string(),
                    },
                ],
            },
        );

        tree.insert("功能设置".to_string(), Category { subcategories: feat_subs });
    }

    // ===== TTS 配置 =====
    {
        let mut tts_subs = BTreeMap::new();

        tts_subs.insert(
            "基础设置".to_string(),
            Subcategory {
                description: "文字转语音（TTS）的相关设置".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::AUTO_START_TTS_SOFTWARE.to_string(),
                        value: read_setting(app, keys::AUTO_START_TTS_SOFTWARE, "false"),
                        description: "启动游戏时自动启动 TTS 软件".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::TTS_SOFTWARE_PATH.to_string(),
                        value: read_setting(app, keys::TTS_SOFTWARE_PATH, ""),
                        description: "TTS 软件的可执行文件路径".to_string(),
                        setting_type: "path".to_string(),
                    },
                    ConfigSetting {
                        key: keys::VOICE_CHECK.to_string(),
                        value: read_setting(app, keys::VOICE_CHECK, "false"),
                        description: "启动时检查语音模型是否就绪".to_string(),
                        setting_type: "bool".to_string(),
                    },
                ],
            },
        );

        tts_subs.insert(
            "适配器 URL".to_string(),
            Subcategory {
                description: "各个 TTS 后端的 API 地址，对应原环境变量 SIMPLE_VITS_API_URL / STYLE_BERT_VITS2_URL 等".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: tts::keys::SIMPLE_VITS_API_URL.to_string(),
                        value: read_setting(app, tts::keys::SIMPLE_VITS_API_URL, "http://127.0.0.1:23456"),
                        description: "Simple-Vits-API 地址（VITS 适配器）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::BV2_API_URL.to_string(),
                        value: read_setting(app, tts::keys::BV2_API_URL, "http://127.0.0.1:6006"),
                        description: "Simple-Vits-API 地址（Bert-Vits2 适配器）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::GSV_API_URL.to_string(),
                        value: read_setting(app, tts::keys::GSV_API_URL, "http://127.0.0.1:9880"),
                        description: "GPT-SoVITS API 地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::SBV2_API_URL.to_string(),
                        value: read_setting(app, tts::keys::SBV2_API_URL, "http://127.0.0.1:5000"),
                        description: "Style-Bert-Vits2 本地服务地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::SBV2API_API_URL.to_string(),
                        value: read_setting(app, tts::keys::SBV2API_API_URL, "http://localhost:3000"),
                        description: "SBV2 API 服务地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::AIVIS_API_URL.to_string(),
                        value: read_setting(app, tts::keys::AIVIS_API_URL, "https://api.aivis-project.com/v1"),
                        description: "AIVIS 云 API 地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::AIVIS_API_KEY.to_string(),
                        value: read_setting(app, tts::keys::AIVIS_API_KEY, ""),
                        description: "AIVIS API 密钥（原环境变量 AIVIS_API_KRY）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::INDEXTTS_API_URL.to_string(),
                        value: read_setting(app, tts::keys::INDEXTTS_API_URL, "http://127.0.0.1:23467/voice/indextts/presets"),
                        description: "IndexTTS2 API 地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                ],
            },
        );

        tts_subs.insert(
            "音频参数".to_string(),
            Subcategory {
                description: "TTS 音频输出格式与语言设置，对应原环境变量 TTS_AUDIO_FORMAT / VOICE_LANG".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: tts::keys::TTS_AUDIO_FORMAT.to_string(),
                        value: read_setting(app, tts::keys::TTS_AUDIO_FORMAT, "wav"),
                        description: "音频文件格式（wav / mp3 / flac / ogg 等）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: tts::keys::VOICE_LANG.to_string(),
                        value: read_setting(app, tts::keys::VOICE_LANG, "ja"),
                        description: "语音合成语言（ja / zh / auto）".to_string(),
                        setting_type: "text".to_string(),
                    },
                ],
            },
        );

        tree.insert("TTS 配置".to_string(), Category { subcategories: tts_subs });
    }

    // ===== 主动对话配置 =====
    {
        let mut proactive_subs = BTreeMap::new();

        // 核心开关
        proactive_subs.insert(
            "基础开关".to_string(),
            Subcategory {
                description: "主动对话功能的核心开关与触发频率设置".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: proactive::keys::ENABLE_PROACTIVE_SYSTEM.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_PROACTIVE_SYSTEM, "false"),
                        description: "ENABLE_PROACTIVE_SYSTEM — 是否启用主动对话系统".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::MAX_PROACTIVE_TIMES.to_string(),
                        value: read_setting(app, proactive::keys::MAX_PROACTIVE_TIMES, "3"),
                        description: "MAX_PROACTIVE_TIMES — 每日主动出击上限次数".to_string(),
                        setting_type: "text".to_string(),
                    },
                ],
            },
        );

        // 视觉与模型配置
        proactive_subs.insert(
            "视觉与模型配置".to_string(),
            Subcategory {
                description: "主动对话时调用的 Vision LLM 视觉分析模型以及截图分析设置".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: proactive::keys::VD_API_KEY.to_string(),
                        value: read_setting(app, proactive::keys::VD_API_KEY, ""),
                        description: "VD_API_KEY — 视觉模型 API Key".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::VD_BASE_URL.to_string(),
                        value: read_setting(app, proactive::keys::VD_BASE_URL, "https://dashscope.aliyuncs.com/compatible-mode/v1"),
                        description: "VD_BASE_URL — 视觉模型 API Base URL".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::VD_MODEL.to_string(),
                        value: read_setting(app, proactive::keys::VD_MODEL, "qwen3.5-plus"),
                        description: "VD_MODEL — 视觉模型型号".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_VISUAL_PRECEPTION.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_VISUAL_PRECEPTION, "true"),
                        description: "ENABLE_VISUAL_PRECEPTION — 是否允许主动视觉感知桌面画面（偷看屏幕）".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::SCREEN_WEIGHT.to_string(),
                        value: read_setting(app, proactive::keys::SCREEN_WEIGHT, "30.0"),
                        description: "SCREEN_WEIGHT — 视觉模式触发权重（越大越容易偷看屏幕聊天，默认 30）".to_string(),
                        setting_type: "text".to_string(),
                    },
                ],
            },
        );

        // 感知与话题配置
        proactive_subs.insert(
            "感知与话题配置".to_string(),
            Subcategory {
                description: "日程、TODO与随机对话的权重及开关配置".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: proactive::keys::ENABLE_TOPIC_CREATER.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_TOPIC_CREATER, "true"),
                        description: "ENABLE_TOPIC_CREATER — 允许自主寻找并开启新话题".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::TOPIC_WEIGHT.to_string(),
                        value: read_setting(app, proactive::keys::TOPIC_WEIGHT, "30.0"),
                        description: "TOPIC_WEIGHT — 随机话题触发权重（默认 30）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_TODO_PRECEPTION.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_TODO_PRECEPTION, "true"),
                        description: "ENABLE_TODO_PRECEPTION — 允许在闲暇时自动读取未完成 TODO 并温和提醒".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::TODO_WEIGHT.to_string(),
                        value: read_setting(app, proactive::keys::TODO_WEIGHT, "40.0"),
                        description: "TODO_WEIGHT — TODO 提醒触发权重（默认 40）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_SCHEDULE_REMINDER.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_SCHEDULE_REMINDER, "true"),
                        description: "ENABLE_SCHEDULE_REMINDER — 启用强日程日程报时弹窗提醒".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_IMPORTANT_DAY_REMINDER.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_IMPORTANT_DAY_REMINDER, "true"),
                        description: "ENABLE_IMPORTANT_DAY_REMINDER — 启用重要节日与特殊日子暖心提醒".to_string(),
                        setting_type: "bool".to_string(),
                    },
                ],
            },
        );

        tree.insert("主动对话配置".to_string(), Category { subcategories: proactive_subs });
    }

    // ===== 其他设置 =====
    {
        let mut other_subs = BTreeMap::new();

        other_subs.insert(
            "基础设置".to_string(),
            Subcategory {
                description: "其他杂项配置".to_string(),
                settings: vec![ConfigSetting {
                    key: keys::COMMUNITY_URL.to_string(),
                    value: read_setting(app, keys::COMMUNITY_URL, ""),
                    description: "社区或资源下载地址".to_string(),
                    setting_type: "text".to_string(),
                }],
            },
        );

        tree.insert("其他设置".to_string(), Category { subcategories: other_subs });
    }

    tree
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub fn get_settings_tree(app: AppHandle) -> ConfigTree {
    build_config_tree(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, values: BTreeMap<String, String>) -> Result<String, String> {
    let store = settings_store(&app).map_err(|e| e.to_string())?;

    for (key, value) in &values {
        let json_value = if value == "true" {
            JsonValue::Bool(true)
        } else if value == "false" {
            JsonValue::Bool(false)
        } else if let Ok(n) = value.parse::<i64>() {
            JsonValue::Number(n.into())
        } else if let Ok(n) = value.parse::<f64>() {
            // 浮点数也存为 Number（serde_json 内部会区分）
            if let Some(f) = serde_json::Number::from_f64(n) {
                JsonValue::Number(f)
            } else {
                JsonValue::String(value.clone())
            }
        } else {
            JsonValue::String(value.clone())
        };
        store.set(key.clone(), json_value);
    }

    store.save().map_err(|e| e.to_string())?;

    Ok("配置已成功保存并已生效！".to_string())
}

#[tauri::command]
pub fn get_setting_by_key(app: AppHandle, key: String) -> Result<ConfigSetting, String> {
    let tree = build_config_tree(&app);
    for category in tree.values() {
        for sub in category.subcategories.values() {
            for setting in &sub.settings {
                if setting.key == key {
                    return Ok(setting.clone());
                }
            }
        }
    }
    Err(format!("Key '{}' not found", key))
}

#[tauri::command]
pub fn select_file(app: AppHandle) -> Result<Option<String>, String> {
    let file = app.dialog().file().blocking_pick_file();
    Ok(file.map(|f| f.to_string()))
}
