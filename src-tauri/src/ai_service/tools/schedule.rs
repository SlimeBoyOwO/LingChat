use std::collections::HashMap;
use std::fs;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_service::proactive_system::types::{TodoGroup, TodoItem, UserScheduleSettings};
use crate::ai_service::types::ToolDefinition;
use crate::api::data_dir;
use crate::AppState;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{atomic_replace, ensure_no_args};

fn schedules_path() -> std::path::PathBuf {
    data_dir().join("game_data").join("schedules.json")
}

/// 读入日程配置。文件不存在时返回空配置；读取或解析失败必须显式报错，避免写工具
/// 把损坏的原文件当成空配置覆盖掉。
fn load_schedule_settings() -> Result<UserScheduleSettings, String> {
    let path = schedules_path();
    if !path.exists() {
        return Ok(UserScheduleSettings::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("读取日程配置失败: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("解析日程配置失败: {e}"))
}

/// 原子写入日程配置（.tmp + rename）。
fn save_schedule_settings(settings: &UserScheduleSettings) -> Result<(), String> {
    let path = schedules_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建日程目录失败: {e}"))?;
    }
    let content =
        serde_json::to_string_pretty(settings).map_err(|e| format!("序列化日程配置失败: {e}"))?;
    atomic_replace(&path, content.as_bytes()).map_err(|e| format!("保存日程配置失败: {e}"))
}

/// 重载主动对话系统的日程提醒配置（与 api/schedule.rs save_schedules 一致）。
async fn reload_proactive(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(proactive) = &state.proactive_system {
        let mut sys = proactive.lock().await;
        sys.reload().await;
    }
}

fn next_todo_id(settings: &UserScheduleSettings) -> Result<i64, String> {
    let maximum = settings
        .todo_groups
        .as_ref()
        .into_iter()
        .flat_map(|groups| groups.values())
        .flat_map(|group| group.todos.iter())
        .map(|todo| todo.id)
        .max()
        .unwrap_or(0)
        .max(0);
    maximum
        .checked_add(1)
        .ok_or_else(|| "待办 ID 已达到上限，无法继续添加".to_string())
}

/// 按 ID 定位待办所在分组。旧数据可能在多个分组里存在重复 ID；此时要求模型
/// 补充 group，避免静默修改/删除错误的待办。
fn locate_todo_group(
    settings: &UserScheduleSettings,
    id: i64,
    requested_group: Option<&str>,
) -> Result<String, String> {
    let Some(groups) = settings.todo_groups.as_ref() else {
        return Err(format!("待办 {id} 不存在"));
    };
    if let Some(group_name) = requested_group {
        let Some(group) = groups.get(group_name) else {
            return Err(format!("待办分组 {group_name} 不存在"));
        };
        return if group.todos.iter().any(|todo| todo.id == id) {
            Ok(group_name.to_string())
        } else {
            Err(format!("分组 {group_name} 中不存在待办 {id}"))
        };
    }

    let mut matches: Vec<String> = groups
        .iter()
        .filter(|(_, group)| group.todos.iter().any(|todo| todo.id == id))
        .map(|(name, _)| name.clone())
        .collect();
    matches.sort();
    match matches.as_slice() {
        [] => Err(format!("待办 {id} 不存在")),
        [group] => Ok(group.clone()),
        _ => Err(format!(
            "待办 ID {id} 在多个分组中重复，请提供 group（可选: {}）",
            matches.join(", ")
        )),
    }
}

fn optional_i32(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    tool: &str,
) -> Result<Option<i32>, ToolError> {
    let Some(value) = obj.get(key) else {
        return Ok(None);
    };
    let raw = value
        .as_i64()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 的 {key} 必须是整数")))?;
    i32::try_from(raw)
        .map(Some)
        .map_err(|_| ToolError::InvalidArguments(format!("{tool} 的 {key} 超出 i32 范围")))
}

/// schedule_get_all：获取全部日程、待办和重要日子。
pub struct GetAllSchedule;

#[async_trait]
impl Tool for GetAllSchedule {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "schedule_get_all",
            "获取当前全部日程分组、待办事项和重要日子",
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
        _context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        ensure_no_args(&arguments, "schedule_get_all").map_err(ToolError::Execution)?;
        let settings = load_schedule_settings().map_err(ToolError::Execution)?;
        Ok(serde_json::to_value(&settings).map_err(|e| ToolError::Execution(e.to_string()))?)
    }
}

/// schedule_add_todo：向指定待办分组添加待办事项。
pub struct AddTodo;

#[async_trait]
impl Tool for AddTodo {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "schedule_add_todo",
            "添加一条待办事项。可指定分组（默认 default）、优先级（默认 0）和截止时间",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "待办内容"},
                    "group": {"type": "string", "description": "分组名，默认 default"},
                    "priority": {"type": "integer", "description": "优先级，默认 0"},
                    "deadline": {"type": "string", "description": "截止时间，可选"}
                },
                "required": ["text"],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "schedule_add_todo")?;
        let text = obj
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("schedule_add_todo 需要 text".into()))?;
        if text.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "schedule_add_todo 的 text 不能为空".into(),
            ));
        }
        let group_name = obj
            .get("group")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();
        let priority = optional_i32(obj, "priority", "schedule_add_todo")?.unwrap_or(0);
        let deadline = obj
            .get("deadline")
            .and_then(Value::as_str)
            .map(str::to_string);

        let mut settings = load_schedule_settings().map_err(ToolError::Execution)?;
        let new_id = next_todo_id(&settings).map_err(ToolError::Execution)?;
        let groups = settings.todo_groups.get_or_insert_with(HashMap::new);
        let group = groups
            .entry(group_name.clone())
            .or_insert_with(|| TodoGroup {
                title: group_name.clone(),
                description: None,
                todos: Vec::new(),
            });
        group.todos.push(TodoItem {
            id: new_id,
            text,
            priority,
            completed: false,
            deadline,
        });

        save_schedule_settings(&settings).map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        reload_proactive(&app).await;
        Ok(json!({"ok": true, "id": new_id, "group": group_name}))
    }
}

/// schedule_update_todo：更新待办状态或内容。
pub struct UpdateTodo;

#[async_trait]
impl Tool for UpdateTodo {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "schedule_update_todo",
            "按 ID 更新待办事项的完成状态、内容或优先级，至少提供一项",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "待办 ID"},
                    "done": {"type": "boolean", "description": "是否已完成，可选"},
                    "text": {"type": "string", "description": "新的待办内容，可选"},
                    "priority": {"type": "integer", "description": "新的优先级，可选"},
                    "group": {"type": "string", "description": "分组名；旧数据 ID 重复时必须提供"}
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
        let obj = require_object(&arguments, "schedule_update_todo")?;
        let id = obj.get("id").and_then(Value::as_i64).ok_or_else(|| {
            ToolError::InvalidArguments("schedule_update_todo 需要整数 id".into())
        })?;
        let done = obj.get("done").and_then(Value::as_bool);
        let text = obj.get("text").and_then(Value::as_str).map(str::to_string);
        let priority = optional_i32(obj, "priority", "schedule_update_todo")?;
        let requested_group = obj.get("group").and_then(Value::as_str);
        if done.is_none() && text.is_none() && priority.is_none() {
            return Err(ToolError::InvalidArguments(
                "schedule_update_todo 至少需要 done/text/priority 中的一项".into(),
            ));
        }

        if text.as_deref().is_some_and(|value| value.trim().is_empty()) {
            return Err(ToolError::InvalidArguments(
                "schedule_update_todo 的 text 不能为空".into(),
            ));
        }
        let mut settings = load_schedule_settings().map_err(ToolError::Execution)?;
        let group_name =
            locate_todo_group(&settings, id, requested_group).map_err(ToolError::Execution)?;
        let todo = settings
            .todo_groups
            .as_mut()
            .and_then(|groups| groups.get_mut(&group_name))
            .and_then(|group| group.todos.iter_mut().find(|todo| todo.id == id))
            .ok_or_else(|| ToolError::Execution(format!("待办 {id} 不存在")))?;
        if let Some(d) = done {
            todo.completed = d;
        }
        if let Some(t) = text {
            todo.text = t;
        }
        if let Some(p) = priority {
            todo.priority = p;
        }

        save_schedule_settings(&settings).map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        reload_proactive(&app).await;
        Ok(json!({"ok": true, "id": id, "group": group_name}))
    }
}

/// schedule_delete_todo：删除指定待办事项。
pub struct DeleteTodo;

#[async_trait]
impl Tool for DeleteTodo {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "schedule_delete_todo",
            "按 ID 删除待办事项",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "待办 ID"},
                    "group": {"type": "string", "description": "分组名；旧数据 ID 重复时必须提供"}
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
        let obj = require_object(&arguments, "schedule_delete_todo")?;
        let id = obj.get("id").and_then(Value::as_i64).ok_or_else(|| {
            ToolError::InvalidArguments("schedule_delete_todo 需要整数 id".into())
        })?;

        let requested_group = obj.get("group").and_then(Value::as_str);
        let mut settings = load_schedule_settings().map_err(ToolError::Execution)?;
        let group_name =
            locate_todo_group(&settings, id, requested_group).map_err(ToolError::Execution)?;
        let group = settings
            .todo_groups
            .as_mut()
            .and_then(|groups| groups.get_mut(&group_name))
            .ok_or_else(|| ToolError::Execution(format!("待办分组 {group_name} 不存在")))?;
        group.todos.retain(|todo| todo.id != id);

        save_schedule_settings(&settings).map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        reload_proactive(&app).await;
        Ok(json!({"ok": true, "id": id, "group": group_name}))
    }
}

/// 校验参数为 JSON object 并返回引用。
fn require_object<'a>(
    arguments: &'a Value,
    tool: &str,
) -> Result<&'a serde_json::Map<String, Value>, ToolError> {
    arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 参数必须是 JSON object")))
}
