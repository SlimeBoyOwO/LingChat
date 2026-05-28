use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 用户当前的状态分类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserState {
    IDLE,
    BROWSING,
    WORK,
    GAME,
    CASUAL,
}

impl UserState {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserState::IDLE => "IDLE",
            UserState::BROWSING => "BROWSING",
            UserState::WORK => "WORK",
            UserState::GAME => "GAME",
            UserState::CASUAL => "CASUAL",
        }
    }
}

/// 系统感知外部环境/用户行为后的汇总结果。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerceptionResult {
    pub state: UserState,
    pub description: String,
    pub interest_modifier: i32,
    pub visual_change_detected: bool,
    pub current_screen_text: String,
}

// ==========================================
// 日程与待办配置结构 (schedules.json 映射)
// ==========================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleItem {
    pub name: String,
    pub time: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleGroup {
    pub title: String,
    pub description: String,
    pub items: Vec<ScheduleItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: i64,
    pub text: String,
    pub priority: i32,
    pub completed: bool,
    pub deadline: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TodoGroup {
    pub title: String,
    pub description: Option<String>,
    pub todos: Vec<TodoItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportantDay {
    pub id: String,
    pub date: String,
    pub title: String,
    pub desc: Option<String>,
    pub cycle: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserScheduleSettings {
    pub schedule_groups: Option<HashMap<String, ScheduleGroup>>,
    pub todo_groups: Option<HashMap<String, TodoGroup>>,
    pub important_days: Option<Vec<ImportantDay>>,
}
