//! Chapter — wraps a chapter YAML config and runs its events sequentially.
//!
//! Replaces Python `Chapter` class.

use anyhow::Result;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::ScriptContext;
use crate::ai_service::game_system::script_engine::events_handler::EventsHandler;
use crate::ai_service::game_system::script_engine::responses::{
    ChapterChangePayload, WatchJumpPayload, event_names::SCRIPT_CHAPTER_CHANGE,
    event_names::SCRIPT_WATCH_JUMP,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::ScriptStatus;

/// A chapter loaded from a chapter YAML file.
pub struct Chapter {
    /// Chapter identifier (the YAML file path relative to the script).
    pub _chapter_id: String,
    /// Display name from the chapter config.
    pub chapter_name: String,
    /// Sequential event processor for this chapter.
    pub events_handler: EventsHandler,
}

impl Chapter {
    /// Construct a `Chapter` from a chapter config dict and script status.
    pub fn new(chapter_id: String, chapter_config: Value, _script_status: &ScriptStatus) -> Self {
        let chapter_name = chapter_config
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&chapter_id)
            .to_string();

        let event_list = chapter_config
            .get("events")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Self {
            _chapter_id: chapter_id,
            chapter_name,
            events_handler: EventsHandler::new(event_list),
        }
    }

    /// Run all events in this chapter.
    /// Returns the name of the next chapter to load.
    pub async fn run(&mut self, ctx: &mut ScriptContext<'_>) -> Result<String> {
        // Emit chapter_change event to frontend
        let payload = ChapterChangePayload {
            chapter_name: self.chapter_name.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_CHAPTER_CHANGE, &payload);

        tracing::info!(
            "[ScriptEngine] 开始章节: '{}' ({} events)",
            self.chapter_name,
            self.events_handler.event_list.len()
        );

        // Execute events one by one
        while !self.events_handler.is_finished() {
            // watch_file 优先：目标 .chr 消失时立刻让位给崩坏章节（DDLC Act3 的
            // 实时 monika.chr 检查），不等当前章节播完。
            let pending = ctx.channels.lock().await.watch_jump.take();
            if let Some(target) = pending {
                tracing::info!(
                    "[ScriptEngine] 文件监视触发：章节 '{}' 中断 → 跳转 '{}'",
                    self.chapter_name,
                    target
                );
                // 前端可能还堆着被中断章节的积压事件：先清队列再开崩坏章，
                // 否则玩家要点完旧台词才能看到崩坏，实时性就丢了。
                let _ = emit(
                    ctx.app,
                    SCRIPT_WATCH_JUMP,
                    &WatchJumpPayload {
                        target: target.clone(),
                    },
                );
                return Ok(target);
            }
            match self.events_handler.process_next_event(ctx).await {
                Ok(()) => {},
                Err(error) => {
                    // 阻塞中的事件被监视器丢弃通道后会报错让位；同样优先响应跳转
                    let pending = ctx.channels.lock().await.watch_jump.take();
                    if let Some(target) = pending {
                        tracing::info!(
                            "[ScriptEngine] 文件监视触发（事件中）：章节 '{}' 中断 → 跳转 '{}'",
                            self.chapter_name,
                            target
                        );
                        let _ = emit(
                            ctx.app,
                            SCRIPT_WATCH_JUMP,
                            &WatchJumpPayload {
                                target: target.clone(),
                            },
                        );
                        return Ok(target);
                    }
                    return Err(error);
                },
            }
        }

        let result = self.events_handler.get_chapter_result();
        tracing::info!(
            "[ScriptEngine] 章节 '{}' 结束 → 下一章节: '{}'",
            self.chapter_name,
            result
        );

        Ok(result)
    }
}
