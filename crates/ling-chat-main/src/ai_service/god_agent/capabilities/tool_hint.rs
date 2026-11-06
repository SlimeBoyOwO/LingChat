//! 工具推荐：每轮用户发言前，把「这一轮可能用得上」的工具点出来。
//!
//! 面向的是「模型看得到工具、却想不起来用」这个常见毛病：Tool RAG 把工具收窄之后，
//! 不在本轮定义里的工具只能靠模型自己想起来去 `tool_search`，而多数时候它不会。
//!
//! 两条路（`ToolRecommendMode` 二选一，也可以都关掉）：
//! - `Agent`：上帝 Agent 看一遍最近的对话，按语义挑；
//! - `Rag`：拿玩家这一句话静默检索一次工具，分数过阈值的才留下。
//!
//! 两者都只产出一条临时记忆（走 `status_ops::RequestScoped`，只搭本轮请求、
//! 不写台词表、不进记忆），外加几个要并进本轮工具集的名字。推荐不构成授权：命中的
//! 名字在调用方（`tool_loop::activate_recommended`）还要过一遍权限集才会真正下发。
//!
//! 另有一条回复后的补充检索：生成结束时拿助手自己说过的正文与动作再检索一次，
//! 只暂存工具名，下一轮再由 [`hint_for_names`] 渲染成同样的提示文本。

use anyhow::Result;
use serde_json::Value;
use tauri::AppHandle;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::tools::agent::AgentTask;
use crate::ai_service::tools::builtin::god_agent::RECOMMEND_TOOLS;
use crate::ai_service::tools::tool_rag::{AUTO_RECOMMEND_MIN_SCORE, AUTO_RECOMMEND_TOP_K, ToolRag};
use crate::ai_service::types::{GameLine, ToolDefinition};
use crate::utils::prompt::PromptRole;

use crate::ai_service::god_agent::core::{GodAgentCore, tail_lines};

/// 最多点几件。再多就等于把工具清单原样塞回去，反而没有重点。
pub const MAX_RECOMMENDATIONS: usize = 5;

/// 推荐任务的输入视图（锁内取台词，锁外拼工具清单）。
pub struct ToolHintView {
    /// 最近若干条台词（由旧到新）。
    pub lines: Vec<GameLine>,
    /// (工具名, 用途)，已按权限过滤、且不含 tool_search 这类元工具。
    pub tools: Vec<(String, String)>,
}

impl ToolHintView {
    /// 锁内快照最近台词。工具清单依赖当前角色权限，由调用方算好再 [`with_tools`] 挂上。
    pub fn capture(gs: &GameStatus, window: usize) -> Self {
        Self {
            lines: tail_lines(gs, window),
            tools: Vec::new(),
        }
    }

    pub fn with_tools(mut self, tools: Vec<(String, String)>) -> Self {
        self.tools = tools;
        self
    }
}

/// 推荐结果：可以直接拼进请求的那句提示，外加要并进本轮工具集的名字。
pub struct ToolHint {
    pub text: String,
    pub names: Vec<String>,
}

/// 跑一次推荐。没有推荐（或模型没调用工具）时返回 `None`。
pub async fn recommend(
    core: &GodAgentCore,
    view: &ToolHintView,
    app: &AppHandle,
) -> Result<Option<ToolHint>> {
    if view.tools.is_empty() {
        return Ok(None);
    }
    let output = core.run_task(&build_task(view), app).await?;
    let Some(call) = output
        .calls
        .iter()
        .find(|call| call.name == RECOMMEND_TOOLS)
    else {
        return Ok(None);
    };
    if !call.ok {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(&call.result).unwrap_or(Value::Null);
    let picked: Vec<String> = value
        .get("names")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(render_hint(view, &picked))
}

/// RAG 推荐：拿玩家这一句话静默检索一次，分数过阈值的工具才推荐。
pub async fn recommend_by_rag(
    rag: &ToolRag,
    role: Option<&str>,
    query: &str,
    candidates: &[ToolDefinition],
) -> Result<Option<ToolHint>> {
    let hits = rag
        .search_and_activate(
            role,
            query,
            candidates,
            AUTO_RECOMMEND_TOP_K,
            AUTO_RECOMMEND_MIN_SCORE,
        )
        .await?;
    Ok(render_rag_hint(candidates, &hits))
}

/// 把命中的工具渲染成提示文本。命中为空（全都没过阈值）时不加任何提示。
fn render_rag_hint(candidates: &[ToolDefinition], hits: &[(String, f32)]) -> Option<ToolHint> {
    let names: Vec<String> = hits
        .iter()
        .take(AUTO_RECOMMEND_TOP_K)
        .map(|(name, _)| name.clone())
        .collect();
    render_names(candidates, &names)
}

/// 用一组已知的工具名渲染推荐提示。
///
/// 回复后推荐只暂存名字，下一轮再渲染：两轮之间角色可能切换，名字要先对回当前的
/// 候选集，渲染出来才不会是调不动的工具。
pub fn hint_for_names(candidates: &[ToolDefinition], names: &[String]) -> Option<ToolHint> {
    render_names(candidates, names)
}

/// 按名字渲染提示，对不上候选集的（或不在榜单前几位的）直接丢掉。
fn render_names(candidates: &[ToolDefinition], names: &[String]) -> Option<ToolHint> {
    let mut picked = Vec::new();
    for name in names.iter().take(AUTO_RECOMMEND_TOP_K) {
        if !candidates.iter().any(|def| &def.function.name == name) {
            continue;
        }
        picked.push(name.clone());
    }
    if picked.is_empty() {
        return None;
    }
    Some(ToolHint {
        text: hint_text(&picked),
        names: picked,
    })
}

/// 把挑中的名字对回工具清单，拼成提示。清单之外的名字丢弃（模型会编）。
fn render_hint(view: &ToolHintView, picked: &[String]) -> Option<ToolHint> {
    let mut names = Vec::new();
    for name in picked {
        if names.len() >= MAX_RECOMMENDATIONS {
            break;
        }
        if names.iter().any(|existing: &String| existing == name) {
            continue;
        }
        if !view.tools.iter().any(|(tool, _)| tool == name) {
            continue;
        }
        names.push(name.clone());
    }
    if names.is_empty() {
        return None;
    }
    Some(ToolHint {
        text: hint_text(&names),
        names,
    })
}

/// 与旁白/系统提示同一种信封：临时记忆注入后模型看到的是一条 `{旁白: （系统提示：…）}`，
/// 而不是 system 消息——后者会跟人设层抢位置，模型也几乎不感知。
fn hint_text(names: &[String]) -> String {
    PromptRole::System.build_prompt(&format!("参考推荐工具：{}", names.join("、")))
}

// ============================================================
// 请求构建
// ============================================================

fn build_task(view: &ToolHintView) -> AgentTask {
    let mut dialog = String::from("【最近对话（由旧到新）】\n");
    if view.lines.is_empty() {
        dialog.push_str("（无对话记录）\n");
    } else {
        for line in &view.lines {
            let name = line.base.display_name.as_deref().unwrap_or("未知");
            dialog.push_str(&format!("{}: {}\n", name, line.base.content));
        }
    }

    let mut menu = String::from("【本次可用的全部工具】\n");
    for (name, purpose) in &view.tools {
        menu.push_str(&format!("- {}：{}\n", name, purpose.trim()));
    }

    // 与发言者选择同一条约束：system 只放任务说明，数据放 user（单发 system 时
    // Codex 的 Responses API 会转出空 input 被拒）。
    AgentTask {
        system_prompt:
            "你是一个工具推荐助手。根据最近的对话，判断对话模型接下来可能用得上哪些工具。\
                       只从给出的工具清单里挑，最多 5 个；都不相关就把 names 留空。\
                       不要推荐清单以外的名字，也不要把「可能有用」的工具全列一遍。"
                .to_string(),
        payload: format!("{}\n{}", dialog, menu),
        tool_names: vec![RECOMMEND_TOOLS.to_string()],
        max_rounds: 1,
    }
}
