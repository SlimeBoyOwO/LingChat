//! Sequential event processor for a chapter.
//!
//! Replaces Python `EventsHandler` — iterates through a YAML event list,
//! dispatches each event to the registered handler, and collects chapter-end results.

use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::script_engine::events::{ScriptContext, create_event};
use crate::ai_service::game_system::script_engine::responses::{
    ScriptProgressPayload, event_names::SCRIPT_PROGRESS,
};
use crate::ai_service::game_system::script_engine::utils::script_function::replace_placeholder;
use crate::ai_service::message_system::events::emit;

/// Processes a chapter's event list sequentially.
pub struct EventsHandler {
    /// Current event index within the chapter.
    pub progress: usize,
    /// Raw event dicts from chapter YAML.
    pub event_list: Vec<Value>,
    /// Set when a chapter_end event returns a result (the next chapter name).
    pub chapter_result: Option<String>,
    /// 当前章节的标识（YAML 路径相对剧本目录），随阅读锚点广播给前端，
    /// 使前端存的恢复点能定位到章节而非仅事件下标。
    pub chapter_key: String,
}

impl EventsHandler {
    pub fn new(event_list: Vec<Value>, chapter_key: String) -> Self {
        Self {
            progress: 0,
            event_list,
            chapter_result: None,
            chapter_key,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.chapter_result.is_some() || self.progress >= self.event_list.len()
    }

    /// Returns the next chapter name (or `"end"` if no result was set).
    pub fn get_chapter_result(&self) -> String {
        self.chapter_result
            .clone()
            .unwrap_or_else(|| "end".to_string())
    }

    /// Advance to the next event and execute it.
    /// If the event returns `Some(next_chapter)`, stores it as chapter_result.
    pub async fn process_next_event(&mut self, ctx: &mut ScriptContext<'_>) -> Result<()> {
        if self.is_finished() {
            return Ok(());
        }

        let event_data = self.event_list[self.progress].clone();
        self.progress += 1;

        let event_index = self.progress - 1;
        // 把「即将执行的事件下标」同步进 script_status，并向前端广播阅读锚点。
        // 引擎在阅读型事件上不停顿、一路跑到下一个阻塞事件（选项/输入/自由对话），
        // 锚点随事件流进入前端队列、按阅读速度被消费——玩家读到哪，锚点就跟到哪，
        // 存档时据此记录精确的恢复点（章节 + 事件下标 + 当时台词条数 + 当时剧本变量）。
        let (line_count, vars) = {
            let mut gs = ctx.game_status.lock().await;
            if let Some(ref mut ss) = gs.script_status {
                ss.current_event_process = event_index as i32;
            }
            let vars = gs
                .script_status
                .as_ref()
                .map(|s| s.vars.clone())
                .unwrap_or_default();
            (gs.line_list.len(), vars)
        };
        let _ = emit(
            ctx.app,
            SCRIPT_PROGRESS,
            &ScriptProgressPayload {
                chapter: self.chapter_key.clone(),
                event_index: event_index as i32,
                line_count: line_count as i32,
                vars,
            },
        );

        let event_type = event_data
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("事件缺少 'type' 字段，索引: {}", self.progress - 1))?
            .to_string();

        // Resolve placeholders in event data before dispatching
        let event_data = resolve_placeholders(event_data, &*ctx.game_status.lock().await);

        // Check condition
        if !check_condition(&event_data, &*ctx.game_status.lock().await) {
            tracing::info!(
                "[ScriptEngine] 跳过事件 type='{}'（条件不满足），索引: {}",
                event_type,
                self.progress - 1
            );
            return Ok(());
        }

        let mut handler = create_event(&event_type, event_data)
            .ok_or_else(|| anyhow!("未注册的事件类型: '{}'", event_type))?;

        if let Some(result) = handler.execute(ctx).await? {
            self.chapter_result = Some(result);
        }

        Ok(())
    }
}

/// Pre-process event data: replace `%player%` in text fields.
fn resolve_placeholders(mut event_data: Value, game_status: &GameStatus) -> Value {
    if let Value::Object(ref mut map) = event_data {
        let text_fields = [
            "text",
            "prompt",
            "hint",
            "end_line",
            "dialog_prompt",
            "end_prompt",
            "content",
            "description",
        ];
        for field in &text_fields {
            if let Some(Value::String(s)) = map.get_mut(*field) {
                *s = replace_placeholder(s, game_status);
            }
        }
    }
    event_data
}

/// Evaluate the optional `condition` field on an event dict.
fn check_condition(event_data: &Value, game_status: &GameStatus) -> bool {
    let condition = event_data
        .get("condition")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if condition.is_empty() {
        return true;
    }
    // Use script variables if available, otherwise global variables
    let vars = if let Some(ref ss) = game_status.script_status {
        &ss.vars
    } else {
        return true; // no script → no condition filtering
    };
    crate::ai_service::game_system::script_engine::events::evaluate_condition(condition, vars)
}
