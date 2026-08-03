//! 章节结束事件 —— 决定下一章。
//!
//! 三种子类型：
//! - `linear`：直接返回 `next_chapter` / `next` 字段
//! - `branching`：对 `script_status.vars` 求值条件来选择分支
//! - `ai_judged`：调用 LLM 在命名选项中决定（需 LLM）

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    evaluate_condition, parse_duration, register_event, ScriptContext, ScriptEvent,
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
    duration: Option<f64>,
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
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for ChapterEndEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 校验 script_status 存在（短暂持锁）
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
                // 检查 default 选项
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
                // AI 判断走向：必须调 LLM。LLM 不可用就终止剧本，不再回落到任何
                // 默认分支——那会让剧本以错误逻辑继续跑（上游复核明确要求）。
                let llm = ctx.llm.clone().ok_or_else(|| {
                    anyhow!(
                        "「AI 判断」章节结束需要大模型判断走向，但 LLM 不可用，剧本终止。请先配置并选择模型。"
                    )
                })?;
                self.call_llm_for_judgment(&llm, ctx).await?
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

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

impl ChapterEndEvent {
    /// 调用 LLM，依据 prompt 与命名选项判断下一章。
    async fn call_llm_for_judgment(
        &self,
        llm: &Arc<LlmClient>,
        ctx: &mut ScriptContext<'_>,
    ) -> Result<String> {
        // 收集选项名供 prompt 使用
        let option_names: Vec<&str> = self
            .options
            .iter()
            .filter_map(|opt| opt.get("name").and_then(|v| v.as_str()))
            .collect();

        // 用当前角色的记忆构建对话上下文
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

        // 把回复与选项名匹配（子串匹配）
        if let Some(next) = match_ai_response_options(&response, &self.options) {
            return Ok(next);
        }

        // 兜底：取第一个选项的 next
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
