use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::ai_service::llm::provider_config::resolve_chat_provider;

pub mod keys {
    pub const ENABLE_PROACTIVE_SYSTEM: &str = "ENABLE_PROACTIVE_SYSTEM";
    pub const MAX_PROACTIVE_TIMES: &str = "MAX_PROACTIVE_TIMES";
    pub const VD_FOLLOW_CHAT_MODEL: &str = "VD_FOLLOW_CHAT_MODEL";
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
}

#[derive(Clone, Debug)]
pub struct ProactiveConfig {
    pub enable_proactive_system: bool,
    pub max_proactive_times: i32,
    pub vd_api_key: String,
    pub vd_base_url: String,
    pub vd_model: String,
    pub enable_visual_perception: bool,
    pub screen_weight: f64,
    pub enable_topic_creator: bool,
    pub topic_weight: f64,
    pub enable_todo_perception: bool,
    pub todo_weight: f64,
    pub enable_schedule_reminder: bool,
    pub enable_important_day_reminder: bool,
}

impl ProactiveConfig {
    pub fn load(app: &AppHandle) -> Self {
        let store = app.store(super::STORE_FILE).ok();

        let get_bool = |key: &str, default: bool| -> bool {
            store
                .as_ref()
                .and_then(|s| s.get(key))
                .and_then(|v| match v {
                    Value::Bool(b) => Some(b),
                    Value::String(s) => Some(s == "true"),
                    _ => None,
                })
                .unwrap_or(default)
        };

        let get_string = |key: &str, default: &str| -> String {
            store
                .as_ref()
                .and_then(|s| s.get(key))
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| default.to_string())
        };

        let get_i32 = |key: &str, default: i32| -> i32 {
            store
                .as_ref()
                .and_then(|s| s.get(key))
                .and_then(|v| match v {
                    Value::Number(n) => n.as_i64().map(|x| x as i32),
                    Value::String(s) => s.parse::<i32>().ok(),
                    _ => None,
                })
                .unwrap_or(default)
        };

        let get_f64 = |key: &str, default: f64| -> f64 {
            store
                .as_ref()
                .and_then(|s| s.get(key))
                .and_then(|v| match v {
                    Value::Number(n) => n.as_f64(),
                    Value::String(s) => s.parse::<f64>().ok(),
                    _ => None,
                })
                .unwrap_or(default)
        };

        let mut vd_api_key = get_string(keys::VD_API_KEY, "");
        let mut vd_base_url = get_string(
            keys::VD_BASE_URL,
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        );
        let mut vd_model = get_string(keys::VD_MODEL, "qwen3.5-plus");

        if get_bool(keys::VD_FOLLOW_CHAT_MODEL, false) {
            if let Some(provider) = resolve_chat_provider(app) {
                if provider.provider != "openai"
                    && provider.provider != "lmstudio"
                    && provider.provider != "kimicode"
                {
                    tracing::warn!(
                        "[ProactiveConfig] Chat provider '{}' may not support the OpenAI-compatible vision API",
                        provider.provider
                    );
                }
                tracing::info!(
                    "[ProactiveConfig] Visual model follows chat provider: {} ({})",
                    provider.label,
                    provider.model
                );
                vd_api_key = provider.api_key;
                vd_base_url = match provider.provider.as_str() {
                    "openai" if provider.base_url.trim().is_empty() => {
                        "https://api.openai.com/v1".to_string()
                    }
                    "kimicode" => {
                        let base_url = provider.base_url.trim().trim_end_matches('/');
                        if base_url.is_empty() {
                            "https://api.kimi.com/coding/v1".to_string()
                        } else if base_url.ends_with("/v1/messages") {
                            base_url.trim_end_matches("/messages").to_string()
                        } else if base_url.ends_with("/v1/chat/completions") {
                            base_url.trim_end_matches("/chat/completions").to_string()
                        } else if base_url.ends_with("/v1") {
                            base_url.to_string()
                        } else {
                            format!("{base_url}/v1")
                        }
                    }
                    _ => provider.base_url,
                };
                vd_model = provider.model;
            } else {
                tracing::warn!(
                    "[ProactiveConfig] Visual model is set to follow chat, but no usable chat provider was found; using the dedicated visual configuration"
                );
            }
        }

        Self {
            enable_proactive_system: get_bool(keys::ENABLE_PROACTIVE_SYSTEM, false),
            max_proactive_times: get_i32(keys::MAX_PROACTIVE_TIMES, 3),
            vd_api_key,
            vd_base_url,
            vd_model,
            enable_visual_perception: get_bool(keys::ENABLE_VISUAL_PRECEPTION, true),
            screen_weight: get_f64(keys::SCREEN_WEIGHT, 30.0),
            enable_topic_creator: get_bool(keys::ENABLE_TOPIC_CREATER, true),
            topic_weight: get_f64(keys::TOPIC_WEIGHT, 60.0),
            enable_todo_perception: get_bool(keys::ENABLE_TODO_PRECEPTION, true),
            todo_weight: get_f64(keys::TODO_WEIGHT, 10.0),
            enable_schedule_reminder: get_bool(keys::ENABLE_SCHEDULE_REMINDER, true),
            enable_important_day_reminder: get_bool(keys::ENABLE_IMPORTANT_DAY_REMINDER, true),
        }
    }
}
