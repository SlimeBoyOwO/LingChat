//! 导演事件工具：把 schema.rs 的确定性事件注册成可调用工具（D2/D3）。
//!
//! 工具定义不在导演侧另抄字段表，一律从 `build_schema()` 的 `EventSpec` 转换；
//! 执行时补上 `type` 组装 event_data，交给既有的 `create_event` 处理器并构造
//! `ScriptContext` 执行。确定性事件内部只 emit 到前端队列 + 写台词表，不等待
//! 玩家，工具返回「已投递」而非「已播完」。

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

/// D3 本期可执行的 8 类确定性事件，取值即 schema.rs 的 `type_key`。
/// dialogue 硬依赖 ScriptStatus 的角色空间，自由对话下不可用，降级为拒绝占位。
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

/// 事件工具统一前缀，模型按 `director_<type_key>` 调用。
pub const EVENT_TOOL_PREFIX: &str = "director_";

/// set_variable / dialogue 的占位工具名：清单里保留，但调用一律明确拒绝。
pub const DIRECTOR_SET_VARIABLE: &str = "director_set_variable";
pub const DIRECTOR_DIALOGUE: &str = "director_dialogue";

/// 拒绝 set_variable 的固定文案（D3）。
pub const SET_VARIABLE_REJECT_MESSAGE: &str = "该事件依赖剧本变量容器，自由对话暂不可用";
/// 拒绝 dialogue 的固定文案（自由对话无 ScriptStatus 角色空间）。
pub const DIALOGUE_REJECT_MESSAGE: &str = "角色对白依赖剧本角色空间，自由对话暂不可用";

/// 由事件类型名拼出工具名。
pub fn event_tool_name(type_key: &str) -> String {
    let prefix = EVENT_TOOL_PREFIX;
    format!("{prefix}{type_key}")
}

/// 按 type_key 取可执行事件的 spec；不在白名单或 schema 里没有则拒绝。
pub fn executable_event_spec(type_key: &str) -> Option<EventSpec> {
    if !EXECUTABLE_EVENT_TYPES.contains(&type_key) {
        return None;
    }
    build_schema()
        .events
        .into_iter()
        .find(|spec| spec.type_key == type_key)
}

/// 把 EventSpec 转成工具定义；未知或不可执行的 type_key 返回 None。
pub fn event_tool_definition(type_key: &str) -> Option<ToolDefinition> {
    let spec = executable_event_spec(type_key)?;
    Some(build_definition(&spec))
}

/// 字段控件类型到 JSON Schema 类型的映射。
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
        // 其余控件只出现在被排除的事件里，9 类事件用不到
        _ => "string",
    }
}

/// 文本类字段：素材、角色、章节、下拉、单行/多行文本。
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

/// 字段说明由 label + hint 拼成，再附上默认值与示例，全部取自 schema。
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

/// 由 EventSpec 生成 JSON Schema：properties、required、description 全部来自字段表。
fn build_definition(spec: &EventSpec) -> ToolDefinition {
    let mut properties = serde_json::Map::new();
    let mut required: Vec<&str> = Vec::new();
    for field in &spec.fields {
        // 类型映射照抄字段表的 kind；情绪不挂 enum——候选项由前端情绪表决定，
        // 表外的值会回落成「正常」，导演侧不做枚举校验
        let mut schema = json!({
            "type": field_schema_type(field.kind),
            "description": field_description(field),
        });
        // Select 的候选值给模型做提示；执行器的 Schema 子集不校验 enum，工具内另兜一层
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
    let parameters = json!({
        "type": "object",
        "properties": properties,
        "required": required,
    });
    let label = spec.label;
    let type_key = spec.type_key;
    let description = format!(
        "安排一个「{label}」剧本事件（type={type_key}）。参数与剧本 YAML 事件一致；调用成功后事件即进入前端播放队列，不等待播放完成。"
    );
    ToolDefinition::new(event_tool_name(type_key), description, parameters)
}

/// 用 schema 的 FieldSpec 兜一层必填/类型校验。
///
/// 执行器只认 JSON Schema 的一个子集，enum、非空约束都不生效；素材类字段
/// 在 MVP 只校验非空字符串、不检查文件是否存在；emotion 完全不校验。
pub fn validate_event_arguments(spec: &EventSpec, arguments: &Value) -> Result<(), String> {
    let type_key = spec.type_key;
    let object = arguments
        .as_object()
        .ok_or_else(|| format!("{type_key} 事件的参数必须是 JSON object"))?;
    for field in &spec.fields {
        // 情绪值由前端情绪表兜底，表外的值会回落成「正常」，这里不管
        if matches!(field.kind, FieldKind::Emotion) {
            continue;
        }
        let key = field.key;
        let value = object.get(key);
        if field.required {
            let Some(value) = value else {
                return Err(format!("{type_key} 事件缺少必填字段 {key}"));
            };
            if value.is_null() {
                return Err(format!("{type_key} 事件的必填字段 {key} 不能为 null"));
            }
            // 素材 / 角色 / 文本类只要求非空字符串，不强制素材文件存在（MVP）
            if is_text_kind(field.kind) {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("{type_key} 事件的字段 {key} 必须是字符串"))?;
                if text.trim().is_empty() {
                    return Err(format!("{type_key} 事件的字段 {key} 不能为空"));
                }
            }
        }
        // 可选字段传 null 视作「没传」，避免模型填 null 时被误判成类型错误
        if let Some(value) = value.filter(|v| !v.is_null()) {
            check_field_value(type_key, field, value)?;
        }
    }
    Ok(())
}

/// 校验单个字段的值：基础类型 + Select 的候选值。
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
    // 执行器不校验 enum，Select 的合法值在这里兜住
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

/// 一个事件对应一个工具：持有转换好的定义与原始 spec。
pub struct DirectorEventTool {
    type_key: &'static str,
    spec: EventSpec,
    definition: ToolDefinition,
}

impl DirectorEventTool {
    /// 白名单外的 type_key 直接拒绝，不生成工具。
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
        // 确定性事件只 emit 不等待玩家，但写台词 / 走 TTS 富化可能略慢，给足余量
        Some(Duration::from_secs(30))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        // 先在工具内校验，缺少宿主环境时也能明确报参数错误而不是 panic
        validate_event_arguments(&self.spec, &arguments).map_err(ToolError::InvalidArguments)?;
        let app = context.require_app()?;
        // 与周期草案确认共用同一条执行路径（内部含参数校验与 emit）
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

/// 校验并执行一个确定性事件：白名单 + 参数校验 + create_event + ScriptContext。
///
/// 导演工具（S2）与周期草案确认（S3/D2）共用同一条执行路径，避免复制粘贴
/// 依赖组装与 emit 逻辑。参数即事件的 YAML 字段（不含 type），函数内补齐。
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
    // 锁内只取快照，事件执行期间不持有 ai_service 的锁
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

    // 补上事件 YAML 里的 type 字段，其余参数原样交给处理器
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

/// 事件清单里保留、但首期不可执行的占位工具：调用时返回明确拒绝文案。
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

    /// set_variable 占位实例。
    pub fn set_variable() -> Self {
        Self::new(
            "set_variable",
            DIRECTOR_SET_VARIABLE,
            SET_VARIABLE_REJECT_MESSAGE,
        )
    }

    /// dialogue 占位实例：自由对话没有 ScriptStatus 角色空间。
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
