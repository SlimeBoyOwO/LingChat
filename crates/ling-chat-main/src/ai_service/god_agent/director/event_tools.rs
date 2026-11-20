//! 导演事件工具复用剧本事件定义，成功只表示事件已投递。

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::game_system::script_engine::events::{ScriptContext, create_event};
use crate::ai_service::game_system::script_engine::schema::{
    EventSpec, FieldKind, FieldSpec, build_schema,
};
use crate::ai_service::llm::slot_snapshot;
use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::types::ToolDefinition;

/// 可执行事件白名单；对白依赖剧本角色空间，因此不在其中。
pub const EXECUTABLE_EVENT_TYPES: &[&str] = &[
    "narration",
    "player",
    "background",
    "background_effect",
    "present_pic",
    "music",
    "sound",
    "ambient",
];

pub const EVENT_TOOL_PREFIX: &str = "director_";

pub const DIRECTOR_SET_VARIABLE: &str = "director_set_variable";
pub const DIRECTOR_DIALOGUE: &str = "director_dialogue";

pub const SET_VARIABLE_REJECT_MESSAGE: &str = "该事件依赖剧本变量容器，自由对话暂不可用";
pub const DIALOGUE_REJECT_MESSAGE: &str = "角色对白依赖剧本角色空间，自由对话暂不可用";

pub fn event_tool_name(type_key: &str) -> String {
    let prefix = EVENT_TOOL_PREFIX;
    format!("{prefix}{type_key}")
}

pub fn executable_event_spec(type_key: &str) -> Option<EventSpec> {
    if !EXECUTABLE_EVENT_TYPES.contains(&type_key) {
        return None;
    }
    build_schema()
        .events
        .into_iter()
        .find(|spec| spec.type_key == type_key)
}

pub fn event_tool_definition(type_key: &str) -> Option<ToolDefinition> {
    let spec = executable_event_spec(type_key)?;
    Some(build_definition(&spec))
}

fn field_schema_type(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text
        | FieldKind::Textarea
        | FieldKind::Select
        | FieldKind::Asset
        | FieldKind::Character
        | FieldKind::Emotion
        | FieldKind::Chapter => "string",
        FieldKind::Number => "number",
        FieldKind::Bool => "boolean",
        _ => "string",
    }
}

fn is_text_kind(kind: FieldKind) -> bool {
    matches!(
        kind,
        FieldKind::Text
            | FieldKind::Textarea
            | FieldKind::Select
            | FieldKind::Asset
            | FieldKind::Character
            | FieldKind::Emotion
            | FieldKind::Chapter
    )
}

fn field_description(field: &FieldSpec) -> String {
    let mut parts = vec![field.label.to_string()];
    if let Some(hint) = field.hint {
        parts.push(hint.to_string());
    }
    if let Some(default_desc) = field.default_desc {
        parts.push(format!("默认：{default_desc}"));
    }
    if let Some(placeholder) = field.placeholder {
        parts.push(format!("示例：{placeholder}"));
    }
    parts.join("；")
}

fn build_definition(spec: &EventSpec) -> ToolDefinition {
    let mut properties = serde_json::Map::new();
    let mut required: Vec<&str> = Vec::new();
    for field in &spec.fields {
        let mut schema = json!({
            "type": field_schema_type(field.kind),
            "description": field_description(field),
        });
        if matches!(field.kind, FieldKind::Select)
            && !field.options.is_empty()
            && let Some(object) = schema.as_object_mut()
        {
            object.insert("enum".to_string(), json!(field.options));
        }
        properties.insert(field.key.to_string(), schema);
        if field.required {
            required.push(field.key);
        }
    }
    if let Some(field) = build_schema()
        .common_fields
        .iter()
        .find(|field| field.key == "duration")
    {
        properties.insert(
            field.key.to_string(),
            json!({ "type": "number", "description": field_description(field) }),
        );
    }
    let parameters = json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    });
    let label = spec.label;
    let type_key = spec.type_key;
    let description = format!(
        "安排一个「{label}」剧本事件（type={type_key}）。参数与剧本 YAML 事件一致；调用成功后事件即进入前端播放队列，不等待播放完成。"
    );
    ToolDefinition::new(event_tool_name(type_key), description, parameters)
}

/// 补齐执行器未覆盖的必填、类型与取值校验。
pub fn validate_event_arguments(spec: &EventSpec, arguments: &Value) -> Result<(), String> {
    let type_key = spec.type_key;
    let object = arguments
        .as_object()
        .ok_or_else(|| format!("{type_key} 事件的参数必须是 JSON object"))?;
    let schema = build_schema();
    let fields = spec.fields.iter().chain(
        schema
            .common_fields
            .iter()
            .filter(|field| field.key == "duration"),
    );
    if let Some(key) = object
        .keys()
        .find(|key| !fields.clone().any(|field| field.key == key.as_str()))
    {
        return Err(format!("{type_key} 事件包含未知字段 {key}"));
    }
    for field in fields {
        let key = field.key;
        let value = object.get(key);
        if field.required {
            let Some(value) = value else {
                return Err(format!("{type_key} 事件缺少必填字段 {key}"));
            };
            if value.is_null() {
                return Err(format!("{type_key} 事件的必填字段 {key} 不能为 null"));
            }
            if is_text_kind(field.kind) {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("{type_key} 事件的字段 {key} 必须是字符串"))?;
                if text.trim().is_empty() {
                    return Err(format!("{type_key} 事件的字段 {key} 不能为空"));
                }
            }
        }
        if let Some(value) = value.filter(|v| !v.is_null()) {
            check_field_value(type_key, field, value)?;
        }
    }
    if type_key == "ambient"
        && object.get("stop").and_then(Value::as_bool) != Some(true)
        && object
            .get("ambientPath")
            .and_then(Value::as_str)
            .is_none_or(|path| path.trim().is_empty())
    {
        return Err("播放环境音时 ambientPath 不能为空".to_string());
    }
    Ok(())
}

fn check_field_value(type_key: &str, field: &FieldSpec, value: &Value) -> Result<(), String> {
    let key = field.key;
    if matches!(field.kind, FieldKind::Number) && !value.is_number() {
        return Err(format!("{type_key} 事件的字段 {key} 必须是数字"));
    }
    if matches!(field.kind, FieldKind::Bool) && !value.is_boolean() {
        return Err(format!("{type_key} 事件的字段 {key} 必须是布尔值"));
    }
    if is_text_kind(field.kind) && !value.is_string() {
        return Err(format!("{type_key} 事件的字段 {key} 必须是字符串"));
    }
    if matches!(field.kind, FieldKind::Select) && !field.options.is_empty() {
        let text = value.as_str().unwrap_or_default();
        let allowed = field
            .options
            .iter()
            .any(|option| option.eq_ignore_ascii_case(text));
        if !allowed {
            let options = field.options.join(" / ");
            return Err(format!("{type_key} 事件的字段 {key} 只能是：{options}"));
        }
    }
    Ok(())
}

pub struct DirectorEventTool {
    type_key: &'static str,
    spec: EventSpec,
    definition: ToolDefinition,
}

impl DirectorEventTool {
    pub fn new(type_key: &'static str) -> Option<Self> {
        let spec = executable_event_spec(type_key)?;
        let definition = build_definition(&spec);
        Some(Self {
            type_key,
            spec,
            definition,
        })
    }
}

#[async_trait]
impl Tool for DirectorEventTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        validate_event_arguments(&self.spec, &arguments).map_err(ToolError::InvalidArguments)?;
        let app = context.require_app()?;
        execute_director_event(&app, self.type_key, arguments)
            .await
            .map_err(ToolError::Execution)?;

        Ok(json!({
            "ok": true,
            "delivered": true,
            "event": self.type_key,
        }))
    }
}

/// 校验白名单和参数，再交给剧本事件处理器。
pub async fn execute_director_event(
    app: &AppHandle,
    type_key: &str,
    arguments: Value,
) -> Result<(), String> {
    if !EXECUTABLE_EVENT_TYPES.contains(&type_key) {
        return Err(format!("事件 {type_key} 不在可执行白名单内"));
    }
    let spec = executable_event_spec(type_key)
        .ok_or_else(|| format!("事件 {type_key} 在 schema 里没有可执行定义"))?;
    validate_event_arguments(&spec, &arguments)?;

    let state = app.state::<AppState>();
    if !crate::api::director::director_tools_enabled(&state).await {
        return Err("当前正在运行固定剧本，导演事件已关闭".to_string());
    }
    // 事件执行期间不持有服务锁
    let (db, data_dir, config, game_status, channels) = {
        let service = state.ai_service.lock().await;
        (
            service.db.clone(),
            service.data_dir.clone(),
            service.config.clone(),
            service.game_status.clone(),
            state.script_channels.clone(),
        )
    };
    let llm = slot_snapshot(&state.chat.llm).await;

    let mut event_data = arguments;
    if let Some(object) = event_data.as_object_mut() {
        object.insert("type".to_string(), json!(type_key));
    }
    let mut event = create_event(type_key, event_data)
        .ok_or_else(|| format!("事件 {type_key} 没有注册处理器"))?;

    let mut ctx = ScriptContext {
        db: &db,
        data_dir: &data_dir,
        app,
        game_status,
        config: &config,
        llm: llm.as_ref(),
        channels,
        is_preview: false,
    };
    event
        .execute(&mut ctx)
        .await
        .map_err(|error| format!("事件 {type_key} 执行失败: {error}"))?;
    Ok(())
}

/// 清单中不可执行的事件始终返回拒绝结果。
pub struct RejectedEventTool {
    type_key: &'static str,
    tool_name: &'static str,
    message: &'static str,
}

impl RejectedEventTool {
    pub fn new(type_key: &'static str, tool_name: &'static str, message: &'static str) -> Self {
        Self {
            type_key,
            tool_name,
            message,
        }
    }

    pub fn set_variable() -> Self {
        Self::new(
            "set_variable",
            DIRECTOR_SET_VARIABLE,
            SET_VARIABLE_REJECT_MESSAGE,
        )
    }

    pub fn dialogue() -> Self {
        Self::new("dialogue", DIRECTOR_DIALOGUE, DIALOGUE_REJECT_MESSAGE)
    }
}

#[async_trait]
impl Tool for RejectedEventTool {
    fn definition(&self) -> ToolDefinition {
        let message = self.message;
        ToolDefinition::new(
            self.tool_name,
            format!("占位工具：{message}。清单里能看到该事件，但请不要调用本工具。"),
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        _arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        Ok(json!({
            "ok": false,
            "event": self.type_key,
            "error": {
                "code": "event_unavailable",
                "message": self.message,
            }
        }))
    }
}
