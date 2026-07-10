use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_store::{Store, StoreExt};

pub const STORE_FILE: &str = "settings.json";

pub mod proactive;
pub mod tts;

use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, load_providers, load_role_assignment, save_providers,
    save_role_assignment, LlmProviderConfig, LlmProvidersResponse,
};
use crate::config::tts::TtsConfig;
use crate::window_geometry::{
    self, WindowDimensions, WindowSizePlan, MAIN_WINDOW_DEFAULT_HEIGHT, MAIN_WINDOW_DEFAULT_WIDTH,
};

// ========== 字段键（对标 Python .env） ==========
pub mod keys {
    // LLM 连接（对应 LLM_PROVIDER / MODEL_TYPE / CHAT_API_KEY / CHAT_BASE_URL）
    pub const LLM_PROVIDER: &str = "llm.provider";
    pub const LLM_MODEL: &str = "llm.model";
    pub const LLM_API_KEY: &str = "llm.api_key";
    pub const LLM_BASE_URL: &str = "llm.base_url";

    // LLM 多供应商管理
    pub const LLM_PROVIDERS: &str = "llm.providers";
    pub const LLM_CHAT_PROVIDER_ID: &str = "llm.chat_provider_id";
    pub const LLM_TRANSLATE_PROVIDER_ID: &str = "llm.translate_provider_id";
    pub const LLM_GOD_AGENT_PROVIDER_ID: &str = "llm.god_agent_provider_id";

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

    // 界面设置
    pub const WINDOW_WIDTH: &str = "ui.window_width";
    pub const WINDOW_HEIGHT: &str = "ui.window_height";
    pub const WINDOW_RESOLUTION_PRESET: &str = "ui.window_resolution_preset";

    // 功能开关
    pub const USE_PERSISTENT_MEMORY: &str = "features.use_persistent_memory";
    pub const MEMORY_UPDATE_INTERVAL: &str = "features.memory_update_interval";
    pub const MEMORY_RECENT_WINDOW: &str = "features.memory_recent_window";

    // TTS
    pub const AUTO_START_TTS_SOFTWARE: &str = "tts.auto_start";
    pub const TTS_SOFTWARE_PATH: &str = "tts.software_path";
    pub const VOICE_CHECK: &str = "tts.voice_check";

    // 其他
    /// 上次游玩的角色 ID（启动时自动恢复）
    pub const LAST_CHARACTER_ID: &str = "game.last_character_id";
    /// 上次选择的场景 ID（启动时自动恢复）
    pub const LAST_SCENE_ID: &str = "game.last_scene_id";
    /// 场景感知开关（切换场景时是否自动产生旁白台词）
    pub const SCENE_AWARENESS_ENABLED: &str = "game.scene_awareness_enabled";

    // 上帝 Agent（God Agent）多人对话
    pub const GOD_AGENT_MAX_CONSECUTIVE_NPC: &str = "god_agent.max_consecutive_npc";
    pub const GOD_AGENT_RECENT_WINDOW: &str = "god_agent.recent_window";

    // 创意工坊
    /// GitHub Personal Access Token（可选，用于 GraphQL 获取 upvote 数）
    pub const GITHUB_TOKEN: &str = "workshop.github_token";
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

    /// TTS 引擎配置（适配器 URL、音频格式等）
    #[serde(default)]
    pub tts: TtsConfig,
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
    50
}
fn default_memory_recent_window() -> u32 {
    15
}

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
            use_persistent_memory: true,
            memory_update_interval: 50,
            memory_recent_window: 15,
            auto_start_tts_software: false,
            tts_software_path: None,
            voice_check: false,
            tts: TtsConfig::default(),
        }
    }
}

fn get_string(store: &Store<Wry>, key: &str) -> Option<String> {
    store
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// Public accessor for reading a string value from the settings store.
pub fn get_setting_string(app: &AppHandle, key: &str) -> Option<String> {
    settings_store(app)
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
            use_persistent_memory: get_bool(&store, keys::USE_PERSISTENT_MEMORY, true),
            memory_update_interval: get_u32(&store, keys::MEMORY_UPDATE_INTERVAL, 250),
            memory_recent_window: get_u32(&store, keys::MEMORY_RECENT_WINDOW, 30),
            auto_start_tts_software: get_bool(&store, keys::AUTO_START_TTS_SOFTWARE, false),
            tts_software_path: get_string(&store, keys::TTS_SOFTWARE_PATH),
            voice_check: get_bool(&store, keys::VOICE_CHECK, false),
            tts: TtsConfig::from_store(Some(&store)),
        })
    }
}

pub fn settings_store(app: &AppHandle) -> Result<Arc<Store<Wry>>> {
    app.store(STORE_FILE)
        .context("Failed to open settings store")
}

// ========== 结构化配置树（前端"高级设置"页面使用） ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSetting {
    pub key: String,
    pub value: String,
    pub description: String,
    #[serde(rename = "type")]
    pub setting_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
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

pub fn read_setting(app: &AppHandle, key: &str, default: &str) -> String {
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

        // 高级选项
        llm_subs.insert(
            "高级选项".to_string(),
            Subcategory {
                description: "调优 AI 对话行为的高级参数".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::LLM_OUTPUT_SEC_LANG.to_string(),
                        value: read_setting(app, keys::LLM_OUTPUT_SEC_LANG, "true"),
                        description:
                            "LLM_OUTPUT_SEC_LANG — 是否允许输出第二语言（关闭后仅输出中文）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::CONSUMERS.to_string(),
                        value: read_setting(app, keys::CONSUMERS, "3"),
                        description: "COMSUMERS — 并发消费者数量（增大可加速流式输出，默认 3）"
                            .to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::LLM_NO_EMOTION_LIMIT.to_string(),
                        value: read_setting(app, keys::LLM_NO_EMOTION_LIMIT, "false"),
                        description:
                            "NO_EMOTION_LIMIT_PROMPT — 解除 emotion 数量限制（可能增加 token 消耗）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                ],
            },
        );

        tree.insert(
            "LLM 配置".to_string(),
            Category {
                subcategories: llm_subs,
            },
        );
    }

    // ===== 翻译配置 =====
    {
        let mut trans_subs = BTreeMap::new();

        trans_subs.insert(
            "功能选项".to_string(),
            Subcategory {
                description: "翻译功能的开关与行为控制".to_string(),
                settings: vec![ConfigSetting {
                    key: keys::TRANSLATE_ENABLE.to_string(),
                    value: read_setting(app, keys::TRANSLATE_ENABLE, "true"),
                    description: "ENABLE_TRANSLATE — 启用 AI 翻译（将中文对话翻译为第二语言）"
                        .to_string(),
                    setting_type: "bool".to_string(),
                    options: vec![],
                }],
            },
        );

        tree.insert(
            "翻译配置".to_string(),
            Category {
                subcategories: trans_subs,
            },
        );
    }

    // ===== 功能设置 =====
    {
        let mut feat_subs = BTreeMap::new();

        // 对话增强
        feat_subs.insert(
            "对话增强".to_string(),
            Subcategory {
                description: "这里可以设置是否启用时间感知和情绪分类器功能".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::ENABLE_TIME_SENSE.to_string(),
                        value: read_setting(app, keys::ENABLE_TIME_SENSE, "true"),
                        description: "USE_TIME_SENSE — 启用时间感知（根据上下文时间添加系统提醒）".to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::ENABLE_EMOTION_CLASSIFIER.to_string(),
                        value: read_setting(app, keys::ENABLE_EMOTION_CLASSIFIER, "true"),
                        description: "ENABLE_EMOTION_CLASSIFIER — 启用情感分类器（ONNX 模型，用于自动标注对话 emotion）".to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                ],
            },
        );

        // 记忆系统
        feat_subs.insert(
            "记忆系统".to_string(),
            Subcategory {
                description: "在这里设定你想要的永久记忆效果".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::USE_PERSISTENT_MEMORY.to_string(),
                        value: read_setting(app, keys::USE_PERSISTENT_MEMORY, "true"),
                        description:
                            "USE_PERSISTENT_MEMORY — 开启后记忆会自动压缩，减少 token 消耗"
                                .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::MEMORY_UPDATE_INTERVAL.to_string(),
                        value: read_setting(app, keys::MEMORY_UPDATE_INTERVAL, "250"),
                        description: "MEMORY_UPDATE_INTERVAL — 触发记忆摘要的新消息数（默认 250）"
                            .to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::MEMORY_RECENT_WINDOW.to_string(),
                        value: read_setting(app, keys::MEMORY_RECENT_WINDOW, "30"),
                        description: "MEMORY_RECENT_WINDOW — 摘要时保留的最近消息数（默认 30）"
                            .to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                ],
            },
        );

        tree.insert(
            "功能设置".to_string(),
            Category {
                subcategories: feat_subs,
            },
        );
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
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::TTS_SOFTWARE_PATH.to_string(),
                        value: read_setting(app, keys::TTS_SOFTWARE_PATH, ""),
                        description: "TTS 软件的可执行文件路径".to_string(),
                        setting_type: "path".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::VOICE_CHECK.to_string(),
                        value: read_setting(app, keys::VOICE_CHECK, "false"),
                        description: "启动时检查语音模型是否就绪".to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
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
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::BV2_API_URL.to_string(),
                        value: read_setting(app, tts::keys::BV2_API_URL, "http://127.0.0.1:6006"),
                        description: "Simple-Vits-API 地址（Bert-Vits2 适配器）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::GSV_API_URL.to_string(),
                        value: read_setting(app, tts::keys::GSV_API_URL, "http://127.0.0.1:9880"),
                        description: "GPT-SoVITS API 地址".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::SBV2_API_URL.to_string(),
                        value: read_setting(app, tts::keys::SBV2_API_URL, "http://127.0.0.1:5000"),
                        description: "Style-Bert-Vits2 本地服务地址".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::SBV2API_API_URL.to_string(),
                        value: read_setting(app, tts::keys::SBV2API_API_URL, "http://localhost:3000"),
                        description: "SBV2 API 服务地址".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::AIVIS_API_URL.to_string(),
                        value: read_setting(app, tts::keys::AIVIS_API_URL, "https://api.aivis-project.com/v1"),
                        description: "AIVIS 云 API 地址".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::AIVIS_API_KEY.to_string(),
                        value: read_setting(app, tts::keys::AIVIS_API_KEY, ""),
                        description: "AIVIS API 密钥（原环境变量 AIVIS_API_KRY）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::INDEXTTS_API_URL.to_string(),
                        value: read_setting(app, tts::keys::INDEXTTS_API_URL, "http://127.0.0.1:23467/voice/indextts/presets"),
                        description: "IndexTTS2 API 地址".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::OPENTTS_API_URL.to_string(),
                        value: read_setting(app, tts::keys::OPENTTS_API_URL, "https://api.siliconflow.cn/v1"),
                        description: "OpenTTS API 地址（硅基流动）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::OPENTTS_API_KEY.to_string(),
                        value: read_setting(app, tts::keys::OPENTTS_API_KEY, ""),
                        description: "OpenTTS API 密钥".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::OPENTTS_MODEL.to_string(),
                        value: read_setting(app, tts::keys::OPENTTS_MODEL, "FunAudioLLM/CosyVoice2-0.5B"),
                        description: "OpenTTS 模型名称".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: tts::keys::OPENTTS_VOICE.to_string(),
                        value: read_setting(app, tts::keys::OPENTTS_VOICE, "speech:pai:7s86w73x9i:vkgcswgqicskwpdwevri"),
                        description: "OpenTTS voice / 音色标识".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                ],
            },
        );

        tts_subs.insert(
            "音频参数".to_string(),
            Subcategory {
                description:
                    "TTS 音频输出格式与语言设置，对应原环境变量 TTS_AUDIO_FORMAT / VOICE_LANG"
                        .to_string(),
                settings: vec![
                    ConfigSetting {
                        key: tts::keys::TTS_AUDIO_FORMAT.to_string(),
                        value: read_setting(app, tts::keys::TTS_AUDIO_FORMAT, "wav"),
                        description: "音频文件格式（wav / mp3 / flac / ogg 等）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    // 语音语言改为角色级配置，全局入口隐藏
                    // ConfigSetting {
                    //     key: tts::keys::VOICE_LANG.to_string(),
                    //     value: read_setting(app, tts::keys::VOICE_LANG, "ja"),
                    //     description: "语音合成语言（ja / zh / auto）".to_string(),
                    //     setting_type: "text".to_string(),
                    //     options: vec![],
                    // },
                ],
            },
        );

        tree.insert(
            "TTS 配置".to_string(),
            Category {
                subcategories: tts_subs,
            },
        );
    }

    // ===== 创意工坊 =====
    {
        let mut workshop_subs = BTreeMap::new();

        workshop_subs.insert(
            "GitHub Token".to_string(),
            Subcategory {
                description: "配置 GitHub Personal Access Token 以获取准确的 Discussion upvote 热度排序（可选）".to_string(),
                settings: vec![ConfigSetting {
                    key: keys::GITHUB_TOKEN.to_string(),
                    value: read_setting(app, keys::GITHUB_TOKEN, ""),
                    description: "填入你的 GitHub Token（无需任何权限，仅用于调用 GraphQL API）。留空使用 REST API，无法获取独立 upvote 数（会用 👍 表情数代替）。Token 创建地址：https://github.com/settings/tokens".to_string(),
                    setting_type: "text".to_string(),
                        options: vec![],
                    }],
            },
        );

        tree.insert(
            "创意工坊".to_string(),
            Category {
                subcategories: workshop_subs,
            },
        );
    }

    // ===== 界面设置 =====
    {
        let mut ui_subs = BTreeMap::new();

        ui_subs.insert(
            "窗口".to_string(),
            Subcategory {
                description: "主窗口内容区尺寸（逻辑像素；保存后立即安全应用）".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::WINDOW_RESOLUTION_PRESET.to_string(),
                        value: read_setting(app, keys::WINDOW_RESOLUTION_PRESET, "default"),
                        description: "尺寸预设".to_string(),
                        setting_type: "select".to_string(),
                        options: vec![
                            "default".to_string(),
                            "fit".to_string(),
                            "1920x1080".to_string(),
                            "2560x1440".to_string(),
                            "1280x720".to_string(),
                            "custom".to_string(),
                        ],
                    },
                    ConfigSetting {
                        key: keys::WINDOW_WIDTH.to_string(),
                        value: read_setting(app, keys::WINDOW_WIDTH, "1500"),
                        description: "内容区宽度（逻辑像素，默认 1500）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: keys::WINDOW_HEIGHT.to_string(),
                        value: read_setting(app, keys::WINDOW_HEIGHT, "800"),
                        description: "内容区高度（逻辑像素，默认 800）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                ],
            },
        );

        tree.insert(
            "界面设置".to_string(),
            Category {
                subcategories: ui_subs,
            },
        );
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
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::MAX_PROACTIVE_TIMES.to_string(),
                        value: read_setting(app, proactive::keys::MAX_PROACTIVE_TIMES, "3"),
                        description: "MAX_PROACTIVE_TIMES — 在用户响应之前，能主动对话的次数".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::PROACTIVE_INTERVAL_SECS.to_string(),
                        value: read_setting(app, proactive::keys::PROACTIVE_INTERVAL_SECS, "10"),
                        description: "PROACTIVE_INTERVAL_SECS — 主动对话轮询间隔（秒，默认 10，最小 2）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::INTEREST_TRIGGER_THRESHOLD.to_string(),
                        value: read_setting(app, proactive::keys::INTEREST_TRIGGER_THRESHOLD, "30.0"),
                        description: "INTEREST_TRIGGER_THRESHOLD — 兴趣度触发阈值（默认 30，越低越容易被触发）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::INTEREST_DECAY_STEP.to_string(),
                        value: read_setting(app, proactive::keys::INTEREST_DECAY_STEP, "15.0"),
                        description: "INTEREST_DECAY_STEP — 每次主动对话后兴趣度上限衰减量（默认 15，设为 0 则不衰减）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
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
                        key: proactive::keys::VD_FOLLOW_CHAT_MODEL.to_string(),
                        value: read_setting(app, proactive::keys::VD_FOLLOW_CHAT_MODEL, "true"),
                        description: "VD_FOLLOW_CHAT_MODEL — 跟随当前对话模型；若当前是不可关闭长思考的模型，视觉识别也会明显变慢。低延迟建议关闭并配置独立轻量视觉模型".to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::VD_API_KEY.to_string(),
                        value: read_setting(app, proactive::keys::VD_API_KEY, ""),
                        description: "VD_API_KEY — 视觉模型 API Key".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::VD_BASE_URL.to_string(),
                        value: read_setting(
                            app,
                            proactive::keys::VD_BASE_URL,
                            "https://dashscope.aliyuncs.com/compatible-mode/v1",
                        ),
                        description: "VD_BASE_URL — 视觉模型 API Base URL".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::VD_MODEL.to_string(),
                        value: read_setting(app, proactive::keys::VD_MODEL, "qwen3-vl-flash"),
                        description:
                            "VD_MODEL — 独立视觉模型型号（低延迟推荐非思考型 VL/Flash，例如 qwen3-vl-flash）"
                                .to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_VISUAL_PRECEPTION.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_VISUAL_PRECEPTION, "true"),
                        description:
                            "ENABLE_VISUAL_PRECEPTION — 是否允许主动视觉感知桌面画面（偷看屏幕）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::SCREEN_WEIGHT.to_string(),
                        value: read_setting(app, proactive::keys::SCREEN_WEIGHT, "30.0"),
                        description:
                            "SCREEN_WEIGHT — 视觉模式触发权重（越大越容易偷看屏幕聊天，默认 30）"
                                .to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::VISUAL_PERCEPTION_PRIORITY.to_string(),
                        value: read_setting(app, proactive::keys::VISUAL_PERCEPTION_PRIORITY, "false"),
                        description:
                            "VISUAL_PERCEPTION_PRIORITY — 视觉理解优先模式（开启后每次主动回复优先看屏幕，失败才找话题）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
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
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::TOPIC_WEIGHT.to_string(),
                        value: read_setting(app, proactive::keys::TOPIC_WEIGHT, "60.0"),
                        description: "TOPIC_WEIGHT — 随机话题触发权重（默认 60）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_TODO_PRECEPTION.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_TODO_PRECEPTION, "true"),
                        description:
                            "ENABLE_TODO_PRECEPTION — 允许在闲暇时自动读取未完成 TODO 并温和提醒"
                                .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::TODO_WEIGHT.to_string(),
                        value: read_setting(app, proactive::keys::TODO_WEIGHT, "10.0"),
                        description: "TODO_WEIGHT — TODO 提醒触发权重（默认 10）".to_string(),
                        setting_type: "text".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_SCHEDULE_REMINDER.to_string(),
                        value: read_setting(app, proactive::keys::ENABLE_SCHEDULE_REMINDER, "true"),
                        description: "ENABLE_SCHEDULE_REMINDER — 启用强日程日程报时弹窗提醒"
                            .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                    ConfigSetting {
                        key: proactive::keys::ENABLE_IMPORTANT_DAY_REMINDER.to_string(),
                        value: read_setting(
                            app,
                            proactive::keys::ENABLE_IMPORTANT_DAY_REMINDER,
                            "true",
                        ),
                        description:
                            "ENABLE_IMPORTANT_DAY_REMINDER — 启用重要节日与特殊日子暖心提醒"
                                .to_string(),
                        setting_type: "bool".to_string(),
                        options: vec![],
                    },
                ],
            },
        );

        tree.insert(
            "主动对话配置".to_string(),
            Category {
                subcategories: proactive_subs,
            },
        );
    }

    tree
}

// ========== Tauri 命令 ==========

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSaveResult {
    pub status: String,
    pub requested: WindowDimensions,
    pub applied: WindowDimensions,
    pub adjusted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsResult {
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowSaveResult>,
}

fn string_to_json_value(value: &str) -> JsonValue {
    if value == "true" {
        JsonValue::Bool(true)
    } else if value == "false" {
        JsonValue::Bool(false)
    } else if let Ok(number) = value.parse::<i64>() {
        JsonValue::Number(number.into())
    } else if let Ok(number) = value.parse::<f64>() {
        serde_json::Number::from_f64(number)
            .map(JsonValue::Number)
            .unwrap_or_else(|| JsonValue::String(value.to_string()))
    } else {
        JsonValue::String(value.to_string())
    }
}

fn restore_store_values(
    store: &Arc<Store<Wry>>,
    previous_values: &[(String, Option<JsonValue>)],
) -> Result<(), String> {
    for (key, previous) in previous_values {
        match previous {
            Some(value) => store.set(key.clone(), value.clone()),
            None => {
                store.delete(key);
            }
        }
    }
    store
        .save()
        .map_err(|error| format!("回滚配置存储失败：{error}"))
}

pub(crate) fn persist_main_window_size(
    app: &AppHandle,
    dimensions: WindowDimensions,
    preset: Option<&str>,
) -> Result<(), String> {
    let store = settings_store(app).map_err(|error| error.to_string())?;
    store.set(
        keys::WINDOW_WIDTH,
        JsonValue::Number(dimensions.width.into()),
    );
    store.set(
        keys::WINDOW_HEIGHT,
        JsonValue::Number(dimensions.height.into()),
    );
    if let Some(preset) = preset {
        store.set(
            keys::WINDOW_RESOLUTION_PRESET,
            JsonValue::String(preset.to_string()),
        );
    }
    store.save().map_err(|error| error.to_string())
}

fn resolve_window_plan(
    app: &AppHandle,
    window: &tauri::WebviewWindow<Wry>,
    values: &mut BTreeMap<String, String>,
) -> Result<WindowSizePlan, String> {
    let preset = values
        .get(keys::WINDOW_RESOLUTION_PRESET)
        .map(String::as_str);

    let requested = match preset {
        Some("default") => {
            WindowDimensions::new(MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_DEFAULT_HEIGHT)
        }
        Some("fit") => window_geometry::recommended_main_window_size(window)?,
        Some("1920x1080") => WindowDimensions::new(1920, 1080),
        Some("2560x1440") => WindowDimensions::new(2560, 1440),
        Some("1280x720") => WindowDimensions::new(1280, 720),
        Some("custom") | None => {
            let width_raw = values.get(keys::WINDOW_WIDTH).cloned().unwrap_or_else(|| {
                read_setting(
                    app,
                    keys::WINDOW_WIDTH,
                    &MAIN_WINDOW_DEFAULT_WIDTH.to_string(),
                )
            });
            let height_raw = values.get(keys::WINDOW_HEIGHT).cloned().unwrap_or_else(|| {
                read_setting(
                    app,
                    keys::WINDOW_HEIGHT,
                    &MAIN_WINDOW_DEFAULT_HEIGHT.to_string(),
                )
            });
            window_geometry::parse_main_window_size(&width_raw, &height_raw)?
        }
        Some(other) => return Err(format!("未知的窗口尺寸预设：{other}")),
    };

    let plan = window_geometry::plan_main_window_size(window, requested)?;
    // Persist the safe effective size, not an impossible off-screen request.
    values.insert(
        keys::WINDOW_WIDTH.to_string(),
        plan.applied.width.to_string(),
    );
    values.insert(
        keys::WINDOW_HEIGHT.to_string(),
        plan.applied.height.to_string(),
    );
    Ok(plan)
}

#[tauri::command]
pub fn get_settings_tree(app: AppHandle) -> ConfigTree {
    build_config_tree(&app)
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: tauri::State<'_, crate::api::pet::HitTestState>,
    mut values: BTreeMap<String, String>,
) -> Result<SaveSettingsResult, String> {
    let store = settings_store(&app).map_err(|e| e.to_string())?;

    let has_window_values = values.contains_key(keys::WINDOW_RESOLUTION_PRESET)
        || values.contains_key(keys::WINDOW_WIDTH)
        || values.contains_key(keys::WINDOW_HEIGHT);

    // Serialize main-window saves with pet-mode transitions.  Unrelated
    // settings do not need to wait on native window operations.
    let _window_transition = if has_window_values {
        Some(
            state
                .transition_lock
                .lock()
                .map_err(|error| format!("等待窗口模式切换失败：{error}"))?,
        )
    } else {
        None
    };

    let main_window = if has_window_values {
        Some(
            app.get_webview_window("main")
                .ok_or_else(|| "找不到 main 窗口，无法校验并应用窗口尺寸".to_string())?,
        )
    } else {
        None
    };

    let target_window_plan = if let Some(window) = main_window.as_ref() {
        Some(
            resolve_window_plan(&app, window, &mut values).map_err(|message| {
                tracing::warn!("拒绝保存无效窗口尺寸：{message}");
                message
            })?,
        )
    } else {
        None
    };

    // Read the mode before mutating the store so a poisoned state lock cannot
    // leave a successfully written configuration with no corresponding result.
    let pet_mode_enabled = if target_window_plan.is_some() {
        Some(
            *state
                .enabled
                .lock()
                .map_err(|error| format!("读取桌宠模式状态失败：{error}"))?,
        )
    } else {
        None
    };

    let previous_values: Vec<_> = values
        .keys()
        .map(|key| (key.clone(), store.get(key)))
        .collect();

    for (key, value) in &values {
        store.set(key.clone(), string_to_json_value(value));
    }

    if let Err(error) = store.save() {
        let rollback_error = restore_store_values(&store, &previous_values).err();
        return Err(match rollback_error {
            Some(rollback_error) => {
                format!("保存配置失败：{error}；同时无法完整恢复原配置：{rollback_error}")
            }
            None => format!("保存配置失败，已恢复原配置：{error}"),
        });
    }

    let window_result = if let (Some(window), Some(plan)) =
        (main_window.as_ref(), target_window_plan.as_ref())
    {
        if pet_mode_enabled.unwrap_or(false) {
            tracing::info!(
                width = plan.applied.width,
                height = plan.applied.height,
                "桌宠模式已开启，主窗口尺寸已保存并延迟到退出桌宠时应用"
            );
            Some(WindowSaveResult {
                status: "deferred".to_string(),
                requested: plan.requested,
                applied: plan.applied,
                adjusted: plan.adjusted,
            })
        } else {
            let applied = match window_geometry::apply_main_window_plan(window, plan, None) {
                Ok(applied) => applied,
                Err(error) => {
                    let rollback_error = restore_store_values(&store, &previous_values).err();
                    return Err(match rollback_error {
                        Some(rollback_error) => format!(
                            "应用窗口尺寸失败：{error}；同时无法完整恢复原配置：{rollback_error}"
                        ),
                        None => format!("应用窗口尺寸失败，已恢复原配置：{error}"),
                    });
                }
            };
            let final_plan = applied.plan;

            // Fullscreen/maximized transitions can change the measured frame or
            // active DPI.  If the final safe plan differs from the preliminary
            // one, commit the authoritative size and roll back both native state
            // and Store if this second persistence step fails.
            if final_plan.applied != plan.applied {
                store.set(
                    keys::WINDOW_WIDTH,
                    JsonValue::Number(final_plan.applied.width.into()),
                );
                store.set(
                    keys::WINDOW_HEIGHT,
                    JsonValue::Number(final_plan.applied.height.into()),
                );
                if let Err(error) = store.save() {
                    let store_rollback = restore_store_values(&store, &previous_values).err();
                    let window_rollback =
                        window_geometry::rollback_applied_main_window_plan(window, &applied).err();
                    let mut rollback_errors = Vec::new();
                    if let Some(error) = store_rollback {
                        rollback_errors.push(error);
                    }
                    if let Some(error) = window_rollback {
                        rollback_errors.push(error);
                    }
                    return Err(if rollback_errors.is_empty() {
                        format!("保存最终窗口尺寸失败，配置与窗口均已恢复：{error}")
                    } else {
                        format!(
                            "保存最终窗口尺寸失败：{error}；回滚不完整：{}",
                            rollback_errors.join("；")
                        )
                    });
                }
            }
            Some(WindowSaveResult {
                status: "applied".to_string(),
                requested: final_plan.requested,
                applied: final_plan.applied,
                adjusted: final_plan.adjusted,
            })
        }
    } else {
        None
    };

    let message = match window_result.as_ref() {
        Some(result) if result.status == "deferred" && result.adjusted => format!(
            "配置已保存；目标尺寸 {}x{} 超出当前显示器工作区，已安全调整为 {}x{}，将在退出桌宠后应用。",
            result.requested.width,
            result.requested.height,
            result.applied.width,
            result.applied.height
        ),
        Some(result) if result.status == "deferred" => format!(
            "配置已保存；主窗口将在退出桌宠后调整为 {}x{}。",
            result.applied.width, result.applied.height
        ),
        Some(result) if result.adjusted => format!(
            "目标尺寸 {}x{} 超出当前显示器工作区，已安全调整并应用为 {}x{}。",
            result.requested.width,
            result.requested.height,
            result.applied.width,
            result.applied.height
        ),
        Some(result) => format!(
            "配置已保存，主窗口内容区已调整为 {}x{}。",
            result.applied.width, result.applied.height
        ),
        None => "配置已成功保存。".to_string(),
    };

    Ok(SaveSettingsResult {
        status: "success".to_string(),
        message,
        window: window_result,
    })
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

// ============================================================
// LLM multi-provider management commands
// ============================================================

#[tauri::command]
pub fn list_llm_providers(app: AppHandle) -> LlmProvidersResponse {
    let providers = load_providers(&app);
    let assignment = load_role_assignment(&app);
    LlmProvidersResponse {
        providers,
        chat_provider_id: assignment.chat_provider_id,
        translate_provider_id: assignment.translate_provider_id,
        god_agent_provider_id: assignment.god_agent_provider_id,
    }
}

#[tauri::command]
pub fn save_llm_provider(app: AppHandle, provider: LlmProviderConfig) -> Result<(), String> {
    let mut providers = load_providers(&app);

    let id = if provider.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        provider.id.clone()
    };

    let mut updated = provider;
    updated.id = id.clone();

    // Insert or update
    if let Some(pos) = providers.iter().position(|p| p.id == id) {
        providers[pos] = updated;
    } else {
        providers.push(updated);
    }

    save_providers(&app, &providers).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_llm_provider(app: AppHandle, id: String) -> Result<(), String> {
    let mut providers = load_providers(&app);
    providers.retain(|p| p.id != id);
    save_providers(&app, &providers).map_err(|e| e.to_string())?;

    // Clear role assignment if this was the selected provider
    let mut assignment = load_role_assignment(&app);
    let mut changed = false;
    if assignment.chat_provider_id.as_deref() == Some(&id) {
        assignment.chat_provider_id = None;
        changed = true;
    }
    if assignment.translate_provider_id.as_deref() == Some(&id) {
        assignment.translate_provider_id = None;
        changed = true;
    }
    if changed {
        save_role_assignment(&app, &assignment).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn set_llm_role(
    app: AppHandle,
    role: String,
    provider_id: Option<String>,
) -> Result<(), String> {
    // Validate that the provider exists (unless setting to None)
    if let Some(ref pid) = provider_id {
        let providers = load_providers(&app);
        if !providers.iter().any(|p| p.id == *pid) {
            return Err(format!("Provider '{pid}' not found"));
        }
    }

    let mut assignment = load_role_assignment(&app);
    match role.as_str() {
        "chat" => assignment.chat_provider_id = provider_id,
        "translate" => assignment.translate_provider_id = provider_id,
        "god_agent" => assignment.god_agent_provider_id = provider_id,
        other => return Err(format!("Invalid role: {other}")),
    }
    save_role_assignment(&app, &assignment).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_llm_provider(
    provider: LlmProviderConfig,
    message: String,
) -> Result<String, String> {
    let Some(client) = build_llm_client_from_provider(&provider) else {
        return Err("无法创建 LLM 客户端：请检查 API Key 和模型名称".to_string());
    };

    let messages = vec![
        crate::ai_service::types::LlmMessage::system(
            "你是一个有帮助的AI助手。请简洁地回答用户的问题。",
        ),
        crate::ai_service::types::LlmMessage::user(&message),
    ];

    let timeout = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.complete(&messages),
    )
    .await
    .map_err(|_| "请求超时（30秒），请检查网络或 API 地址".to_string())?;

    timeout.map_err(|e| format!("测试请求失败: {e}"))
}
