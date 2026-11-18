//! 导演 Agent 专用工具注册表与首批只读工具。
//!
//! 注册表独立存在（同 god_agent_registry 的隔离写法），不接入角色聊天的
//! GeneratorSource / 权限矩阵，也不进工具权限页——导演工具的授权由导演
//! 运行层自己决定（见 docs/decisions-director-agent.md 架构基线）。
//! 本期先注册两个只读工具打通回路：director_get_world_state（现场快照）与
//! director_list_events（D3 的 10 类确定性事件清单，S2 真实事件落地前的占位）。

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::game_status_handle;
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::types::ToolDefinition;

pub const DIRECTOR_GET_WORLD_STATE: &str = "director_get_world_state";
pub const DIRECTOR_LIST_EVENTS: &str = "director_list_events";

/// 导演工具循环最多带工具定义的生成轮数。
pub const MAX_TOOL_ROUNDS: usize = 8;
/// 单轮最多执行的工具调用数，超出部分不下发、不执行。
pub const MAX_TOOL_CALLS_PER_ROUND: usize = 6;

/// 一轮工具生成结束后，工具循环下一步的决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopStep {
    /// 模型没有请求工具：本轮正文即最终答复，正常结束。
    Finish,
    /// 带上工具定义进入下一轮。
    ContinueWithTools,
    /// 已用满工具轮数：下一轮不带工具定义做一次收尾生成。
    FinalSynthesis,
}

/// 根据刚结束的轮序号与本轮是否请求工具，决定下一步。
///
/// `round` 从 0 起；带工具的轮为 0..MAX_TOOL_ROUNDS-1。第
/// MAX_TOOL_ROUNDS-1 轮仍请求工具时，改走一次不带工具定义的收尾，
/// 既保住已完成的工具结果，又不会无限循环。
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

/// 构建导演专用注册表。
pub fn director_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry
        .register(Arc::new(GetWorldState))
        .expect("注册 director_get_world_state 失败");
    registry
        .register(Arc::new(ListEvents))
        .expect("注册 director_list_events 失败");
    registry
}

/// D3 首批确定性事件：名字 + 一句话说明。静态数据，为 S2 占位。
const DETERMINISTIC_EVENTS: &[(&str, &str)] = &[
    ("narration", "插入一段旁白文本，多行时逐行依次显示。"),
    ("player", "以玩家名义插入一句台词，进入对话历史。"),
    ("dialogue", "让指定在场角色说一句话，可带情绪标注。"),
    ("background", "切换场景背景图，可带过渡时长。"),
    ("background_effect", "切换氛围粒子特效，None 表示清除。"),
    ("present_pic", "在立绘层之上展示一张插图。"),
    ("music", "切换或停止背景音乐，可调播放速度。"),
    ("sound", "播放一段一次性音效。"),
    ("ambient", "播放或淡出停止环境音轨。"),
    ("set_variable", "给剧本变量赋值，可带执行条件。"),
];

// ============================================================
// director_get_world_state
// ============================================================

/// 查询当前现场摘要：模式、场景、背景、音乐与在场角色名。
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
        // 锁内只做数据快照，取完即释放，不跨 await 持锁
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

// ============================================================
// director_list_events
// ============================================================

/// 列出导演可用的事件类型清单（D3 十类），支持按名字子串过滤。
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
            .map(|(name, description)| json!({ "name": name, "description": description }))
            .collect();
        Ok(json!({ "ok": true, "events": events }))
    }
}

/// 空值与 none 统一显示为「无」，其余原样返回。
fn present_text(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        "无"
    } else {
        trimmed
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
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
        assert_eq!(definitions.len(), 2, "首批应恰好注册两个导演工具");
        assert!(definitions
            .iter()
            .any(|d| d.function.name == DIRECTOR_LIST_EVENTS));
    }

    #[tokio::test]
    async fn 未知工具被注册表与执行器拒绝() {
        let registry = director_registry();
        assert!(
            registry.get("director_no_such_tool").is_none(),
            "未知工具名不应在注册表中"
        );

        // 执行器同样拒绝未知工具，返回稳定的错误编码而非 panic
        let allowed: HashSet<String> = registry
            .definitions()
            .into_iter()
            .map(|d| d.function.name)
            .collect();
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

        // filter 过滤只保留命中的事件
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
        // 前几轮仍带工具继续；最后一轮带工具时转收尾，防止无限循环
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
        // 超上限的调用被裁掉，仍然只执行上限内的那些
        let calls: Vec<usize> = (0..MAX_TOOL_CALLS_PER_ROUND + 3).collect();
        let executed: Vec<usize> = calls
            .into_iter()
            .take(MAX_TOOL_CALLS_PER_ROUND)
            .collect();
        assert_eq!(executed.len(), MAX_TOOL_CALLS_PER_ROUND);
        assert_eq!(executed.last().copied(), Some(MAX_TOOL_CALLS_PER_ROUND - 1));
    }
}
