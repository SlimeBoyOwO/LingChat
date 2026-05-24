//! Chapter — wraps a chapter YAML config and runs its events sequentially.
//!
//! Replaces Python `Chapter` class.

use anyhow::Result;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::ScriptContext;
use crate::ai_service::game_system::script_engine::events_handler::EventsHandler;
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_CHAPTER_CHANGE, ChapterChangePayload,
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
            self.events_handler.process_next_event(ctx).await?;
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
