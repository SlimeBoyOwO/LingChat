use crate::ai_service::proactive_system::types::{TodoReminder, UserScheduleSettings};
use chrono::{Local, NaiveDateTime};
use std::collections::HashSet;

/// 截止时间的可接受格式。前端 datetime-local 产出第一种，其余兼容 AI 工具的自由输入。
const DEADLINE_FORMATS: [&str; 3] = ["%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M"];

pub struct ScheduleManager {
    last_triggered_key: String,
    /// 已触发过的待办提醒键，避免 30 秒轮询在同一窗口内反复提醒。
    triggered_todo_keys: HashSet<String>,
}

impl Default for ScheduleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduleManager {
    pub fn new() -> Self {
        Self {
            last_triggered_key: String::new(),
            triggered_todo_keys: HashSet::new(),
        }
    }

    pub fn check_schedule_reminder(
        &mut self,
        user_name: &str,
        settings: &UserScheduleSettings,
    ) -> Option<String> {
        let schedule_groups = settings.schedule_groups.as_ref()?;

        let now = Local::now();
        let current_time_str = now.format("%H:%M").to_string();

        for group in schedule_groups.values() {
            for item in &group.items {
                if item.time == current_time_str {
                    let trigger_key = format!("{}_{}", item.time, item.name);
                    if trigger_key != self.last_triggered_key {
                        self.last_triggered_key = trigger_key;

                        tracing::info!(
                            "[ScheduleManager] Triggered alarm for schedule: {}",
                            item.name
                        );
                        return Some(format!(
                            "{{你突然想起来 {} 设定的日程时间到了：{} ({})，提醒他一下吧？}}",
                            user_name, item.name, item.content
                        ));
                    }
                }
            }
        }

        None
    }

    /// 检查所有「已过提醒时刻、但尚未到截止时刻」的待办。
    /// 只有同时配了 deadline 与 remind_before_minutes 且未完成的条目参与。
    pub fn check_todo_reminder(&mut self, settings: &UserScheduleSettings) -> Option<TodoReminder> {
        let todo_groups = settings.todo_groups.as_ref()?;
        let now = Local::now().naive_local();

        // 先收集全部候选，用于清理已经改期/删除/完成的历史触发键
        let mut live_keys = HashSet::new();
        for group in todo_groups.values() {
            for todo in &group.todos {
                if todo.completed {
                    continue;
                }
                if let (Some(deadline), Some(minutes)) =
                    (todo.deadline.as_deref(), todo.remind_before_minutes)
                {
                    live_keys.insert(todo_trigger_key(todo.id, deadline, minutes));
                }
            }
        }
        self.triggered_todo_keys
            .retain(|key| live_keys.contains(key));

        for group in todo_groups.values() {
            for todo in &group.todos {
                if todo.completed {
                    continue;
                }
                let (Some(deadline), Some(minutes)) =
                    (todo.deadline.as_deref(), todo.remind_before_minutes)
                else {
                    continue;
                };
                let key = todo_trigger_key(todo.id, deadline, minutes);
                if self.triggered_todo_keys.contains(&key) {
                    continue;
                }
                let Some(deadline_at) = parse_deadline(deadline) else {
                    continue;
                };
                let remind_at = deadline_at - chrono::Duration::minutes(minutes as i64);
                // 命中窗口：[提醒时刻, 截止时刻)
                if now >= remind_at && now < deadline_at {
                    self.triggered_todo_keys.insert(key);
                    tracing::info!(
                        "[ScheduleManager] Triggered todo reminder: {} (截止 {})",
                        todo.text,
                        deadline
                    );
                    return Some(TodoReminder {
                        todo_text: todo.text.clone(),
                        minutes_before: minutes,
                        deadline: deadline.to_string(),
                    });
                }
            }
        }

        None
    }
}

fn todo_trigger_key(id: i64, deadline: &str, minutes: i32) -> String {
    format!("{id}_{deadline}_{minutes}")
}

/// 解析截止时间字符串。无法识别时返回 None，该条目静默跳过（AI 写入的是自由字符串）。
fn parse_deadline(raw: &str) -> Option<NaiveDateTime> {
    let raw = raw.trim();
    for format in DEADLINE_FORMATS {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(parsed);
        }
    }
    // 兜底：AI 可能带上时区偏移
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.naive_local())
}
