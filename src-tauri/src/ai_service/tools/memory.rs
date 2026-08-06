use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Manager;
use uuid::Uuid;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::ToolDefinition;
use crate::api::character::read_character_settings;
use crate::api::data_dir;
use crate::db::managers::role_repo::RoleRepo;
use crate::AppState;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{atomic_replace, ensure_no_args, game_status_handle};

// ─── 手动笔记：按角色独立存储 ───

/// 一条手动记忆笔记。每个角色一个文件，存于 `data/game_data/notes/<角色名>.json`。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
}

fn notes_dir() -> PathBuf {
    data_dir().join("game_data").join("notes")
}

/// 角色笔记文件路径。文件名取自 LingChat 权威角色名（display_name），
/// sanitize 后拼接，保证路径安全且与角色信息对齐。
fn role_notes_path(display_name: &str) -> PathBuf {
    notes_dir().join(format!("{}.json", sanitize_role_name(display_name)))
}

/// 清理角色名中的非法文件名字符，兜底防空/防 `..`。
fn sanitize_role_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "unknown".to_string()
    } else {
        cleaned
    }
}

fn load_role_notes(display_name: &str) -> Result<Vec<Note>, String> {
    let path = role_notes_path(display_name);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("读取角色 {display_name} 的笔记失败: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("解析角色 {display_name} 的笔记失败: {e}"))
}

/// 原子写入角色笔记（.tmp + rename）。
fn save_role_notes(display_name: &str, notes: &[Note]) -> Result<(), String> {
    let path = role_notes_path(display_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建笔记目录失败: {e}"))?;
    }
    let content =
        serde_json::to_string_pretty(notes).map_err(|e| format!("序列化笔记失败: {e}"))?;
    atomic_replace(&path, content.as_bytes()).map_err(|e| format!("保存笔记失败: {e}"))
}

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

fn apply_note_update(note: &mut Note, content: Option<String>, tags: Option<Vec<String>>) {
    if let Some(content) = content {
        note.content = content;
    }
    if let Some(tags) = tags {
        note.tags = tags;
    }
}

/// 取当前角色的权威名，供写操作定位笔记文件。返回后不持有锁。
async fn current_role_name_for_write(context: &ToolContext) -> Result<String, ToolError> {
    let app = context.require_app()?;
    let state = app.state::<AppState>();
    let db = state.db.clone();
    let gs = game_status_handle(&app).await;
    let mut gs = gs.lock().await;
    let name = current_display_name(&mut gs, &db).await?;
    Ok(name)
}

// ─── 工具 ───

/// memory_get_current：获取当前角色的自动记忆库文本。
pub struct GetCurrentMemory;

#[async_trait]
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
            "role_id": role_id,
            "memory": memory,
        }))
    }
}

/// memory_get_notes：获取手动记忆笔记。
///
/// 默认读取**当前角色**的笔记；可传 `role` 指定读取**其他角色**的笔记（只读）。
pub struct GetNotes;

#[async_trait]
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

        let notes = if let Some(role_name) = obj.get("role").and_then(Value::as_str) {
            let dn = resolve_display_name(&db, role_name).await?;
            load_role_notes(&dn).map_err(ToolError::Execution)?
        } else {
            let gs = game_status_handle(&app).await;
            let mut gs = gs.lock().await;
            let dn = current_display_name(&mut gs, &db).await?;
            load_role_notes(&dn).map_err(ToolError::Execution)?
        };
        Ok(json!(notes))
    }
}

/// memory_add_note：向当前角色添加一条手动记忆笔记。
pub struct AddNote;

#[async_trait]
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
        let mut notes = load_role_notes(&role_name).map_err(ToolError::Execution)?;
        let note = Note {
            id: Uuid::new_v4().to_string(),
            content,
            tags,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let id = note.id.clone();
        notes.push(note);
        save_role_notes(&role_name, &notes).map_err(ToolError::Execution)?;
        Ok(json!({"ok": true, "id": id}))
    }
}

/// memory_update_note：更新当前角色的手动记忆笔记。
pub struct UpdateNote;

#[async_trait]
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
            }
            None => None,
        };
        let tags = parse_tags(obj.get("tags"), "memory_update_note")?;

        let role_name = current_role_name_for_write(context).await?;
        let mut notes = load_role_notes(&role_name).map_err(ToolError::Execution)?;
        let Some(note) = notes.iter_mut().find(|n| n.id == id) else {
            return Err(ToolError::Execution(format!("笔记 {id} 不存在")));
        };
        apply_note_update(note, content, tags);
        save_role_notes(&role_name, &notes).map_err(ToolError::Execution)?;
        Ok(json!({"ok": true, "id": id}))
    }
}

/// memory_delete_note：删除当前角色的手动记忆笔记。
pub struct DeleteNote;

#[async_trait]
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
        let mut notes = load_role_notes(&role_name).map_err(ToolError::Execution)?;
        let before = notes.len();
        notes.retain(|n| n.id != id);
        if notes.len() == before {
            return Err(ToolError::Execution(format!("笔记 {id} 不存在")));
        }
        save_role_notes(&role_name, &notes).map_err(ToolError::Execution)?;
        Ok(json!({"ok": true, "id": id}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_serializes_with_tags() {
        let note = Note {
            id: "abc".into(),
            content: "记住用户的生日".into(),
            tags: vec!["用户".into()],
            created_at: "2026-08-02T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&note).unwrap();
        assert_eq!(json["id"], "abc");
        assert_eq!(json["tags"][0], "用户");
    }

    #[test]
    fn sanitize_role_name_handles_hostile_input() {
        assert_eq!(sanitize_role_name(" 玲玲 "), "玲玲");
        assert_eq!(sanitize_role_name("a/b:c*d"), "abcd");
        assert_eq!(sanitize_role_name("灵-01"), "灵-01");
        assert_eq!(sanitize_role_name(".."), "unknown");
        assert_eq!(sanitize_role_name(""), "unknown");
        assert_eq!(sanitize_role_name("   "), "unknown");
    }

    #[test]
    fn role_notes_path_uses_sanitized_name() {
        crate::init::static_copy::init_data_dir_for_tests();
        let path = role_notes_path("灵灵/..");
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(file.starts_with("灵灵") && file.ends_with(".json"));
        // 文件名必须安全：不含路径分隔符、不含 `..` 穿越
        assert!(!file.contains('/') && !file.contains(".."));
    }

    #[test]
    fn parse_tags_distinguishes_missing_empty_and_invalid() {
        assert_eq!(parse_tags(None, "test").unwrap(), None);
        assert!(parse_tags(Some(&json!("not_array")), "test").is_err());
        assert!(parse_tags(Some(&json!(["a", 2])), "test").is_err());
        let tags = parse_tags(Some(&json!(["a", "b"])), "test")
            .unwrap()
            .unwrap();
        assert_eq!(tags, vec!["a".to_string(), "b".to_string()]);
    }
}
