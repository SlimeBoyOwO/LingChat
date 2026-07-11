use crate::ai_service::proactive_system::types::UserScheduleSettings;
use chrono::Local;
use std::collections::HashSet;

pub struct ScheduleManager {
    triggered_date: String,
    triggered_keys: HashSet<String>,
}

impl ScheduleManager {
    pub fn new() -> Self {
        Self {
            triggered_date: String::new(),
            triggered_keys: HashSet::new(),
        }
    }

    pub fn check_schedule_reminder(
        &mut self,
        user_name: &str,
        settings: &UserScheduleSettings,
    ) -> Option<String> {
        let now = Local::now();
        let current_date_str = now.format("%Y-%m-%d").to_string();
        let current_time_str = now.format("%H:%M").to_string();

        self.check_schedule_reminder_at(user_name, settings, &current_date_str, &current_time_str)
    }

    fn check_schedule_reminder_at(
        &mut self,
        user_name: &str,
        settings: &UserScheduleSettings,
        current_date: &str,
        current_time: &str,
    ) -> Option<String> {
        let schedule_groups = settings.schedule_groups.as_ref()?;

        if self.triggered_date != current_date {
            self.triggered_date.clear();
            self.triggered_date.push_str(current_date);
            self.triggered_keys.clear();
        }

        for (group_id, group) in schedule_groups {
            for (item_index, item) in group.items.iter().enumerate() {
                if item.time == current_time {
                    let trigger_key = format!("{group_id}:{item_index}:{}", item.time);
                    if self.triggered_keys.insert(trigger_key) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::proactive_system::types::{ScheduleGroup, ScheduleItem};
    use std::collections::HashMap;

    fn settings_with_two_due_items() -> UserScheduleSettings {
        UserScheduleSettings {
            schedule_groups: Some(HashMap::from([
                (
                    "work".to_string(),
                    ScheduleGroup {
                        title: "工作".to_string(),
                        items: vec![ScheduleItem {
                            name: "开会".to_string(),
                            time: "09:30".to_string(),
                            content: "周会".to_string(),
                        }],
                        ..Default::default()
                    },
                ),
                (
                    "health".to_string(),
                    ScheduleGroup {
                        title: "健康".to_string(),
                        items: vec![ScheduleItem {
                            name: "喝水".to_string(),
                            time: "09:30".to_string(),
                            content: "补充水分".to_string(),
                        }],
                        ..Default::default()
                    },
                ),
            ])),
            ..Default::default()
        }
    }

    #[test]
    fn each_due_item_only_triggers_once_per_day() {
        let settings = settings_with_two_due_items();
        let mut manager = ScheduleManager::new();

        let first = manager
            .check_schedule_reminder_at("测试用户", &settings, "2026-07-10", "09:30")
            .expect("first reminder should trigger");
        let second = manager
            .check_schedule_reminder_at("测试用户", &settings, "2026-07-10", "09:30")
            .expect("second reminder should trigger");

        assert_ne!(first, second);
        assert!(first.contains("开会") || first.contains("喝水"));
        assert!(second.contains("开会") || second.contains("喝水"));
        assert!(manager
            .check_schedule_reminder_at("测试用户", &settings, "2026-07-10", "09:30")
            .is_none());
    }

    #[test]
    fn reminders_reset_on_the_next_day() {
        let settings = settings_with_two_due_items();
        let mut manager = ScheduleManager::new();

        assert!(manager
            .check_schedule_reminder_at("测试用户", &settings, "2026-07-10", "09:30")
            .is_some());
        assert!(manager
            .check_schedule_reminder_at("测试用户", &settings, "2026-07-11", "09:30")
            .is_some());
    }
}
