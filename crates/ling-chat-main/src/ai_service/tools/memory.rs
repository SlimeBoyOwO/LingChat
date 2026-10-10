use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use tauri::Manager;

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::ToolDefinition;
use crate::api::character::read_character_settings;
use crate::api::data_dir;
use crate::db::managers::role_repo::RoleRepo;

use ling_chat_notes::NotesStore;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{ensure_no_args, game_status_handle};

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
pub struct GetNotes;

#[async_trait::async_trait]
impl Tool for GetNotes {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_get_notes",
            "获取手动记忆笔记（含 id、内容、标签）。默认读取当前角色的笔记；可传 role 指定读取其他角色的笔记（只读，不能写入）",
            json!({
                "type": "object",
                "properties": {
                    "role": {"type": "string", "description": "要读取其笔记的角色名；不传时读取当前角色"}
                },
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
        let obj = require_object(&arguments, "memory_get_notes")?;
        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let db = state.db.clone();
        let store = notes_store();

        let notes = if let Some(role_name) = obj.get("role").and_then(Value::as_str) {
            let dn = resolve_display_name(&db, role_name).await?;
            store
                .load(&dn)
                .map_err(|e| ToolError::Execution(e.to_string()))?
        } else {
            let gs = game_status_handle(&app).await;
            let mut gs = gs.lock().await;
            let dn = current_display_name(&mut gs, &db).await?;
            store
                .load(&dn)
                .map_err(|e| ToolError::Execution(e.to_string()))?
        };
        Ok(json!({ "ok": true, "notes": notes }))
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
