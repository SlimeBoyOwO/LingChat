//! 无头工具智能体运行器：非流式、无前端呈现的一轮「prompt → 工具调用 → 执行 → 输出」。
//!
//! 与 [`super::tool_loop`] 并列但目标不同：`tool_loop` 服务聊天，带呈现栅栏、翻译/TTS
//! 与台词表回填；本模块服务不需要把过程展示给用户的后台决策型 Agent（目前是上帝
//! Agent），因此不产出任何前端事件，也不引入流式聊天的续写提示词。
//!
//! 工具定义一律从 [`ToolRegistry`] 按名取，执行一律走 [`ToolExecutor`]，从而复用
//! 统一的 schema 校验、超时与错误编码。

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::Value;
use tauri::AppHandle;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::LlmMessage;

use super::executor::{ToolContext, ToolExecutor};
use super::registry::ToolRegistry;

/// 无头工具智能体的一次运行输入（领域无关）。
pub struct AgentTask {
    /// 任务说明（system 消息）。
    pub system_prompt: String,
    /// 数据载荷（user 消息）。
    pub payload: String,
    /// 本任务允许调用的工具名；定义从注册表按名取。
    pub tool_names: Vec<String>,
    /// 最大决策轮数（含首轮）。1 表示单发决策，执行结果不再回填给 LLM。
    pub max_rounds: usize,
}

/// 单次工具调用记录。
pub struct AgentCall {
    pub name: String,
    pub arguments: String,
    /// 执行器原始返回：成功 `{"ok":true,..}`，失败 `{"ok":false,"error":{..}}`。
    pub result: String,
    pub ok: bool,
}

impl AgentCall {
    /// 取执行器错误编码里的可读消息。失败结果形如
    /// `{"ok":false,"error":{"code","message"}}`；取不到时回落为原始结果串。
    pub fn error_message(&self) -> String {
        serde_json::from_str::<Value>(&self.result)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| self.result.clone())
    }

    /// 工具是否明确报告了「本次无事可做」（而非失败）。
    /// 例如好感度工具的 `role_id` 不在场时会返回 `{"ok":false,"skipped":true}`。
    pub fn skipped(&self) -> bool {
        !self.ok
            && serde_json::from_str::<Value>(&self.result)
                .ok()
                .and_then(|value| value.get("skipped").and_then(Value::as_bool))
                .unwrap_or(false)
    }
}

/// 无头运行产物。
pub struct AgentOutput {
    /// LLM 正文；只调用工具而不产出文本时为空串。
    pub text: String,
    pub calls: Vec<AgentCall>,
}

/// 无头运行一次工具智能体：不流式、不呈现、不翻译/TTS、不写台词表。
pub async fn run_tool_agent(
    llm: &Arc<LlmClient>,
    registry: &ToolRegistry,
    task: &AgentTask,
    app: Option<AppHandle>,
) -> Result<AgentOutput> {
    let allowed: HashSet<String> = task.tool_names.iter().cloned().collect();
    let definitions = registry.definitions_for_allowed(&allowed);
    if definitions.is_empty() {
        return Err(anyhow!("工具集为空，无法运行工具智能体"));
    }

    let context = match app {
        Some(app) => ToolContext::new(allowed).with_app(app),
        None => ToolContext::new(allowed),
    };
    let executor = ToolExecutor::new(registry);

    let mut messages = vec![
        LlmMessage::system(task.system_prompt.clone()),
        LlmMessage::user(task.payload.clone()),
    ];
    let mut output = AgentOutput {
        text: String::new(),
        calls: Vec::new(),
    };
    let rounds = task.max_rounds.max(1);

    for round in 0..rounds {
        let response = llm
            .complete_with_tools(&messages, &definitions, Some("auto"))
            .await
            .map_err(|e| anyhow!("LLM 调用失败: {}", e))?;

        if let Some(text) = response.content {
            output.text = text;
        }
        let Some(calls) = response.tool_calls.filter(|calls| !calls.is_empty()) else {
            break;
        };

        // 末轮不再续跑，回填的 assistant / tool 消息没有消费者，跳过以免白造一份历史。
        let follow_up = round + 1 < rounds;
        if follow_up {
            messages.push(LlmMessage {
                role: "assistant".to_string(),
                content: output.text.clone(),
                tool_calls: Some(calls.clone()),
                tool_call_id: None,
                image_data_url: None,
            });
        }

        for call in calls {
            tracing::info!(tool = %call.function.name, "执行无头工具调用");
            let result = executor
                .execute(&call.function.name, &call.function.arguments, &context)
                .await;
            let ok = result_is_ok(&result);
            output.calls.push(AgentCall {
                name: call.function.name.clone(),
                arguments: call.function.arguments.clone(),
                result: result.clone(),
                ok,
            });
            if follow_up {
                messages.push(LlmMessage::tool_result(call.id, result));
            }
        }
    }

    Ok(output)
}

/// 判定工具结果成败。执行器把可恢复错误统一编码为 `{"ok": false, ...}`；
/// 工具自身的 `ok` 缺省视为成功。
fn result_is_ok(result: &str) -> bool {
    serde_json::from_str::<Value>(result)
        .ok()
        .and_then(|v| v.get("ok").and_then(Value::as_bool))
        .unwrap_or(true)
}
