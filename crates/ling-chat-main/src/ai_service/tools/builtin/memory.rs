use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use tauri::Manager;

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::ToolDefinition;
use crate::api::character::read_character_settings;
use crate::api::data_dir;
use crate::db::managers::role_repo::RoleRepo;

use ling_chat_notes::Note;
use ling_chat_notes::NotesStore;
use ling_chat_notes::embedding::Embedder;
use ling_chat_notes::retrieval::{DEFAULT_TOP_K, MAX_TOP_K, NotesVectorIndex};

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::{ensure_no_args, game_status_handle};

// ─── 角色名解析（工具层）：把外部角色名映射到笔记库使用的权威展示名 ───

/// 取当前对话角色的权威展示名（display_name），与权限系统使用同一个名字来源。
async fn current_display_name(
    gs: &mut GameStatus,
    db: &DatabaseConnection,
) -> Result<String, ToolError> {
    let Some(role_id) = gs.current_role_id else {
        return Err(ToolError::Execution("当前没有对话角色".into()));
    };
    let role = gs
        .get_role(db, role_id)
        .await
        .map_err(|e| ToolError::Execution(format!("获取当前角色失败: {e}")))?;
    role.display_name
        .clone()
        .ok_or_else(|| ToolError::Execution("当前角色没有展示名".into()))
}

/// 把外部传入的角色名解析为 LingChat 权威展示名（ai_name）。
///
/// 优先精确匹配 `ai_name`，其次匹配角色 DB 名 `role.name`。
/// 解析成功后才能定位到对应角色的笔记文件，保证"文件名对齐"。
async fn resolve_display_name(db: &DatabaseConnection, given: &str) -> Result<String, ToolError> {
    let given = given.trim();
    if given.is_empty() {
        return Err(ToolError::InvalidArguments("role 不能为空".into()));
    }
    let roles = RoleRepo::get_all_main_roles(db)
        .await
        .map_err(|e| ToolError::Execution(format!("查询角色列表失败: {e}")))?;
    for role in &roles {
        let dn =
            read_character_settings(role.resource_folder.as_deref().unwrap_or_default()).ai_name;
        if dn == given || dn.trim() == given {
            return Ok(dn);
        }
    }
    // 兜底：按 DB 角色名匹配，仍返回该角色的 ai_name
    for role in &roles {
        if role.name == given {
            return Ok(read_character_settings(
                role.resource_folder.as_deref().unwrap_or_default(),
            )
            .ai_name);
        }
    }
    Err(ToolError::Execution(format!("未找到角色: {given}")))
}

/// 取当前角色的权威名，供写操作定位笔记文件。返回后不持有锁。
async fn current_role_name_for_write(context: &ToolContext) -> Result<String, ToolError> {
    let app = context.require_app()?;
    let state = app.state::<AppState>();
    let db = state.db.clone();
    let gs = game_status_handle(&app).await;
    let mut gs = gs.lock().await;
    current_display_name(&mut gs, &db).await
}

fn notes_store() -> NotesStore {
    NotesStore::new(data_dir())
}

fn parse_tags(value: Option<&Value>, tool: &str) -> Result<Option<Vec<String>>, ToolError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let array = value
        .as_array()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 的 tags 必须是字符串数组")))?;
    let mut tags = Vec::with_capacity(array.len());
    for value in array {
        let tag = value.as_str().ok_or_else(|| {
            ToolError::InvalidArguments(format!("{tool} 的 tags 必须全部是字符串"))
        })?;
        tags.push(tag.to_string());
    }
    Ok(Some(tags))
}

fn require_object<'a>(
    arguments: &'a Value,
    tool: &str,
) -> Result<&'a serde_json::Map<String, Value>, ToolError> {
    arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 参数必须是 JSON object")))
}

// ─── 工具 ───

/// memory_get_current：获取当前角色的自动记忆库文本。
pub struct GetCurrentMemory;

#[async_trait::async_trait]
impl Tool for GetCurrentMemory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_get_current",
            "获取当前角色的自动记忆库文本（关于用户的信息、重要约定、长期经历）",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "你还记得关于我的什么",
            "你记着我哪些事",
            "说说你对我的印象",
            "我们之前聊过什么",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        ensure_no_args(&arguments, "memory_get_current").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let gs = game_status_handle(&app).await;
        let gs = gs.lock().await;
        let Some(role_id) = gs.current_role_id else {
            return Err(ToolError::Execution("当前没有对话角色".into()));
        };
        let memory = gs.role_manager.get_role_memory_text(role_id).await;
        Ok(json!({
            "ok": true,
            "role_id": role_id,
            "memory": memory,
        }))
    }
}

/// memory_get_notes：获取手动记忆笔记。
///
/// 默认读取**当前角色**的笔记；可传 `role` 指定读取**其他角色**的笔记（只读）。
/// 传 `query` 时按语义检索返回 Top-K，不传时保持全量返回。
pub struct GetNotes {
    index: Arc<NotesVectorIndex>,
    warned_disabled: AtomicBool,
}

impl GetNotes {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            index: Arc::new(NotesVectorIndex::new(data_dir)),
            warned_disabled: AtomicBool::new(false),
        }
    }

    fn full_result(notes: Vec<Note>) -> Value {
        json!({ "ok": true, "notes": notes })
    }
}

/// 非法或缺失的 top_k 一律回落到默认值，整数则限制在合法区间。
fn parse_top_k(value: Option<&Value>) -> usize {
    value
        .and_then(Value::as_i64)
        .map(|value| value.clamp(1, MAX_TOP_K as i64) as usize)
        .unwrap_or(DEFAULT_TOP_K)
}

/// 按检索给出的 ID 顺序挑选笔记，返回（结果，总数）。
fn select_notes(notes: Vec<Note>, note_ids: &[String]) -> (Vec<Note>, usize) {
    let total = notes.len();
    let by_id: HashMap<&str, &Note> = notes.iter().map(|note| (note.id.as_str(), note)).collect();
    let selected = note_ids
        .iter()
        .filter_map(|id| by_id.get(id.as_str()).map(|note| (*note).clone()))
        .collect();
    (selected, total)
}

#[async_trait::async_trait]
impl Tool for GetNotes {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_get_notes",
            "获取手动记忆笔记（含 id、内容、标签）。不传 query 时返回该角色的全部笔记；传 query 时按语义检索最相关的 top_k 条。默认读取当前角色的笔记；可传 role 指定读取其他角色的笔记（只读，不能写入）",
            json!({
                "type": "object",
                "properties": {
                    "role": {"type": "string", "description": "要读取其笔记的角色名；不传时读取当前角色"},
                    "query": {"type": "string", "description": "检索关键词或简短问句；不传则返回全部笔记。建议用陈述性关键词（如「食物过敏」「睡前约定」）"},
                    "top_k": {"type": "integer", "minimum": 1, "maximum": MAX_TOP_K, "description": format!("返回条数，默认 {DEFAULT_TOP_K}")}
                },
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "还记得我的生日是几号吗？",
            "我想看看你记得我的事情",
            "看看你给我记的笔记",
            "有没有关于睡前的记录",
        ])
    }

    fn timeout_hint(&self) -> Option<Duration> {
        // 首次检索要为整份笔记建索引；缓存命中后只有一次查询嵌入。
        Some(Duration::from_secs(15))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_get_notes")?;
        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let db = state.db.clone();
        let store = notes_store();

        let role = if let Some(role_name) = obj.get("role").and_then(Value::as_str) {
            resolve_display_name(&db, role_name).await?
        } else {
            let gs = game_status_handle(&app).await;
            let mut gs = gs.lock().await;
            current_display_name(&mut gs, &db).await?
        };
        let notes = store
            .load(&role)
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let Some(query) = obj
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(str::to_string)
        else {
            return Ok(Self::full_result(notes));
        };
        if notes.is_empty() {
            return Ok(Self::full_result(notes));
        }

        let engine = state.embedding.clone();
        if !engine.is_enabled() {
            if !self.warned_disabled.swap(true, Ordering::Relaxed) {
                tracing::warn!("文本嵌入后端未启用，memory_get_notes 回退为全量返回");
            }
            return Ok(Self::full_result(notes));
        }

        let top_k = parse_top_k(obj.get("top_k"));
        let index = self.index.clone();
        let notes_for_search = notes.clone();
        let query_for_search = query.clone();
        let search = tokio::task::spawn_blocking(move || {
            index.search(
                engine.as_ref(),
                &role,
                &notes_for_search,
                &query_for_search,
                top_k,
            )
        })
        .await;

        let note_ids = match search {
            Ok(Ok(ids)) => ids,
            Ok(Err(e)) => {
                tracing::warn!("笔记语义检索失败，回退为全量返回: {e}");
                return Ok(Self::full_result(notes));
            },
            Err(e) => {
                tracing::warn!("笔记语义检索任务失败，回退为全量返回: {e}");
                return Ok(Self::full_result(notes));
            },
        };

        let (selected, total) = select_notes(notes, &note_ids);
        let returned = selected.len();
        Ok(json!({
            "ok": true,
            "notes": selected,
            "total": total,
            "returned": returned,
            "retrieved": true,
        }))
    }
}

/// memory_add_note：向当前角色添加一条手动记忆笔记。
pub struct AddNote;

#[async_trait::async_trait]
impl Tool for AddNote {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_add_note",
            "向当前角色添加一条手动记忆笔记，可附带标签（仅能写入当前角色的笔记）",
            json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "笔记内容"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "标签，可选"}
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "记住我喜欢吃草莓",
            "要记住我的生日哦",
            "别忘了我不吃香菜",
            "把这个记到小本本上",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_add_note")?;
        let content = obj
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("memory_add_note 需要 content".into()))?;
        if content.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "memory_add_note 的 content 不能为空".into(),
            ));
        }
        let tags = parse_tags(obj.get("tags"), "memory_add_note")?.unwrap_or_default();

        let role_name = current_role_name_for_write(context).await?;
        let id = notes_store()
            .add(&role_name, content, tags)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(json!({"ok": true, "id": id}))
    }
}

/// memory_update_note：更新当前角色的手动记忆笔记。
pub struct UpdateNote;

#[async_trait::async_trait]
impl Tool for UpdateNote {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_update_note",
            "按 ID 更新当前角色的手动记忆笔记的内容或标签，至少提供一项（仅能修改当前角色的笔记）",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "笔记 ID"},
                    "content": {"type": "string", "description": "新的笔记内容，可选"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "新的标签，可选"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "把那条笔记改一下",
            "更正一下你记的内容",
            "我搬家了，更新一下地址",
            "那件事记得不对，改改",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_update_note")?;
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("memory_update_note 需要 id".into()))?;
        let has_content = obj.get("content").is_some();
        let has_tags = obj.get("tags").is_some();
        if !has_content && !has_tags {
            return Err(ToolError::InvalidArguments(
                "memory_update_note 至少需要 content/tags 中的一项".into(),
            ));
        }
        let content = match obj.get("content") {
            Some(value) => {
                let content = value.as_str().ok_or_else(|| {
                    ToolError::InvalidArguments("memory_update_note 的 content 必须是字符串".into())
                })?;
                if content.trim().is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "memory_update_note 的 content 不能为空".into(),
                    ));
                }
                Some(content.to_string())
            },
            None => None,
        };
        let tags = parse_tags(obj.get("tags"), "memory_update_note")?;

        let role_name = current_role_name_for_write(context).await?;
        notes_store()
            .update(&role_name, &id, content, tags)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(json!({"ok": true, "id": id}))
    }
}

/// memory_delete_note：删除当前角色的手动记忆笔记。
pub struct DeleteNote;

#[async_trait::async_trait]
impl Tool for DeleteNote {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_delete_note",
            "按 ID 删除当前角色的手动记忆笔记（仅能删除当前角色的笔记）",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "笔记 ID"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "把那条笔记删了",
            "这件事不用记了",
            "忘掉你记的那个",
            "删掉关于我的那条记录",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_delete_note")?;
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("memory_delete_note 需要 id".into()))?;

        let role_name = current_role_name_for_write(context).await?;
        notes_store()
            .delete(&role_name, &id)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(json!({"ok": true, "id": id}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str) -> Note {
        Note {
            id: id.to_string(),
            content: format!("内容 {id}"),
            tags: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn argument_parsing_and_result_order() {
        assert_eq!(parse_top_k(None), DEFAULT_TOP_K);
        assert_eq!(parse_top_k(Some(&json!("8"))), DEFAULT_TOP_K);
        assert_eq!(parse_top_k(Some(&json!(0))), 1);
        assert_eq!(parse_top_k(Some(&json!(30))), MAX_TOP_K);
        assert_eq!(parse_top_k(Some(&json!(5))), 5);

        let notes = vec![note("a"), note("b"), note("c")];
        let ids = vec!["c".to_string(), "a".to_string(), "不存在".to_string()];
        let (selected, total) = select_notes(notes, &ids);
        assert_eq!(total, 3);
        let selected_ids: Vec<&str> = selected.iter().map(|note| note.id.as_str()).collect();
        assert_eq!(selected_ids, vec!["c", "a"]);
    }
}
