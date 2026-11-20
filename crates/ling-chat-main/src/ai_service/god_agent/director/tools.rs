//! 导演工具使用独立注册表，授权由运行层控制。

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::game_status_handle;
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::types::ToolDefinition;

use super::event_tools::{
    DirectorEventTool, EXECUTABLE_EVENT_TYPES, RejectedEventTool, event_tool_name,
};

pub const DIRECTOR_GET_WORLD_STATE: &str = "director_get_world_state";
pub const DIRECTOR_LIST_EVENTS: &str = "director_list_events";

pub const MAX_TOOL_ROUNDS: usize = 8;
pub const MAX_TOOL_CALLS_PER_ROUND: usize = 6;

/// 超限调用不执行，但保留配对失败结果。
pub fn deferred_tool_error(index: usize) -> Option<String> {
    (index >= MAX_TOOL_CALLS_PER_ROUND).then(|| {
        json!({
            "ok": false,
            "error": {
                "code": "call_limit",
                "message": "本轮工具调用数超过上限，该调用未执行。"
            }
        })
        .to_string()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopStep {
    Finish,
    ContinueWithTools,
    FinalSynthesis,
}

/// 达到轮数上限后，以不带工具的收尾轮结束循环。
pub fn plan_next_step(round: usize, had_tool_calls: bool) -> LoopStep {
    if !had_tool_calls {
        return LoopStep::Finish;
    }
    if round + 1 >= MAX_TOOL_ROUNDS {
        LoopStep::FinalSynthesis
    } else {
        LoopStep::ContinueWithTools
    }
}

pub fn director_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry
        .register(Arc::new(GetWorldState))
        .expect("注册 director_get_world_state 失败");
    registry
        .register(Arc::new(ListEvents))
        .expect("注册 director_list_events 失败");
    for &type_key in EXECUTABLE_EVENT_TYPES {
        let tool = DirectorEventTool::new(type_key)
            .unwrap_or_else(|| panic!("schema 缺少事件 {type_key} 的定义"));
        registry
            .register(Arc::new(tool))
            .unwrap_or_else(|_| panic!("注册 {type_key} 事件工具失败"));
    }
    registry
        .register(Arc::new(RejectedEventTool::set_variable()))
        .expect("注册 director_set_variable 占位失败");
    registry
        .register(Arc::new(RejectedEventTool::dialogue()))
        .expect("注册 director_dialogue 占位失败");
    registry
}

const DETERMINISTIC_EVENTS: &[(&str, &str)] = &[
    ("narration", "插入一段旁白文本，多行时逐行依次显示。"),
    ("player", "以玩家名义插入一句台词，进入对话历史。"),
    (
        "dialogue",
        "让指定在场角色说一句话，可带情绪标注（自由对话暂不可用）。",
    ),
    ("background", "切换场景背景图，可带过渡时长。"),
    ("background_effect", "切换氛围粒子特效，None 表示清除。"),
    ("present_pic", "在立绘层之上展示一张插图。"),
    ("music", "切换或停止背景音乐，可调播放速度。"),
    ("sound", "播放一段一次性音效。"),
    ("ambient", "播放或淡出停止环境音轨。"),
    ("set_variable", "给剧本变量赋值，可带执行条件。"),
];

pub struct GetWorldState;

#[async_trait]
impl Tool for GetWorldState {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            DIRECTOR_GET_WORLD_STATE,
            "查询当前现场状态：对话模式（自由对话或剧本名）、场景、背景、背景音乐与在场角色名列表。\
             想了解玩家当前看到什么时调用，不接受参数。",
            json!({ "type": "object", "properties": {} }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        _arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let game_status = game_status_handle(&app).await;
        // 获取现场快照后立即释放锁
        let (mode, scene, background, music, roles) = {
            let gs = game_status.lock().await;
            let mode = match gs.script_status.as_ref() {
                Some(script) => format!("剧本《{}》", script.name),
                None => "自由对话".to_string(),
            };
            let mut names: Vec<String> = gs
                .present_role_ids
                .iter()
                .filter(|id| **id != 0)
                .map(|id| {
                    gs.role_manager
                        .get_loaded(*id)
                        .map(|role| {
                            role.display_name
                                .clone()
                                .unwrap_or_else(|| role.settings.ai_name.clone())
                        })
                        .unwrap_or_else(|| format!("#{id}"))
                })
                .collect();
            names.sort_unstable();
            (
                mode,
                gs.current_scene_id.clone(),
                gs.background.clone(),
                gs.background_music.clone(),
                names,
            )
        };
        Ok(json!({
            "ok": true,
            "mode": mode,
            "scene": scene.as_deref().unwrap_or("未选择"),
            "background": present_text(&background),
            "music": present_text(&music),
            "present_roles": roles,
        }))
    }
}

pub struct ListEvents;

#[async_trait]
impl Tool for ListEvents {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            DIRECTOR_LIST_EVENTS,
            "列出导演可安排的剧本事件类型，每项含事件名与一句话说明。\
             可选 filter 按事件名做子串过滤；不传返回全部 10 类。",
            json!({
                "type": "object",
                "properties": {
                    "filter": {
                        "type": "string",
                        "description": "可选。按事件名过滤的关键字（不区分大小写），如 music。"
                    }
                }
            }),
        )
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let filter = arguments
            .get("filter")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let events: Vec<Value> = DETERMINISTIC_EVENTS
            .iter()
            .filter(|(name, _)| filter.is_empty() || name.to_lowercase().contains(&filter))
            .map(|(name, description)| {
                let (name, description) = (*name, *description);
                let available = EXECUTABLE_EVENT_TYPES.contains(&name);
                json!({
                    "name": name,
                    "description": description,
                    "available": available,
                    "tool": available.then(|| event_tool_name(name)),
                })
            })
            .collect();
        Ok(json!({ "ok": true, "events": events }))
    }
}

fn present_text(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        "无"
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::ai_service::game_system::script_engine::schema::build_schema;
    use crate::ai_service::god_agent::director::event_tools::{
        DIALOGUE_REJECT_MESSAGE, DIRECTOR_DIALOGUE, DIRECTOR_SET_VARIABLE,
        SET_VARIABLE_REJECT_MESSAGE, event_tool_definition, executable_event_spec,
        validate_event_arguments,
    };
    use crate::ai_service::tools::executor::ToolExecutor;

    #[test]
    fn 注册表按名取到工具且定义完整() {
        let registry = director_registry();
        let tool = registry
            .get(DIRECTOR_GET_WORLD_STATE)
            .expect("应能按名取到 director_get_world_state");
        let definition = tool.definition();
        assert_eq!(definition.function.name, DIRECTOR_GET_WORLD_STATE);
        assert_eq!(definition.function.parameters["type"], "object");

        let definitions = registry.definitions();
        assert_eq!(definitions.len(), 12, "导演工具总数应为 12");
        assert!(
            definitions
                .iter()
                .any(|d| d.function.name == DIRECTOR_LIST_EVENTS)
        );
        assert!(
            definitions
                .iter()
                .any(|d| d.function.name == DIRECTOR_SET_VARIABLE)
        );
        assert!(
            !EXECUTABLE_EVENT_TYPES.contains(&"dialogue"),
            "dialogue 不应出现在可执行事件白名单里"
        );
        let dialogue_definition = definitions
            .iter()
            .find(|d| d.function.name == DIRECTOR_DIALOGUE)
            .expect("dialogue 应有拒绝占位工具");
        assert!(
            dialogue_definition.function.description.contains("占位")
                || dialogue_definition
                    .function
                    .description
                    .contains("暂不可用"),
            "dialogue 占位工具的描述应说明不可用"
        );
        for &type_key in EXECUTABLE_EVENT_TYPES {
            assert!(
                definitions
                    .iter()
                    .any(|d| d.function.name == event_tool_name(type_key)),
                "缺少 {type_key} 的事件工具"
            );
        }
    }

    #[tokio::test]
    async fn 未知工具被注册表与执行器拒绝() {
        let registry = director_registry();
        assert!(
            registry.get("director_no_such_tool").is_none(),
            "未知工具名不应在注册表中"
        );

        let mut allowed: HashSet<String> = registry
            .definitions()
            .into_iter()
            .map(|d| d.function.name)
            .collect();
        allowed.insert("director_no_such_tool".to_string());
        let executor = ToolExecutor::new(&registry);
        let context = ToolContext::new(allowed);
        let result = executor
            .execute("director_no_such_tool", "{}", &context)
            .await;
        assert!(
            result.contains("unknown_tool"),
            "未知工具应返回错误编码: {result}"
        );
    }

    #[tokio::test]
    async fn 事件清单覆盖十类且支持过滤() {
        let registry = director_registry();
        let tool = registry
            .get(DIRECTOR_LIST_EVENTS)
            .expect("应能按名取到 director_list_events");
        let context = ToolContext::new(HashSet::new());
        let all = tool
            .execute(&context, json!({}))
            .await
            .expect("无参调用应成功");
        let events = all["events"].as_array().expect("events 应为数组");
        assert_eq!(events.len(), 10, "D3 首批应恰好十类事件");
        assert!(events.iter().any(|e| e["name"] == "narration"));
        for name in ["dialogue", "set_variable"] {
            let entry = events
                .iter()
                .find(|e| e["name"] == name)
                .expect("清单里应保留该事件");
            assert_eq!(entry["available"], false, "{name} 应标记为不可用");
            assert!(entry["tool"].is_null(), "{name} 不应给出可执行工具名");
        }

        let filtered = tool
            .execute(&context, json!({ "filter": "music" }))
            .await
            .expect("带 filter 调用应成功");
        let names: Vec<&str> = filtered["events"]
            .as_array()
            .expect("events 应为数组")
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert_eq!(names, vec!["music"]);
    }

    #[tokio::test]
    async fn 现场查询无宿主环境时明确报错() {
        let registry = director_registry();
        let tool = registry
            .get(DIRECTOR_GET_WORLD_STATE)
            .expect("应能按名取到 director_get_world_state");
        let error = tool
            .execute(&ToolContext::new(HashSet::new()), json!({}))
            .await
            .expect_err("没有 AppHandle 的上下文应报错");
        assert!(
            error.to_string().contains("宿主环境"),
            "错误信息应提示缺少宿主环境: {error}"
        );
    }

    #[test]
    fn 模型不请求工具时循环正常收尾() {
        assert_eq!(plan_next_step(0, false), LoopStep::Finish);
        assert_eq!(plan_next_step(MAX_TOOL_ROUNDS - 1, false), LoopStep::Finish);
    }

    #[test]
    fn 工具轮数上限触发无工具定义的收尾生成() {
        assert_eq!(plan_next_step(0, true), LoopStep::ContinueWithTools);
        assert_eq!(
            plan_next_step(MAX_TOOL_ROUNDS - 2, true),
            LoopStep::ContinueWithTools
        );
        assert_eq!(
            plan_next_step(MAX_TOOL_ROUNDS - 1, true),
            LoopStep::FinalSynthesis
        );
        assert_eq!(
            plan_next_step(MAX_TOOL_ROUNDS, true),
            LoopStep::FinalSynthesis
        );
    }

    #[test]
    fn 单轮工具调用上限生效() {
        for index in 0..MAX_TOOL_CALLS_PER_ROUND {
            assert_eq!(deferred_tool_error(index), None);
        }
        let output = deferred_tool_error(MAX_TOOL_CALLS_PER_ROUND).expect("超限调用应被拒绝");
        assert!(output.contains("call_limit"));
    }

    #[test]
    fn 八类事件工具名与必填字段齐全() {
        for &type_key in EXECUTABLE_EVENT_TYPES {
            let definition = event_tool_definition(type_key)
                .unwrap_or_else(|| panic!("{type_key} 应能生成工具定义"));
            assert_eq!(
                definition.function.name,
                event_tool_name(type_key),
                "工具名应由前缀 + type_key 组成"
            );
            let spec = executable_event_spec(type_key)
                .unwrap_or_else(|| panic!("{type_key} 应能在 schema 里找到"));
            let parameters = &definition.function.parameters;
            assert_eq!(parameters["type"], "object");
            let properties = parameters["properties"]
                .as_object()
                .expect("properties 应为对象");
            let required: Vec<&str> = parameters["required"]
                .as_array()
                .expect("required 应为数组")
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            let expected: Vec<&str> = spec
                .fields
                .iter()
                .filter(|field| field.required)
                .map(|field| field.key)
                .collect();
            assert_eq!(required, expected, "{type_key} 的必填字段应与 schema 一致");
            for field in &spec.fields {
                assert!(
                    properties.contains_key(field.key),
                    "{type_key} 缺少字段 {}",
                    field.key
                );
            }
        }
    }

    #[test]
    fn 未知事件类型被拒绝() {
        for type_key in [
            "choices",
            "input",
            "ai_dialogue",
            "chapter_end",
            "set_variable",
            "dialogue",
        ] {
            assert!(
                event_tool_definition(type_key).is_none(),
                "{type_key} 不应注册为可执行事件工具"
            );
        }
        assert!(event_tool_definition("director_no_such_event").is_none());
        assert!(DirectorEventTool::new("choices").is_none());
    }

    #[test]
    fn 必填参数缺失时返回错误而不是panic() {
        let spec = executable_event_spec("narration").expect("narration 应可执行");
        let error = validate_event_arguments(&spec, &json!({})).expect_err("缺少 text 应返回错误");
        assert!(error.contains("text"), "错误信息应点明缺失字段: {error}");

        let spec = executable_event_spec("background").expect("background 应可执行");
        let error = validate_event_arguments(&spec, &json!({ "imagePath": "  " }))
            .expect_err("空白素材名应返回错误");
        assert!(error.contains("imagePath"), "错误信息应点明字段: {error}");

        let spec = build_schema()
            .events
            .into_iter()
            .find(|spec| spec.type_key == "dialogue")
            .expect("schema 里应有 dialogue 定义");
        assert!(
            validate_event_arguments(&spec, &json!({ "character": "MAIN", "text": "你好" }))
                .is_ok(),
            "character 与 text 齐全应通过"
        );
        assert!(
            validate_event_arguments(&spec, &json!({ "character": "", "text": "你好" })).is_err(),
            "dialogue 的 character 不能为空"
        );
        assert!(
            validate_event_arguments(
                &spec,
                &json!({ "character": "MAIN", "text": "你好", "emotion": "随便什么值" })
            )
            .is_ok(),
            "emotion 不应被限制在候选值内"
        );
    }

    #[tokio::test]
    async fn set_variable占位工具返回明确拒绝文案() {
        let registry = director_registry();
        let tool = registry
            .get(DIRECTOR_SET_VARIABLE)
            .expect("应能按名取到占位工具");
        let result = tool
            .execute(&ToolContext::new(HashSet::new()), json!({}))
            .await
            .expect("占位工具执行本身不应报错");
        assert_eq!(result["ok"], false);
        assert_eq!(result["event"], "set_variable");
        assert_eq!(result["error"]["code"], "event_unavailable");
        assert_eq!(result["error"]["message"], SET_VARIABLE_REJECT_MESSAGE);
    }

    #[tokio::test]
    async fn dialogue占位工具返回明确拒绝文案() {
        let registry = director_registry();
        let tool = registry
            .get(DIRECTOR_DIALOGUE)
            .expect("应能按名取到 dialogue 占位工具");
        let result = tool
            .execute(&ToolContext::new(HashSet::new()), json!({}))
            .await
            .expect("占位工具执行本身不应报错");
        assert_eq!(result["ok"], false);
        assert_eq!(result["event"], "dialogue");
        assert_eq!(result["error"]["code"], "event_unavailable");
        assert_eq!(result["error"]["message"], DIALOGUE_REJECT_MESSAGE);
        let message = result["error"]["message"].as_str().expect("文案应为字符串");
        assert!(
            message.contains("自由对话暂不可用"),
            "文案应说明原因: {message}"
        );
    }

    #[tokio::test]
    async fn 事件工具缺宿主环境时明确报参数或上下文错误() {
        let registry = director_registry();
        let narration = registry
            .get(&event_tool_name("narration"))
            .expect("应能取到 narration 工具");
        let error = narration
            .execute(&ToolContext::new(HashSet::new()), json!({}))
            .await
            .expect_err("缺少 text 应报错");
        assert!(
            error.to_string().contains("text"),
            "错误应点明字段: {error}"
        );

        let error = narration
            .execute(
                &ToolContext::new(HashSet::new()),
                json!({ "text": "一段旁白" }),
            )
            .await
            .expect_err("没有 AppHandle 应报错");
        assert!(
            error.to_string().contains("宿主环境"),
            "应提示缺少宿主环境: {error}"
        );
    }

    #[test]
    fn 字段类型与select候选值校验生效() {
        let spec = executable_event_spec("music").expect("music 应可执行");
        assert!(
            validate_event_arguments(
                &spec,
                &json!({ "musicPath": "a.mp3", "playbackSpeed": "快" })
            )
            .is_err(),
            "playbackSpeed 必须是数字"
        );
        assert!(
            validate_event_arguments(
                &spec,
                &json!({ "musicPath": "不存在的素材.ogg", "playbackSpeed": 1.0 })
            )
            .is_ok(),
            "素材文件不应在工具层强制校验存在"
        );

        let spec = executable_event_spec("background_effect").expect("特效事件应可执行");
        let valid = spec
            .fields
            .iter()
            .find(|f| f.key == "effect")
            .and_then(|f| f.options.first())
            .expect("effect 应有候选项")
            .to_string();
        assert!(
            validate_event_arguments(&spec, &json!({ "effect": valid })).is_ok(),
            "合法特效候选值应通过"
        );
        assert!(
            validate_event_arguments(&spec, &json!({ "effect": "不是特效名" })).is_err(),
            "非候选值应被工具内校验挡下"
        );
    }
}
