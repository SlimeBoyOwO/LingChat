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

/// 主动对话调度所需的最小只读上下文。
///
/// 这里刻意不持有 `GameStatus` 的锁，避免屏幕分析的网络请求阻塞正常聊天。
#[derive(Clone, Debug, Default)]
pub struct ProactiveContext {
    pub user_name: String,
    pub ai_name: String,
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

// ==========================================
// 主动对话意图暂存（"小本本"）
// ==========================================

use std::time::Instant;

/// 意图类型，带 TTL 和投放优先级。
/// `Ord` 派生顺序 = 投放优先级（高值优先投放）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntentType {
    Topic = 0,        // 闲聊 — 最低优先级
    Screen = 1,       // 屏幕感知 — 时效短（2min TTL）
    Todo = 2,         // 待办提醒
    ImportantDay = 3, // 重要日子
    Alarm = 4,        // 日程闹钟 — 最高优先级，长 TTL（不应过期）
}

impl IntentType {
    /// 意图存活时间（秒）。超时自动作废，不再投放。
    pub fn ttl_secs(self) -> u64 {
        match self {
            Self::Topic => 900,
            Self::Screen => 120,
            Self::Todo => 600,
            Self::ImportantDay => 600,
            Self::Alarm => 1800, // 30 分钟，确保不会被轻易丢弃
        }
    }
}

/// 暂存的主动对话意图。prompt 已完整生成，不可变。
#[derive(Clone, Debug)]
pub struct PendingIntent {
    /// 已格式化的系统旁白（PromptRole::System.build_prompt 的结果）
    pub prompt: String,
    /// 去重使用的原始提示词。只有真正投递成功后才写入主动对话历史。
    pub raw_prompt: Option<String>,
    pub intent_type: IntentType,
    /// 创建策略时的用户交互代次；用于丢弃用户发言前启动的过期屏幕/闲聊结果。
    pub interaction_epoch: u64,
    /// 已进入生成阶段但未产出任何持久化回复的次数。
    pub delivery_attempts: u8,
    /// 生成时间，用于 TTL 过期判断
    pub triggered_at: Instant,
}

impl PendingIntent {
    pub fn new(
        prompt: String,
        raw_prompt: Option<String>,
        intent_type: IntentType,
        interaction_epoch: u64,
    ) -> Self {
        let now = Instant::now();
        Self {
            prompt,
            raw_prompt,
            intent_type,
            interaction_epoch,
            delivery_attempts: 0,
            triggered_at: now,
        }
    }
}

/// 调度器生成的候选意图。prompt 仍是未经过 PromptRole 包装的原始文本。
#[derive(Clone, Debug)]
pub struct ProactiveCandidate {
    pub prompt: String,
    pub intent_type: IntentType,
}

/// 一轮策略选择的结果，同时告诉主循环本轮是否真的调用过视觉模型。
#[derive(Clone, Debug, Default)]
pub struct DispatchOutcome {
    pub candidate: Option<ProactiveCandidate>,
    pub screen_attempted: bool,
}
