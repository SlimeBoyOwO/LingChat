use chrono::Timelike;

use crate::achievements::manager::AchievementManager;
use crate::achievements::types::Achievement;

/// 成就触发器：根据用户消息判断是否触发成就进度。
pub struct AchievementTriggerHandler;

impl AchievementTriggerHandler {
    pub fn handle_user_message(
        message: &str,
        manager: &mut AchievementManager,
    ) -> Vec<Achievement> {
        let mut unlocked = Vec::new();
        if message.is_empty() {
            return unlocked;
        }

        // 系统/番茄钟消息以 '{' 开头
        let is_system_message = message.trim().starts_with('{');

        if is_system_message {
            // 系统消息只检查番茄钟
            if let Some(a) = Self::check_first_pomodoro(message, manager) {
                unlocked.push(a);
            }
        } else {
            // 普通聊天消息：增加聊天进度
            unlocked.extend(Self::update_chat_progress(manager));
            // 检查夜猫子
            if let Some(a) = Self::check_night_owl(manager) {
                unlocked.push(a);
            }
        }

        unlocked
    }

    fn update_chat_progress(manager: &mut AchievementManager) -> Vec<Achievement> {
        let mut unlocked = Vec::new();
        if let Some(a) = manager.increment_progress("first_chat", 1) {
            unlocked.push(a);
        }
        if let Some(a) = manager.increment_progress("chat_master", 1) {
            unlocked.push(a);
        }
        unlocked
    }

    fn check_first_pomodoro(
        message: &str,
        manager: &mut AchievementManager,
    ) -> Option<Achievement> {
        if message.contains("我启动了番茄钟") {
            return manager.increment_progress("first_pomodoro", 1);
        }
        None
    }

    fn check_night_owl(manager: &mut AchievementManager) -> Option<Achievement> {
        let now = chrono::Local::now();
        let hour = now.hour();
        if hour >= 23 || hour < 4 {
            return manager.increment_progress("night_owl", 1);
        }
        None
    }
}
