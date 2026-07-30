//! Chapter end event — determines the next chapter.
//!
//! Three sub-types:
//! - `linear`: returns the `next_chapter` / `next` field directly
//! - `branching`: evaluates conditions on `script_status.vars` to choose a branch
//! - `ai_judged`: calls LLM to decide among named options (stub — needs LLM integration)

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    evaluate_condition, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::utils::script_function::match_ai_response_options;
use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::LlmMessage;

pub struct ChapterEndEvent {
    end_type: String,
    next: Option<String>,
    next_chapter: Option<String>,
    options: Vec<Value>,
    prompt: Option<String>,
}

impl ChapterEndEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            end_type: data
                .get("end_type")
                .and_then(|v| v.as_str())
                .unwrap_or("linear")
                .to_string(),
            next: data
                .get("next")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            next_chapter: data
                .get("next_chapter")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            options: data
                .get("options")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            prompt: data
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl ScriptEvent for ChapterEndEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // Verify script_status exists (brief lock)
        {
            let gs = ctx.game_status.lock().await;
            if gs.script_status.is_none() {
                return Err(anyhow!("ScriptStatus 未设置"));
            }
        }

        let next = match self.end_type.as_str() {
            "linear" => self
                .next
                .clone()
                .or_else(|| self.next_chapter.clone())
                .unwrap_or_else(|| "end".to_string()),
            "branching" => {
                let gs = ctx.game_status.lock().await;
                let script_status = gs.script_status.as_ref().unwrap(); // safe: checked above
                let mut result = "end".to_string();
                for opt in &self.options {
                    let condition = opt.get("condition").and_then(|v| v.as_str()).unwrap_or("");
                    if condition.is_empty() || evaluate_condition(condition, &script_status.vars) {
                        if let Some(next) = opt.get("next").and_then(|v| v.as_str()) {
                            result = next.to_string();
                            break;
                        }
                    }
                }
                // Check for default option
                if result == "end" {
                    for opt in &self.options {
                        if opt
                            .get("default")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            if let Some(next) = opt.get("next").and_then(|v| v.as_str()) {
                                result = next.to_string();
                            }
                        }
                    }
                }
                result
            }
            "ai_judged" => {
                // Try LLM-based judgment
                let llm = ctx.llm.cloned();
                if let Some(llm) = llm {
                    self.call_llm_for_judgment(&llm, ctx).await?
                } else {
                    tracing::warn!("[ChapterEndEvent] ai_judged 需要 LLM 但未配置，默认 end");
                    "end".to_string()
                }
            }
            _ => {
                tracing::warn!(
                    "[ChapterEndEvent] 未知的 end_type: '{}'，默认 end",
                    self.end_type
                );
                "end".to_string()
            }
        };

        tracing::info!(
            "[ChapterEndEvent] end_type={} → next: '{}'",
            self.end_type,
            next
        );
        Ok(Some(next))
    }

    fn event_type() -> &'static str {
        "chapter_end"
    }
}

impl ChapterEndEvent {
    /// Call LLM to judge the next chapter based on a prompt and named options.
    async fn call_llm_for_judgment(
        &self,
        llm: &Arc<LlmClient>,
        ctx: &mut ScriptContext<'_>,
    ) -> Result<String> {
        // Collect option names for the prompt
        let option_names: Vec<&str> = self
            .options
            .iter()
            .filter_map(|opt| opt.get("name").and_then(|v| v.as_str()))
            .collect();

        // Build conversation context from current role's memory
        let conv_text = {
            let mut gs = ctx.game_status.lock().await;
            gs.refresh_memories(ctx.db).await?;
            let rid = gs.current_role_id.or(gs.main_role_id).unwrap_or(0);
            if rid != 0 {
                if let Ok(role) = gs.get_role(ctx.db, rid).await {
                    let memory = role.memory.clone();
                    memory
                        .iter()
                        .filter(|m| m.role != "system")
                        .map(|m| format!("{}: {}", m.role, m.content))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        };

        let prompt_text = self
            .prompt
            .clone()
            .unwrap_or_else(|| "根据对话内容选择最合适的下一章节".to_string());

        let full_prompt = format!(
            "{}\n\n【对话记录】:\n{}\n\n【可选章节】:\n{}\n\n请只回复章节名称本身，不要包含其他内容。",
            prompt_text,
            if conv_text.is_empty() {
                "（无对话记录）"
            } else {
                &conv_text
            },
            option_names
                .iter()
                .enumerate()
                .map(|(i, name)| format!("{}. {}", i + 1, name))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        tracing::info!("[ChapterEndEvent] 请求 LLM 判断下一章节...");
        let messages = vec![LlmMessage::user(full_prompt)];
        let response = llm.complete(&messages).await?;
        let response = response.trim().to_string();
        tracing::info!("[ChapterEndEvent] LLM 判断结果: '{}'", response);

        // Match response against option names (substring match)
        if let Some(next) = match_ai_response_options(&response, &self.options) {
            return Ok(next);
        }

        // Fallback: first option's next
        if let Some(first) = self.options.first() {
            if let Some(next) = first.get("next").and_then(|v| v.as_str()) {
                return Ok(next.to_string());
            }
        }

        Ok("end".to_string())
    }
}

pub fn register() {
    register_event(ChapterEndEvent::event_type(), |data| {
        Box::new(ChapterEndEvent::from_event_data(&data))
    });
}
