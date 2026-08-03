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
use super::ensure_no_args;

fn schedules_path() -> std::path::PathBuf {
    data_dir().join("game_data").join("schedules.json")
}

/// 读入日程配置；文件不存在或解析失败时返回空配置。
fn load_schedule_settings() -> UserScheduleSettings {
    let path = schedules_path();
    if !path.exists() {
        return UserScheduleSettings::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

/// 原子写入日程配置（.tmp + rename）。
fn save_schedule_settings(settings: &UserScheduleSettings) -> Result<(), String> {
    let path = schedules_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建日程目录失败: {e}"))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化日程配置失败: {e}"))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content).map_err(|e| format!("写入日程临时文件失败: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("保存日程配置失败: {e}"))?;
    Ok(())
}

/// 重载主动对话系统的日程提醒配置（与 api/schedule.rs save_schedules 一致）。
async fn reload_proactive(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(proactive) = &state.proactive_system {
        let mut sys = proactive.lock().await;
        sys.reload().await;
    }
}

fn next_todo_id(group: &TodoGroup) -> i64 {
    group.todos.iter().map(|t| t.id).max().unwrap_or(0) + 1
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
        let settings = load_schedule_settings();
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
        let priority = obj.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
        let deadline = obj
            .get("deadline")
            .and_then(Value::as_str)
            .map(str::to_string);

        let mut settings = load_schedule_settings();
        let groups = settings.todo_groups.get_or_insert_with(HashMap::new);
        let group = groups.entry(group_name.clone()).or_insert_with(|| TodoGroup {
            title: group_name,
            description: None,
            todos: Vec::new(),
        });
        let new_id = next_todo_id(group);
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
        Ok(json!({"ok": true, "id": new_id}))
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
                    "priority": {"type": "integer", "description": "新的优先级，可选"}
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
        let id = obj
            .get("id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ToolError::InvalidArguments("schedule_update_todo 需要整数 id".into()))?;
        let done = obj.get("done").and_then(Value::as_bool);
        let text = obj.get("text").and_then(Value::as_str).map(str::to_string);
        let priority = obj
            .get("priority")
            .and_then(Value::as_i64)
            .map(|p| p as i32);
        if done.is_none() && text.is_none() && priority.is_none() {
            return Err(ToolError::InvalidArguments(
                "schedule_update_todo 至少需要 done/text/priority 中的一项".into(),
            ));
        }

        let mut settings = load_schedule_settings();
        let mut found = false;
        if let Some(groups) = &mut settings.todo_groups {
            for group in groups.values_mut() {
                if let Some(todo) = group.todos.iter_mut().find(|t| t.id == id) {
                    if let Some(d) = done {
                        todo.completed = d;
                    }
                    if let Some(t) = text.clone() {
                        todo.text = t;
                    }
                    if let Some(p) = priority {
                        todo.priority = p;
                    }
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return Err(ToolError::Execution(format!("待办 {id} 不存在")));
        }

        save_schedule_settings(&settings).map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        reload_proactive(&app).await;
        Ok(json!({"ok": true, "id": id}))
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
                    "id": {"type": "integer", "description": "待办 ID"}
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
        let id = obj
            .get("id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ToolError::InvalidArguments("schedule_delete_todo 需要整数 id".into()))?;

        let mut settings = load_schedule_settings();
        let mut found = false;
        if let Some(groups) = &mut settings.todo_groups {
            for group in groups.values_mut() {
                let before = group.todos.len();
                group.todos.retain(|t| t.id != id);
                if group.todos.len() != before {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return Err(ToolError::Execution(format!("待办 {id} 不存在")));
        }

        save_schedule_settings(&settings).map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        reload_proactive(&app).await;
        Ok(json!({"ok": true, "id": id}))
    }
}

/// 校验参数为 JSON object 并返回引用。
fn require_object<'a>(arguments: &'a Value, tool: &str) -> Result<&'a serde_json::Map<String, Value>, ToolError> {
    arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 参数必须是 JSON object")))
}
