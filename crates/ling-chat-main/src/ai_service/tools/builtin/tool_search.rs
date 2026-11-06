//! `tool_search`：Tool RAG 的检索入口。
//!
//! 做成真实 [`Tool`] 而不是在工具闭环里内联拦截，是为了自动复用执行器的参数校验、
//! 超时、统一错误编码，以及前端「正在调用工具」的活动事件；闭环本身不必再多一条
//! 特殊分支。

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tauri::Manager;

use crate::AppState;
use crate::ai_service::types::ToolDefinition;

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError};
use crate::ai_service::tools::tool_rag::{SEARCH_TOP_K, TOOL_SEARCH_NAME};

pub struct ToolSearch;

#[async_trait]
impl Tool for ToolSearch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_SEARCH_NAME,
            "按需检索可用的工具。当你想为用户做某件事（切换角色或场景、读写文件、查时间、记笔记、\
             换装、搜索网页等），而随本请求附带的工具里没有合适的一个时，用一句自然语言描述你想做的事\
             调用本工具，它会返回最匹配的工具；拿到结果后再调用那些工具完成任务。\
             若返回的工具为空，说明当前没有能完成这件事的工具，请如实告诉用户你做不到，\
             不要编造能力，也不要假装已经完成。",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "用一句自然语言描述你想完成的事，例如「给角色换一身衣服」"
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        )
    }

    /// 首次调用需要建 ONNX 会话并跑推理，执行器默认的 2 秒不够。
    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(8))
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<Value, ToolError> {
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if query.is_empty() {
            return Err(ToolError::InvalidArguments(
                "缺少 query 参数：请用一句自然语言描述你想完成的事".into(),
            ));
        }

        let app = context.require_app()?;
        // 句柄与候选集都在 await 之前取出，避免跨 await 持有 AppState 借用。
        let (rag, candidates) = {
            let state = app.state::<AppState>();
            (
                state.tool_rag.clone(),
                state
                    .tool_registry
                    .definitions_for_allowed(&context.base_allowed),
            )
        };

        let hits = rag
            .search_and_activate(
                context.role_name.as_deref(),
                &query,
                &candidates,
                SEARCH_TOP_K,
                // 检索是模型主动要求的：哪怕分数都不高也要给回候选，让它自己判断。
                f32::NEG_INFINITY,
            )
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        if hits.is_empty() {
            return Ok(json!({
                "ok": true,
                "tools": [],
                "note": "没有检索到合适的工具。请如实告诉用户你做不到，不要编造能力。",
            }));
        }

        let tools: Vec<Value> = hits
            .iter()
            .filter_map(|(name, _)| {
                candidates
                    .iter()
                    .find(|def| &def.function.name == name)
                    .map(|def| {
                        json!({
                            "name": def.function.name,
                            "description": def.function.description,
                        })
                    })
            })
            .collect();

        tracing::info!(
            count = tools.len(),
            query = query.as_str(),
            "tool_search 命中工具"
        );

        Ok(json!({
            "ok": true,
            "tools": tools,
            "note": "这些工具已加入本轮可用工具，请直接调用它们完成任务。",
        }))
    }
}
