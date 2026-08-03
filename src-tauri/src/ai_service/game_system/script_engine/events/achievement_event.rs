//! 解锁成就事件 —— 剧本流程中给玩家发放成就。
//!
//! 作者必须填 `achievement_id`（成就键名）+ `title`（成就标题）+ `description`
//! （成就描述）：每次执行都按这三项**动态注册**一个成就再解锁——不依赖系统里
//! 是否已存在同名成就，作者可以完全自建。已解锁的成就不会重复广播。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use tauri::{Emitter, Manager};

use crate::achievements::types::AchievementDef;
use crate::AppState;
use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};

pub struct UnlockAchievementEvent {
    achievement_id: String,
    title: String,
    description: String,
    duration: Option<f64>,
}

impl UnlockAchievementEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            achievement_id: data
                .get("achievement_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            title: data.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for UnlockAchievementEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let id = self.achievement_id.trim();
        if id.is_empty() {
            return Err(anyhow!("成就事件缺少 achievement_id（成就键名）"));
        }
        let title = self.title.trim();
        let description = self.description.trim();
        if title.is_empty() || description.is_empty() {
            return Err(anyhow!("成就事件必须填写标题和描述，才能创建这个成就"));
        }

        // 试玩是临时会话，不污染玩家的真实成就存档
        if ctx.is_preview {
            return Ok(None);
        }

        let state = ctx.app.state::<AppState>();
        let unlocked = {
            let mut mgr = state.achievement_manager.lock().await;
            // 动态注册：成就键名全应用唯一，重名会覆盖之前的定义
            mgr.register_achievement(
                id.to_string(),
                AchievementDef {
                    title: title.to_string(),
                    description: description.to_string(),
                    ach_type: "adventure".into(),
                    target_progress: 1,
                    hidden: false,
                    img_url: None,
                    audio_url: None,
                    duration: None,
                },
            );
            mgr.unlock(id)
        };
        if let Some(achievement) = unlocked {
            ctx.app
                .emit("achievement:unlocked", &achievement)
                .map_err(|e| anyhow!("发送成就事件失败: {}", e))?;
        }
        Ok(None)
    }

    fn event_type() -> &'static str {
        "unlock_achievement"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(UnlockAchievementEvent::event_type(), |data| {
        Box::new(UnlockAchievementEvent::from_event_data(&data))
    });
}
