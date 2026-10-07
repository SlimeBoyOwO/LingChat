//! 向量记忆工具：`memory_store` / `vector_search` / `memory_delete`。
//!
//! 三个工具共用 `AppState::memory`（[`MemoryService`]），按 `current_role_id`
//! 隔离记忆。`features.memory_enabled` 关闭或嵌入配置未就绪时直接返回错误，
//! 不让模型误以为记忆已写入。

use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{Value, json};
use tauri::Manager;

use crate::AppState;
use crate::ai_service::memory::{MemoryService, SearchMode};
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::game_status_handle;

/// 写记忆：远端嵌入 + LLM 去重判定，可能较慢。
const STORE_TIMEOUT: Duration = Duration::from_secs(60);
/// 检索：一次嵌入请求。
const SEARCH_TIMEOUT: Duration = Duration::from_secs(30);
/// 删除：语义删除需要嵌入请求。
const DELETE_TIMEOUT: Duration = Duration::from_secs(30);

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
            "嵌入模型未配置，请在设置 → 嵌入模型 中填写接口地址与模型名".into(),
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

/// 解析 `YYYY-MM-DD` 起止日期为 Unix 毫秒区间（含端点整天）。
fn parse_date_range(
    start: Option<&str>,
    end: Option<&str>,
) -> Result<Option<(i64, i64)>, ToolError> {
    let day_start = |raw: &str| -> Result<i64, ToolError> {
        let date = NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
            .map_err(|_| ToolError::InvalidArguments(format!("日期 {raw} 需要 YYYY-MM-DD 格式")))?;
        Ok(date
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().timestamp_millis())
            .unwrap_or(0))
    };
    let day_end = |raw: &str| -> Result<i64, ToolError> {
        let date = NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
            .map_err(|_| ToolError::InvalidArguments(format!("日期 {raw} 需要 YYYY-MM-DD 格式")))?;
        // 次日零点前一毫秒 = 当天 23:59:59.999
        Ok(date
            .succ_opt()
            .and_then(|next| next.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc().timestamp_millis() - 1)
            .unwrap_or(i64::MAX))
    };
    let start_ms = start.map(day_start).transpose()?;
    let end_ms = end.map(day_end).transpose()?;
    match (start_ms, end_ms) {
        (None, None) => Ok(None),
        (start, end) => {
            let start = start.unwrap_or(0);
            let end = end.unwrap_or(i64::MAX);
            if end < start {
                return Err(ToolError::InvalidArguments(
                    "end_date 早于 start_date".into(),
                ));
            }
            Ok(Some((start, end)))
        },
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
        Ok(json!({
            "ok": true,
            "action": outcome.action,
            "memory_id": outcome.memory_id,
            "detail": outcome.detail,
        }))
    }
}

// ─── vector_search ───

/// vector_search：语义 + 关键词混合检索该角色的长期记忆。
pub struct VectorSearch;

#[async_trait]
impl Tool for VectorSearch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "vector_search",
            "检索当前角色关于用户的长期记忆（语义 + 关键词混合，带时间过滤）",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "要找的信息，用自然语言描述"},
                    "mode": {"type": "string", "enum": ["hybrid", "vector"], "description": "检索方式，hybrid=语义+关键词（默认），vector=纯语义"},
                    "top_k": {"type": "integer", "description": "最多返回条数，默认取系统配置"},
                    "min_score": {"type": "number", "description": "最低相似度阈值 0-1，默认取系统配置"},
                    "start_date": {"type": "string", "description": "仅返回该日期（YYYY-MM-DD）之后的记忆"},
                    "end_date": {"type": "string", "description": "仅返回该日期（YYYY-MM-DD）之前的记忆"}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(SEARCH_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "vector_search")?;
        let query = parse_string(obj, "query", "vector_search", true)?.unwrap_or_default();
        let mode = SearchMode::parse(obj.get("mode").and_then(Value::as_str));
        let top_k = match obj.get("top_k") {
            Some(value) => Some(value.as_u64().ok_or_else(|| {
                ToolError::InvalidArguments("vector_search 的 top_k 必须是整数".into())
            })? as usize),
            None => None,
        };
        let min_score = match obj.get("min_score") {
            Some(value) => Some(value.as_f64().ok_or_else(|| {
                ToolError::InvalidArguments("vector_search 的 min_score 必须是数字".into())
            })?),
            None => None,
        };
        let time_range = parse_date_range(
            obj.get("start_date").and_then(Value::as_str),
            obj.get("end_date").and_then(Value::as_str),
        )?;

        let app = context.require_app()?;
        let role_id = current_role_id(context).await?;
        let memory = require_memory(&app)?;
        let hits = memory
            .search(role_id, &query, mode, top_k, min_score, time_range)
            .await
            .map_err(|e| ToolError::Execution(format!("记忆检索失败: {e}")))?;

        let results: Vec<Value> = hits
            .into_iter()
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
        Ok(json!({
            "ok": true,
            "pipeline": "hybrid_search",
            "count": results.len(),
            "results": results,
        }))
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
                    "memory_id": {"type": "string", "description": "要删除的记忆 ID（来自 vector_search/memory_store）"},
                    "query": {"type": "string", "description": "用自然语言描述要删的内容，会语义匹配并删除相似记忆"}
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
