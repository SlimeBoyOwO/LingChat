use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub mod keys {
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
}

#[derive(Clone, Debug)]
pub struct ProactiveConfig {
    pub enable_proactive_system: bool,
    pub max_proactive_times: i32,
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

        Self {
            enable_proactive_system: get_bool(keys::ENABLE_PROACTIVE_SYSTEM, false),
            max_proactive_times: get_i32(keys::MAX_PROACTIVE_TIMES, 3),
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
