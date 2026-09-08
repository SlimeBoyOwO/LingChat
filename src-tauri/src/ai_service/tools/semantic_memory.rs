//! 独立语义记忆工具（`semantic_mem_*`）：与普通记忆库、笔记完全解耦。
//!
//! - [`SemanticMemAdd`]：将一段内容编码为向量并写入当前角色的专属语义库（自动去重）。
//! - [`SemanticMemSearch`]：按语义相似度检索当前角色的语义记忆。
//! - [`SemanticMemDelete`]：按 id 删除一条语义记忆。
//! - [`SemanticMemList`]：列出当前角色的全部语义记忆。

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::ai_service::semantic_memory::{AddOutcome, SemanticMemory};
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{ensure_no_args, game_status_handle};

/// 取当前角色的 role_id 与语义记忆句柄（快速锁定后立即释放 GameStatus）。
async fn current_semantic_memory(
    app: &AppHandle,
) -> Result<(i32, Arc<SemanticMemory>), ToolError> {
    let gs = game_status_handle(app).await;
    let (role_id, sm) = {
        let guard = gs.lock().await;
        let Some(role_id) = guard.current_role_id else {
            return Err(ToolError::Execution("当前没有对话角色".into()));
        };
        let Some(sm) = guard.role_manager.semantic_memory().cloned() else {
            return Err(ToolError::Execution(
                "语义记忆未启用（请在「高级设置 → 语义记忆」中开启；需重启生效）".into(),
            ));
        };
        (role_id, sm)
    };
    Ok((role_id, sm))
}

fn require_object<'a>(
    arguments: &'a Value,
    tool: &str,
) -> Result<&'a serde_json::Map<String, Value>, ToolError> {
    arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 参数必须是 JSON object")))
}

fn parse_tags(value: Option<&Value>, tool: &str) -> Result<Vec<String>, ToolError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 的 tags 必须是字符串数组")))?;
    let mut tags = Vec::with_capacity(array.len());
    for value in array {
        let tag = value
            .as_str()
            .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 的 tags 必须全部是字符串")))?;
        tags.push(tag.to_string());
    }
    Ok(tags)
}

/// semantic_mem_add：向当前角色的独立语义库写入一条记忆。
pub struct SemanticMemAdd;

#[async_trait]
impl Tool for SemanticMemAdd {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "semantic_mem_add",
            "向当前角色的独立语义记忆库写入一条内容（自动嵌入向量并按语义去重）。与普通记忆库/笔记无关，用于保存需要长期记住的专有信息",
            json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "要记住的内容"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "标签，可选"}
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "semantic_mem_add")?;
        let content = obj
            .get("content")
            .and_then(Value::as_str)
            .map(str::trim)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("semantic_mem_add 需要 content".into()))?;
        if content.is_empty() {
            return Err(ToolError::InvalidArguments(
                "semantic_mem_add 的 content 不能为空".into(),
            ));
        }
        let tags = parse_tags(obj.get("tags"), "semantic_mem_add")?;

        let app = context.require_app()?;
        let (role_id, sm) = current_semantic_memory(&app).await?;
        match sm.add(role_id, &content, &tags).await {
            Ok(AddOutcome::Added(id)) => {
                Ok(json!({"ok": true, "result": "added", "id": id}))
            }
            Ok(AddOutcome::Duplicate) => Err(ToolError::Execution(
                "这条内容与已有语义记忆重复，未保存（如需更新请改写措辞再试）".into(),
            )),
            Err(e) => Err(ToolError::Execution(format!("保存语义记忆失败: {e}"))),
        }
    }
}

/// semantic_mem_search：按语义相似度检索当前角色的独立语义记忆。
pub struct SemanticMemSearch;

#[async_trait]
impl Tool for SemanticMemSearch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "semantic_mem_search",
            "按语义相似度检索当前角色的独立语义记忆库，返回最相关的记忆（用于精确回忆上次保存过的事实/事件）",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "要检索的内容或话题描述"},
                    "top_k": {"type": "integer", "description": "返回条数，默认 5，可选"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "semantic_mem_search")?;
        let query = obj
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("semantic_mem_search 需要 query".into()))?;
        if query.is_empty() {
            return Err(ToolError::InvalidArguments(
                "semantic_mem_search 的 query 不能为空".into(),
            ));
        }
        let top_k = obj
            .get("top_k")
            .and_then(Value::as_u64)
            .map(|n| n as usize);

        let app = context.require_app()?;
        let (role_id, sm) = current_semantic_memory(&app).await?;
        if !sm.embedding_ready() {
            return Err(ToolError::Execution(
                "语义记忆检索不可用：嵌入模型未就绪（请先启用「记忆嵌入」并确认模型目录正确）"
                    .into(),
            ));
        }
        let hits = sm.search(role_id, &query, top_k).await;
        let results: Vec<Value> = hits
            .iter()
            .map(|h| json!({"id": h.id, "text": h.text, "similarity": h.score}))
            .collect();
        Ok(json!({"query": query, "count": results.len(), "results": results}))
    }
}

/// semantic_mem_delete：删除当前角色的一条语义记忆。
pub struct SemanticMemDelete;

#[async_trait]
impl Tool for SemanticMemDelete {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "semantic_mem_delete",
            "按 id 删除当前角色的一条独立语义记忆（id 来自 semantic_mem_search / semantic_mem_list）",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "语义记忆 id"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "semantic_mem_delete")?;
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("semantic_mem_delete 需要 id".into()))?;

        let app = context.require_app()?;
        let (role_id, sm) = current_semantic_memory(&app).await?;
        match sm.delete(role_id, &id).await {
            Ok(true) => Ok(json!({"ok": true, "deleted": true, "id": id})),
            Ok(false) => Err(ToolError::Execution(format!("语义记忆 {id} 不存在"))),
            Err(e) => Err(ToolError::Execution(format!("删除语义记忆失败: {e}"))),
        }
    }
}

/// semantic_mem_list：列出当前角色的全部语义记忆。
pub struct SemanticMemList;

#[async_trait]
impl Tool for SemanticMemList {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "semantic_mem_list",
            "列出当前角色保存在独立语义记忆库中的全部内容（含 id、标签、保存时间）",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        ensure_no_args(&arguments, "semantic_mem_list").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let (role_id, sm) = current_semantic_memory(&app).await?;
        let items = sm.list(role_id).await.map_err(ToolError::Execution)?;
        let items: Vec<Value> = items
            .into_iter()
            .map(|item| {
                json!({
                    "id": item.id,
                    "text": item.text,
                    "tags": item.tags,
                    "created_at": item.created_at,
                })
            })
            .collect();
        Ok(json!({"count": items.len(), "items": items}))
    }
}