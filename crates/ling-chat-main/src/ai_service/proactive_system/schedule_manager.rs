use crate::ai_service::proactive_system::types::UserScheduleSettings;
use chrono::Local;

pub struct ScheduleManager {
    last_triggered_key: String,
}

impl ScheduleManager {
    pub fn new() -> Self {
        Self {
            last_triggered_key: String::new(),
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
}
