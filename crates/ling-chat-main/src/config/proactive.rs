//! 主动对话系统配置，从 settings.json 统一加载。

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use super::keys;

/// 两次主动对话之间的期望间隔默认值（分钟）。默认 5 分钟沿用历史上写死的
/// 每轮增长 5.0~10.0 的手感。
pub const DEFAULT_EXPECTED_INTERVAL_MINUTES: f64 = 5.0;
/// 期望间隔的合法区间（分钟）。低于 1 分钟会退化成每轮都搭话，
/// 高于 1 小时则增长量小到需要网格精度兜底，故两端都设界。
pub const MIN_EXPECTED_INTERVAL_MINUTES: f64 = 1.0;
pub const MAX_EXPECTED_INTERVAL_MINUTES: f64 = 60.0;

// ========== ProactiveConfig 结构体 ==========

#[derive(Clone, Debug)]
pub struct ProactiveConfig {
    pub enable_proactive_system: bool,
    pub max_proactive_times: i32,
    /// 两次主动对话之间的期望间隔（分钟），兴趣增长随机范围由它反推。
    pub expected_interval_minutes: f64,
    pub enable_visual_perception: bool,
    pub screen_weight: f64,
    pub enable_topic_creator: bool,
    pub topic_weight: f64,
    pub enable_todo_perception: bool,
    pub todo_weight: f64,
    pub enable_schedule_reminder: bool,
    pub enable_important_day_reminder: bool,
    /// 日程到点提醒是否无视主动对话总开关。
    pub force_schedule_reminder: bool,
    /// 待办截止提醒是否无视主动对话总开关。
    pub force_todo_reminder: bool,
    /// 待办截止提醒方式："toast" = 系统通知，其余值 = 角色提醒。
    pub todo_reminder_style: String,
}

// ========== Default 实现（单一真相源） ==========

impl Default for ProactiveConfig {
    fn default() -> Self {
        Self {
            enable_proactive_system: false,
            max_proactive_times: 3,
            expected_interval_minutes: DEFAULT_EXPECTED_INTERVAL_MINUTES,
            enable_visual_perception: true,
            screen_weight: 30.0,
            enable_topic_creator: true,
            topic_weight: 60.0,
            enable_todo_perception: true,
            todo_weight: 10.0,
            enable_schedule_reminder: true,
            enable_important_day_reminder: true,
            force_schedule_reminder: true,
            force_todo_reminder: true,
            todo_reminder_style: "role".to_string(),
        }
    }
}

// ========== ProactiveConfig 方法 ==========

impl ProactiveConfig {
    /// 从 settings.json 加载主动对话配置，缺失项回退到 `Self::default()`。
    pub fn load(app: &AppHandle) -> Self {
        let store = app.store(super::STORE_FILE).ok();
        let default = Self::default();

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

        let get_string = |key: &str, default: &str| -> String {
            store
                .as_ref()
                .and_then(|s| s.get(key))
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| default.to_string())
        };

        Self {
            enable_proactive_system: get_bool(
                keys::ENABLE_PROACTIVE_SYSTEM,
                default.enable_proactive_system,
            ),
            max_proactive_times: get_i32(keys::MAX_PROACTIVE_TIMES, default.max_proactive_times),
            expected_interval_minutes: get_f64(
                keys::PROACTIVE_EXPECTED_INTERVAL_MINUTES,
                default.expected_interval_minutes,
            ),
            enable_visual_perception: get_bool(
                keys::ENABLE_VISUAL_PRECEPTION,
                default.enable_visual_perception,
            ),
            screen_weight: get_f64(keys::SCREEN_WEIGHT, default.screen_weight),
            enable_topic_creator: get_bool(
                keys::ENABLE_TOPIC_CREATER,
                default.enable_topic_creator,
            ),
            topic_weight: get_f64(keys::TOPIC_WEIGHT, default.topic_weight),
            enable_todo_perception: get_bool(
                keys::ENABLE_TODO_PRECEPTION,
                default.enable_todo_perception,
            ),
            todo_weight: get_f64(keys::TODO_WEIGHT, default.todo_weight),
            enable_schedule_reminder: get_bool(
                keys::ENABLE_SCHEDULE_REMINDER,
                default.enable_schedule_reminder,
            ),
            enable_important_day_reminder: get_bool(
                keys::ENABLE_IMPORTANT_DAY_REMINDER,
                default.enable_important_day_reminder,
            ),
            force_schedule_reminder: get_bool(
                keys::FORCE_SCHEDULE_REMINDER,
                default.force_schedule_reminder,
            ),
            force_todo_reminder: get_bool(keys::FORCE_TODO_REMINDER, default.force_todo_reminder),
            todo_reminder_style: get_string(
                keys::TODO_REMINDER_STYLE,
                &default.todo_reminder_style,
            ),
        }
    }
}
