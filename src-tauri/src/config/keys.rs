//! 所有 settings.json 存储键的字符串常量。
//!
//! 合并自原 mod.rs、proactive.rs、tts.rs 中的 keys 子模块。

// ========== LLM 连接（对应 LLM_PROVIDER / MODEL_TYPE / CHAT_API_KEY / CHAT_BASE_URL） ==========
pub const LLM_PROVIDER: &str = "llm.provider";
pub const LLM_MODEL: &str = "llm.model";
pub const LLM_API_KEY: &str = "llm.api_key";
pub const LLM_BASE_URL: &str = "llm.base_url";

// ========== LLM 多供应商管理 ==========
pub const LLM_PROVIDERS: &str = "llm.providers";
pub const LLM_CHAT_PROVIDER_ID: &str = "llm.chat_provider_id";
pub const LLM_TRANSLATE_PROVIDER_ID: &str = "llm.translate_provider_id";
pub const LLM_GOD_AGENT_PROVIDER_ID: &str = "llm.god_agent_provider_id";
pub const LLM_VISION_PROVIDER_ID: &str = "llm.vision_provider_id";

// ========== LLM 生成参数（对应 TEMPERATURE / TOP_P / ENABLE_THINKING） ==========
pub const LLM_TEMPERATURE: &str = "llm.temperature";
pub const LLM_TOP_P: &str = "llm.top_p";
pub const LLM_ENABLE_THINKING: &str = "llm.enable_thinking";

// ========== LLM 高级选项 ==========
pub const LLM_OUTPUT_SEC_LANG: &str = "llm.output_sec_lang";
pub const CONSUMERS: &str = "llm.consumers";
pub const LLM_NO_EMOTION_LIMIT: &str = "llm.no_emotion_limit_prompt";
pub const LLM_TIMEOUT_SECS: &str = "llm.timeout_secs";

// ========== 翻译（对应 TRANSLATE_LLM_PROVIDER / TRANSLATE_MODEL / TRANSLATE_API_KEY / TRANSLATE_BASE_URL） ==========
pub const TRANSLATE_PROVIDER: &str = "translate.provider";
pub const TRANSLATE_MODEL: &str = "translate.model";
pub const TRANSLATE_API_KEY: &str = "translate.api_key";
pub const TRANSLATE_BASE_URL: &str = "translate.base_url";
pub const TRANSLATE_ENABLE: &str = "translate.enable";

// ========== 对话增强 ==========
pub const ENABLE_TIME_SENSE: &str = "features.enable_time_sense";
pub const ENABLE_EMOTION_CLASSIFIER: &str = "features.enable_emotion_classifier";

// ========== 功能开关（记忆系统） ==========
pub const USE_PERSISTENT_MEMORY: &str = "features.use_persistent_memory";
pub const MEMORY_UPDATE_INTERVAL: &str = "features.memory_update_interval";
pub const MEMORY_RECENT_WINDOW: &str = "features.memory_recent_window";

// ========== TTS 本地引擎 ==========
pub const ENABLE_LOCAL_TTS: &str = "features.enable_local_tts";

// ========== TTS 适配器后端 URL ==========
pub const SIMPLE_VITS_API_URL: &str = "tts.simple_vits_api_url";
pub const BV2_API_URL: &str = "tts.bv2_api_url";
pub const GSV_API_URL: &str = "tts.gsv_api_url";
pub const SBV2_API_URL: &str = "tts.sbv2_api_url";
pub const SBV2API_API_URL: &str = "tts.sbv2api_api_url";
pub const AIVIS_API_URL: &str = "tts.aivis_api_url";
pub const AIVIS_API_KEY: &str = "tts.aivis_api_key";
pub const INDEXTTS_API_URL: &str = "tts.indextts_api_url";

// ========== TTS OpenTTS ==========
pub const OPENTTS_API_URL: &str = "tts.opentts_api_url";
pub const OPENTTS_API_KEY: &str = "tts.opentts_api_key";
pub const OPENTTS_MODEL: &str = "tts.opentts_model";
pub const OPENTTS_VOICE: &str = "tts.opentts_voice";

// ========== TTS 音频参数 ==========
pub const TTS_AUDIO_FORMAT: &str = "tts.audio_format";
pub const VOICE_LANG: &str = "tts.voice_lang";

// ========== 主动对话系统 ==========
pub const ENABLE_PROACTIVE_SYSTEM: &str = "ENABLE_PROACTIVE_SYSTEM";
pub const MAX_PROACTIVE_TIMES: &str = "MAX_PROACTIVE_TIMES";
// 旧的视觉模型独立配置键已废弃，视觉模型统一到大模型管理中配置；
// 这些常量仅保留给迁移逻辑读取旧配置使用。
pub const VD_API_KEY: &str = "VD_API_KEY";
pub const VD_BASE_URL: &str = "VD_BASE_URL";
pub const VD_MODEL: &str = "VD_MODEL";
pub const ENABLE_VISUAL_PRECEPTION: &str = "ENABLE_VISUAL_PRECEPTION";
pub const SCREEN_WEIGHT: &str = "SCREEN_WEIGHT";
pub const ENABLE_TOPIC_CREATER: &str = "ENABLE_TOPIC_CREATER";
pub const TOPIC_WEIGHT: &str = "TOPIC_WEIGHT";
pub const ENABLE_TODO_PRECEPTION: &str = "ENABLE_TODO_PRECEPTION";
pub const TODO_WEIGHT: &str = "TODO_WEIGHT";
pub const ENABLE_SCHEDULE_REMINDER: &str = "ENABLE_SCHEDULE_REMINDER";
pub const ENABLE_IMPORTANT_DAY_REMINDER: &str = "ENABLE_IMPORTANT_DAY_REMINDER";

// ========== 上帝 Agent（God Agent）多人对话 ==========
pub const GOD_AGENT_MAX_CONSECUTIVE_NPC: &str = "god_agent.max_consecutive_npc";
pub const GOD_AGENT_RECENT_WINDOW: &str = "god_agent.recent_window";

// ========== 创意工坊 ==========
/// GitHub Personal Access Token（可选，用于 GraphQL 获取 upvote 数）
pub const GITHUB_TOKEN: &str = "workshop.github_token";

// ========== 日志 ==========
/// 是否启用文件日志记录
pub const LOG_ENABLE: &str = "log.enable";
/// 日志文件保留天数（超过此天数的旧日志在启动时自动清理）
pub const LOG_RETENTION_DAYS: &str = "log.retention_days";
/// 是否记录 LLM 请求体到文件（完整请求 JSON，默认关闭）
pub const LOG_LLM_REQUEST_BODY: &str = "log.llm_request_body";
