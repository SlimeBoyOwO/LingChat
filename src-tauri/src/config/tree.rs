//! 构建前端"高级设置"页面所需的完整配置树。
//!
//! 设计原则：`read_setting()` 的默认值均从对应配置结构体的 `Default` 实现中获取，
//! 确保 UI 显示的默认值与运行时默认值一致。

use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use tauri::AppHandle;

use super::app_config::AppConfig;
use super::keys;
use super::proactive::ProactiveConfig;
use super::tts::TtsConfig;
use super::types::{Category, ConfigSetting, ConfigTree, Subcategory};

// ========== 辅助函数 ==========

fn read_setting(app: &AppHandle, key: &str, default: &str) -> String {
    super::settings_store(app)
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
    let app_defaults = AppConfig::default();
    let tts_defaults = TtsConfig::default();
    let proactive_defaults = ProactiveConfig::default();

    let mut tree = BTreeMap::new();

    // ===== LLM 配置 =====
    {
        let mut llm_subs = BTreeMap::new();

        llm_subs.insert(
            "高级选项".to_string(),
            Subcategory {
                description: "调优 AI 对话行为的高级参数".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::LLM_OUTPUT_SEC_LANG.to_string(),
                        value: read_setting(
                            app,
                            keys::LLM_OUTPUT_SEC_LANG,
                            &app_defaults.llm_output_sec_lang.to_string(),
                        ),
                        description:
                            "LLM_OUTPUT_SEC_LANG — 是否允许输出第二语言（关闭后仅输出中文）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::CONSUMERS.to_string(),
                        value: read_setting(
                            app,
                            keys::CONSUMERS,
                            &app_defaults.consumers.to_string(),
                        ),
                        description: "COMSUMERS — 并发消费者数量（增大可加速流式输出，默认 3）"
                            .to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_TIMEOUT_SECS.to_string(),
                        value: read_setting(
                            app,
                            keys::LLM_TIMEOUT_SECS,
                            &app_defaults.llm_timeout_secs.to_string(),
                        ),
                        description:
                            "LLM 请求空闲超时（秒）— 首次响应及流式相邻事件最长等待时间（10–3600）"
                                .to_string(),
                        setting_type: "number".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LLM_NO_EMOTION_LIMIT.to_string(),
                        value: read_setting(
                            app,
                            keys::LLM_NO_EMOTION_LIMIT,
                            &app_defaults.no_emotion_limit_prompt.to_string(),
                        ),
                        description:
                            "NO_EMOTION_LIMIT_PROMPT — 解除 emotion 数量限制（可能增加 token 消耗）"
                                .to_string(),
                        setting_type: "bool".to_string(),
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
                    value: read_setting(
                        app,
                        keys::TRANSLATE_ENABLE,
                        &app_defaults.enable_translate.to_string(),
                    ),
                    description: "ENABLE_TRANSLATE — 启用 AI 翻译（将中文对话翻译为第二语言）"
                        .to_string(),
                    setting_type: "bool".to_string(),
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
                        value: read_setting(
                            app,
                            keys::ENABLE_TIME_SENSE,
                            &app_defaults.enable_time_sense.to_string(),
                        ),
                        description:
                            "USE_TIME_SENSE — 启用时间感知（根据上下文时间添加系统提醒）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::ENABLE_EMOTION_CLASSIFIER.to_string(),
                        value: read_setting(
                            app,
                            keys::ENABLE_EMOTION_CLASSIFIER,
                            &app_defaults.enable_emotion_classifier.to_string(),
                        ),
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
                description: "在这里设定你想要的永久记忆效果".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::USE_PERSISTENT_MEMORY.to_string(),
                        value: read_setting(
                            app,
                            keys::USE_PERSISTENT_MEMORY,
                            &app_defaults.use_persistent_memory.to_string(),
                        ),
                        description:
                            "USE_PERSISTENT_MEMORY — 开启后记忆会自动压缩，减少 token 消耗"
                                .to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::MEMORY_UPDATE_INTERVAL.to_string(),
                        value: read_setting(
                            app,
                            keys::MEMORY_UPDATE_INTERVAL,
                            &app_defaults.memory_update_interval.to_string(),
                        ),
                        description: "MEMORY_UPDATE_INTERVAL — 触发记忆摘要的新消息数（默认 250）"
                            .to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::MEMORY_RECENT_WINDOW.to_string(),
                        value: read_setting(
                            app,
                            keys::MEMORY_RECENT_WINDOW,
                            &app_defaults.memory_recent_window.to_string(),
                        ),
                        description: "MEMORY_RECENT_WINDOW — 摘要时保留的最近消息数（默认 30）"
                            .to_string(),
                        setting_type: "text".to_string(),
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
            "适配器 URL".to_string(),
            Subcategory {
                description: "各个 TTS 后端的 API 地址，对应原环境变量 SIMPLE_VITS_API_URL / STYLE_BERT_VITS2_URL 等".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::SIMPLE_VITS_API_URL.to_string(),
                        value: read_setting(app, keys::SIMPLE_VITS_API_URL, &tts_defaults.simple_vits_api_url),
                        description: "Simple-Vits-API 地址（VITS 适配器）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::BV2_API_URL.to_string(),
                        value: read_setting(app, keys::BV2_API_URL, &tts_defaults.bv2_api_url),
                        description: "Simple-Vits-API 地址（Bert-Vits2 适配器）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::GSV_API_URL.to_string(),
                        value: read_setting(app, keys::GSV_API_URL, &tts_defaults.gsv_api_url),
                        description: "GPT-SoVITS API 地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::SBV2_API_URL.to_string(),
                        value: read_setting(app, keys::SBV2_API_URL, &tts_defaults.sbv2_api_url),
                        description: "Style-Bert-Vits2 本地服务地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::SBV2API_API_URL.to_string(),
                        value: read_setting(app, keys::SBV2API_API_URL, &tts_defaults.sbv2api_api_url),
                        description: "SBV2 API 服务地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::AIVIS_API_URL.to_string(),
                        value: read_setting(app, keys::AIVIS_API_URL, &tts_defaults.aivis_api_url),
                        description: "AIVIS 云 API 地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::AIVIS_API_KEY.to_string(),
                        value: read_setting(app, keys::AIVIS_API_KEY, ""),
                        description: "AIVIS API 密钥（原环境变量 AIVIS_API_KRY）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::INDEXTTS_API_URL.to_string(),
                        value: read_setting(app, keys::INDEXTTS_API_URL, &tts_defaults.indextts_api_url),
                        description: "IndexTTS2 API 地址".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::OPENTTS_API_URL.to_string(),
                        value: read_setting(app, keys::OPENTTS_API_URL, &tts_defaults.opentts_api_url),
                        description: "OpenTTS API 地址（硅基流动）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::OPENTTS_API_KEY.to_string(),
                        value: read_setting(app, keys::OPENTTS_API_KEY, ""),
                        description: "OpenTTS API 密钥".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::OPENTTS_MODEL.to_string(),
                        value: read_setting(app, keys::OPENTTS_MODEL, &tts_defaults.opentts_model),
                        description: "OpenTTS 模型名称".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::OPENTTS_VOICE.to_string(),
                        value: read_setting(app, keys::OPENTTS_VOICE, &tts_defaults.opentts_voice),
                        description: "OpenTTS voice / 音色标识".to_string(),
                        setting_type: "text".to_string(),
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
                settings: vec![ConfigSetting {
                    key: keys::TTS_AUDIO_FORMAT.to_string(),
                    value: read_setting(app, keys::TTS_AUDIO_FORMAT, &tts_defaults.audio_format),
                    description: "音频文件格式（wav / mp3 / flac / ogg 等）".to_string(),
                    setting_type: "text".to_string(),
                }],
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

    // ===== 日志配置 =====
    // 注意：日志默认值不在 AppConfig 中，由 lib.rs 直接读取，因此此处使用字面量。
    {
        let mut log_subs = BTreeMap::new();

        log_subs.insert(
            "基础设置".to_string(),
            Subcategory {
                description: "程序运行时文件日志的相关设置".to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::LOG_ENABLE.to_string(),
                        value: read_setting(app, keys::LOG_ENABLE, "true"),
                        description:
                            "LOG_ENABLE — 是否将运行日志写入文件（位于 data/log/app/ 目录）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LOG_RETENTION_DAYS.to_string(),
                        value: read_setting(app, keys::LOG_RETENTION_DAYS, "10"),
                        description:
                            "LOG_RETENTION_DAYS — 日志文件保留天数，超过的旧文件在启动时自动清理"
                                .to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::LOG_LLM_REQUEST_BODY.to_string(),
                        value: read_setting(app, keys::LOG_LLM_REQUEST_BODY, "false"),
                        description:
                            "LOG_LLM_REQUEST_BODY — 记录每次 LLM 请求的完整请求体 JSON 到 data/log/llm/ 目录（默认关闭）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                    },
                ],
            },
        );

        tree.insert(
            "日志配置".to_string(),
            Category {
                subcategories: log_subs,
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
                        key: keys::ENABLE_PROACTIVE_SYSTEM.to_string(),
                        value: read_setting(
                            app,
                            keys::ENABLE_PROACTIVE_SYSTEM,
                            &proactive_defaults.enable_proactive_system.to_string(),
                        ),
                        description: "ENABLE_PROACTIVE_SYSTEM — 是否启用主动对话系统".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::MAX_PROACTIVE_TIMES.to_string(),
                        value: read_setting(
                            app,
                            keys::MAX_PROACTIVE_TIMES,
                            &proactive_defaults.max_proactive_times.to_string(),
                        ),
                        description: "MAX_PROACTIVE_TIMES — 在用户响应之前，能主动对话的次数"
                            .to_string(),
                        setting_type: "text".to_string(),
                    },
                ],
            },
        );

        // 视觉感知配置（视觉模型本身在「高级设置 → 大模型管理」中以角色形式配置）
        proactive_subs.insert(
            "视觉感知设置".to_string(),
            Subcategory {
                description: "主动对话时的桌面视觉感知开关与触发权重，视觉模型在大模型管理中配置"
                    .to_string(),
                settings: vec![
                    ConfigSetting {
                        key: keys::ENABLE_VISUAL_PRECEPTION.to_string(),
                        value: read_setting(
                            app,
                            keys::ENABLE_VISUAL_PRECEPTION,
                            &proactive_defaults.enable_visual_perception.to_string(),
                        ),
                        description:
                            "ENABLE_VISUAL_PRECEPTION — 是否允许主动视觉感知桌面画面（偷看屏幕）"
                                .to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::SCREEN_WEIGHT.to_string(),
                        value: read_setting(
                            app,
                            keys::SCREEN_WEIGHT,
                            &proactive_defaults.screen_weight.to_string(),
                        ),
                        description:
                            "SCREEN_WEIGHT — 视觉模式触发权重（越大越容易偷看屏幕聊天，默认 30）"
                                .to_string(),
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
                        key: keys::ENABLE_TOPIC_CREATER.to_string(),
                        value: read_setting(
                            app,
                            keys::ENABLE_TOPIC_CREATER,
                            &proactive_defaults.enable_topic_creator.to_string(),
                        ),
                        description: "ENABLE_TOPIC_CREATER — 允许自主寻找并开启新话题".to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::TOPIC_WEIGHT.to_string(),
                        value: read_setting(
                            app,
                            keys::TOPIC_WEIGHT,
                            &proactive_defaults.topic_weight.to_string(),
                        ),
                        description: "TOPIC_WEIGHT — 随机话题触发权重（默认 60）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::ENABLE_TODO_PRECEPTION.to_string(),
                        value: read_setting(
                            app,
                            keys::ENABLE_TODO_PRECEPTION,
                            &proactive_defaults.enable_todo_perception.to_string(),
                        ),
                        description:
                            "ENABLE_TODO_PRECEPTION — 允许在闲暇时自动读取未完成 TODO 并温和提醒"
                                .to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::TODO_WEIGHT.to_string(),
                        value: read_setting(
                            app,
                            keys::TODO_WEIGHT,
                            &proactive_defaults.todo_weight.to_string(),
                        ),
                        description: "TODO_WEIGHT — TODO 提醒触发权重（默认 10）".to_string(),
                        setting_type: "text".to_string(),
                    },
                    ConfigSetting {
                        key: keys::ENABLE_SCHEDULE_REMINDER.to_string(),
                        value: read_setting(
                            app,
                            keys::ENABLE_SCHEDULE_REMINDER,
                            &proactive_defaults.enable_schedule_reminder.to_string(),
                        ),
                        description: "ENABLE_SCHEDULE_REMINDER — 启用强日程日程报时弹窗提醒"
                            .to_string(),
                        setting_type: "bool".to_string(),
                    },
                    ConfigSetting {
                        key: keys::ENABLE_IMPORTANT_DAY_REMINDER.to_string(),
                        value: read_setting(
                            app,
                            keys::ENABLE_IMPORTANT_DAY_REMINDER,
                            &proactive_defaults.enable_important_day_reminder.to_string(),
                        ),
                        description:
                            "ENABLE_IMPORTANT_DAY_REMINDER — 启用重要节日与特殊日子暖心提醒"
                                .to_string(),
                        setting_type: "bool".to_string(),
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
