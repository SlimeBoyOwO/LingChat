//! 向量记忆工具：`memory_store` / `memory_query` / `memory_delete` / `memory_summarize`。
//!
//! 四个工具共用 `AppState::memory`（[`MemoryService`]），按 `current_role_id`
//! 隔离记忆。`features.memory_enabled` 关闭或嵌入配置未就绪时直接返回错误，
//! 不让模型误以为记忆已写入。长期记忆的**自动**检索注入不在此列——它由生成管线
//! 在用户新消息轮自动完成（RAG 注入，见 `message_system::generator::inject_rag_memories`）；
//! `memory_query` 供模型按需主动检索（额外覆盖已带向量的「旧版整合」条目）。

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tauri::Manager;

use crate::AppState;
use crate::ai_service::memory::{
    DEFAULT_KEEP_RECENT, MemoryService, SearchMode, spawn_auto_summarize_if_needed,
};
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::game_status_handle;

/// 写记忆：远端嵌入 + LLM 去重判定，可能较慢。
const STORE_TIMEOUT: Duration = Duration::from_secs(60);
/// 查询：一次嵌入 + 混合检索。
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
/// 删除：语义删除需要嵌入请求。
const DELETE_TIMEOUT: Duration = Duration::from_secs(30);
/// 摘要：一次 LLM 生成 + 一次（或多次）嵌入写入，最慢。
const SUMMARIZE_TIMEOUT: Duration = Duration::from_secs(120);

/// 取当前对话角色 ID；无角色时报错。
async fn current_role_id(context: &ToolContext) -> Result<i32, ToolError> {
    let app = context.require_app()?;
    let gs = game_status_handle(&app).await;
    let gs = gs.lock().await;
    gs.current_role_id
        .ok_or_else(|| ToolError::Execution("当前没有对话角色".into()))
}

/// 取记忆服务；功能关闭 / 未配置时统一报错。
fn require_memory(app: &tauri::AppHandle) -> Result<std::sync::Arc<MemoryService>, ToolError> {
    let state = app.state::<AppState>();
    let memory = state.memory.clone();
    if !memory.enabled() {
        return Err(ToolError::Execution(
            "记忆功能已关闭（设置 → 记忆系统 → 启用记忆）".into(),
        ));
    }
    if !memory.is_configured() {
        return Err(ToolError::Execution(
            "嵌入模型未配置且未检测到内置模型：请在设置 → 嵌入模型 中填写接口地址与模型名，或确认内置模型文件完整".into(),
        ));
    }
    Ok(memory)
}

fn require_object<'a>(
    arguments: &'a Value,
    tool: &str,
) -> Result<&'a serde_json::Map<String, Value>, ToolError> {
    arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 参数必须是 JSON object")))
}

fn parse_string(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    tool: &str,
    required: bool,
) -> Result<Option<String>, ToolError> {
    match obj.get(key) {
        None => {
            if required {
                Err(ToolError::InvalidArguments(format!("{tool} 需要 {key}")))
            } else {
                Ok(None)
            }
        },
        Some(Value::String(value)) => {
            if value.trim().is_empty() && required {
                Err(ToolError::InvalidArguments(format!(
                    "{tool} 的 {key} 不能为空"
                )))
            } else {
                Ok(Some(value.clone()))
            }
        },
        Some(_) => Err(ToolError::InvalidArguments(format!(
            "{tool} 的 {key} 必须是字符串"
        ))),
    }
}

// ─── memory_store ───

/// memory_store：把一条关于用户的事实写入长期记忆。
pub struct MemoryStore;

#[async_trait]
impl Tool for MemoryStore {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_store",
            "把关于用户的事实、约定、经历写入长期记忆（自动去重，相似记忆会被合并）",
            json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "要记住的事实，一句话到一段话，包含足够上下文"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "分类标签，可选"}
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(STORE_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_store")?;
        let content = parse_string(obj, "content", "memory_store", true)?.unwrap_or_default();
        let tags: Vec<String> = match obj.get("tags") {
            None => Vec::new(),
            Some(Value::Array(items)) => {
                let mut tags = Vec::with_capacity(items.len());
                for item in items {
                    let tag = item.as_str().ok_or_else(|| {
                        ToolError::InvalidArguments("memory_store 的 tags 必须全部是字符串".into())
                    })?;
                    tags.push(tag.to_string());
                }
                tags
            },
            Some(_) => {
                return Err(ToolError::InvalidArguments(
                    "memory_store 的 tags 必须是字符串数组".into(),
                ));
            },
        };

        let app = context.require_app()?;
        let role_id = current_role_id(context).await?;
        let memory = require_memory(&app)?;
        let outcome = memory
            .store(role_id, &content, &tags)
            .await
            .map_err(|e| ToolError::Execution(format!("写入记忆失败: {e}")))?;
        // 写入后记忆规模越过阈值时，后台自动把较早的记忆压缩掉（不阻塞本轮）。
        spawn_auto_summarize_if_needed(memory.clone(), role_id);
        Ok(json!({
            "ok": true,
            "action": outcome.action,
            "memory_id": outcome.memory_id,
            "detail": outcome.detail,
        }))
    }
}

// ─── memory_query ───

/// memory_query：主动检索当前角色的记忆（长期记忆 + 已带向量的旧版整合条目）。
pub struct MemoryQuery;

#[async_trait]
impl Tool for MemoryQuery {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_query",
            "检索当前角色的记忆库：按语义 + 关键词混合检索长期记忆与已导入的旧版存档记忆，返回最相关的若干条。需要回忆用户偏好、过往约定、剧情旧事时调用",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "用自然语言描述要回忆什么"},
                    "top_k": {"type": "integer", "description": "最多返回条数，默认取系统配置（通常 5~8）"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(QUERY_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_query")?;
        let query = parse_string(obj, "query", "memory_query", true)?.unwrap_or_default();
        let top_k = match obj.get("top_k") {
            None => None,
            Some(Value::Number(value)) => value.as_u64().map(|v| v as usize),
            Some(_) => {
                return Err(ToolError::InvalidArguments(
                    "memory_query 的 top_k 必须是整数".into(),
                ));
            },
        };

        let app = context.require_app()?;
        let role_id = current_role_id(context).await?;
        let memory = require_memory(&app)?;
        let hits = memory
            .query(role_id, &query, SearchMode::Hybrid, top_k, None, None)
            .await
            .map_err(|e| ToolError::Execution(format!("查询记忆失败: {e}")))?;

        let items: Vec<Value> = hits
            .iter()
            .map(|hit| {
                json!({
                    "memory_id": hit.memory_id,
                    "content": hit.content,
                    "score": (hit.score * 1000.0).round() / 1000.0,
                    "source": hit.source,
                    "created_at": hit.created_at,
                })
            })
            .collect();
        Ok(json!({"ok": true, "count": items.len(), "items": items}))
    }
}

// ─── memory_delete ───

/// memory_delete：按 ID 或语义查询删除长期记忆。
pub struct MemoryDelete;

#[async_trait]
impl Tool for MemoryDelete {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_delete",
            "删除当前角色的长期记忆：优先按 memory_id 精确删除，否则按语义查询删除相似记忆",
            json!({
                "type": "object",
                "properties": {
                    "memory_id": {"type": "string", "description": "要删除的记忆 ID（来自 memory_store 或 memory_query 的返回；legacy_ 开头表示旧版整合条目）"},
                    "query": {"type": "string", "description": "用自然语言描述要删的内容，会语义匹配并删除相似记忆（仅长期记忆）"}
                },
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(DELETE_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_delete")?;
        let memory_id = parse_string(obj, "memory_id", "memory_delete", false)?;
        let query = parse_string(obj, "query", "memory_delete", false)?;
        if memory_id.is_none() && query.is_none() {
            return Err(ToolError::InvalidArguments(
                "memory_delete 需要 memory_id 或 query 至少一项".into(),
            ));
        }

        let app = context.require_app()?;
        let role_id = current_role_id(context).await?;
        let memory = require_memory(&app)?;

        if let Some(memory_id) = memory_id {
            let deleted = memory
                .delete_by_id(role_id, &memory_id)
                .await
                .map_err(|e| ToolError::Execution(format!("删除记忆失败: {e}")))?;
            if deleted == 0 {
                return Err(ToolError::Execution(format!("记忆 {memory_id} 不存在")));
            }
            return Ok(json!({"ok": true, "deleted": deleted, "memory_id": memory_id}));
        }

        let query = query.unwrap_or_default();
        let deleted = memory
            .delete_by_query(role_id, &query)
            .await
            .map_err(|e| ToolError::Execution(format!("删除记忆失败: {e}")))?;
        let items: Vec<Value> = deleted
            .into_iter()
            .map(|(content, score)| {
                json!({"content": content, "score": (score * 1000.0).round() / 1000.0})
            })
            .collect();
        Ok(json!({"ok": true, "deleted": items.len(), "items": items}))
    }
}

// ─── memory_summarize ───

/// memory_summarize：把较早的长期记忆压缩合并成一条摘要。
///
/// 记忆条数变多后检索质量会下降、去重误判率上升；该工具把「较早的一批」交给 LLM
/// 合成一条 `summary` 记忆再删掉源条目，最新的 `keep_recent` 条保持原样。
/// 写入后的自动压缩（阈值 [`AUTO_SUMMARIZE_THRESHOLD`](crate::ai_service::memory::AUTO_SUMMARIZE_THRESHOLD)）
/// 走的是同一条 `MemoryService::summarize` 链路。
pub struct MemorySummarize;

#[async_trait]
impl Tool for MemorySummarize {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_summarize",
            "整理长期记忆：把较早的一批记忆压缩合并成一条摘要（最新的若干条原样保留）。当记忆条数明显变多、或用户要求整理记忆时调用",
            json!({
                "type": "object",
                "properties": {
                    "keep_recent": {"type": "integer", "description": "保留原样的最新记忆条数，默认 20；0 表示全部参与压缩"}
                },
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(SUMMARIZE_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_summarize")?;
        let keep_recent = match obj.get("keep_recent") {
            None => DEFAULT_KEEP_RECENT,
            Some(value) => value.as_u64().ok_or_else(|| {
                ToolError::InvalidArguments("memory_summarize 的 keep_recent 必须是整数".into())
            })? as usize,
        };

        let app = context.require_app()?;
        let role_id = current_role_id(context).await?;
        let memory = require_memory(&app)?;
        let outcome = memory
            .summarize(role_id, keep_recent)
            .await
            .map_err(|e| ToolError::Execution(format!("记忆摘要失败: {e}")))?;
        Ok(json!({
            "ok": true,
            "action": outcome.action,
            "summarized": outcome.summarized,
            "memory_id": outcome.memory_id,
            "detail": outcome.detail,
        }))
    }
}
