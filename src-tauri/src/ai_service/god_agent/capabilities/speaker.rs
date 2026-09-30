//! 发言者选择能力：状态快照 + 决策 prompt + 结果解读。
//!
//! 发言权的实际切换由 `tools::god_agent::SelectNextSpeaker` 在执行时完成，
//! 所以这里的产物只有「谁被选中」这一条信息。

use anyhow::{Result, anyhow};
use serde_json::Value;
use tauri::AppHandle;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::tools::agent::{AgentOutput, AgentTask};
use crate::ai_service::tools::god_agent::SELECT_NEXT_SPEAKER;
use crate::ai_service::types::GameLine;

use crate::ai_service::god_agent::core::{GodAgentCore, tail_lines};

// ============================================================
// 状态快照
// ============================================================

/// 一个在场 NPC 的展示信息。
pub struct NpcInfo {
    pub role_id: i32,
    pub name: String,
    pub subtitle: String,
    pub info: String,
}

/// 选人任务的输入视图（锁内取，避免持锁调 LLM）。
pub struct SpeakerView {
    pub npcs: Vec<NpcInfo>,
    /// 最近若干条台词（由旧到新）。
    pub lines: Vec<GameLine>,
    /// 快照时的当前发言者；`None` 表示尚未有人发言。
    pub current_speaker: Option<i32>,
    /// 当前发言者的展示名；发言者是玩家或未加载时为空。
    pub current_speaker_name: Option<String>,
}

impl SpeakerView {
    /// 锁内快照。`recent_window` 决定截取多少条台词。
    pub fn capture(gs: &GameStatus, recent_window: usize) -> Self {
        let npcs = gs
            .present_role_ids
            .iter()
            .filter(|&&id| id != 0)
            .map(|&role_id| {
                let loaded = gs.role_manager.get_loaded(role_id);
                NpcInfo {
                    role_id,
                    name: loaded
                        .and_then(|r| r.display_name.clone())
                        .unwrap_or_else(|| format!("角色{role_id}")),
                    subtitle: loaded
                        .and_then(|r| r.settings.ai_subtitle.clone())
                        .unwrap_or_default(),
                    info: loaded
                        .and_then(|r| r.settings.info.clone())
                        .unwrap_or_default(),
                }
            })
            .collect();

        Self {
            npcs,
            lines: tail_lines(gs, recent_window),
            current_speaker: gs.current_role_id,
            current_speaker_name: gs
                .current_role_id
                .filter(|&id| id != 0)
                .and_then(|id| gs.role_manager.get_loaded(id))
                .and_then(|r| r.display_name.clone()),
        }
    }

    /// 在场 NPC 的 role_id（不含玩家 0）。
    pub fn npc_ids(&self) -> Vec<i32> {
        self.npcs.iter().map(|n| n.role_id).collect()
    }
}

// ============================================================
// 入口
// ============================================================

/// 一次发言者决策的结果。
pub struct SpeakerDecision {
    pub role_id: i32,
    /// 目标角色的展示名；工具未回传时为空。
    pub name: Option<String>,
    pub reason: String,
}

/// 决定下一个发言者。单 NPC 时直接返回，不打 LLM。
pub async fn decide_next_speaker(
    core: &GodAgentCore,
    view: &SpeakerView,
    app: &AppHandle,
) -> Result<SpeakerDecision> {
    let npc_ids = view.npc_ids();
    if npc_ids.len() <= 1 {
        return Ok(SpeakerDecision {
            role_id: npc_ids.first().copied().unwrap_or(0),
            name: view.npcs.first().map(|npc| npc.name.clone()),
            reason: "single_npc".into(),
        });
    }

    let task = build_task(view);
    let output = core.run_task(&task, app).await?;
    parse_output(&output)
}

// ============================================================
// prompt 构建
// ============================================================

fn build_task(view: &SpeakerView) -> AgentTask {
    // --- 角色信息 ---
    let mut role_info_block = String::from("【当前在场的非玩家角色列表】\n");
    for npc in &view.npcs {
        role_info_block.push_str(&format!(
            "- role_id={}: {}\n  简介: {}\n  设定: {}\n",
            npc.role_id,
            npc.name,
            if npc.subtitle.is_empty() {
                "无"
            } else {
                &npc.subtitle
            },
            if npc.info.is_empty() {
                "无"
            } else {
                &npc.info
            },
        ));
    }

    // --- 最近对话 ---
    let mut dialog_block = String::from("【最近对话记录（由旧到新）】\n");
    if view.lines.is_empty() {
        dialog_block.push_str("（无对话记录）\n");
    } else {
        for line in &view.lines {
            let name = line.base.display_name.as_deref().unwrap_or("未知");
            let sid = line.base.sender_role_id.unwrap_or(-1);
            let emotion = line
                .base
                .original_emotion
                .as_deref()
                .filter(|v| !v.is_empty())
                .map(|v| format!("【{}】", v))
                .unwrap_or_default();
            let content = &line.base.content;
            dialog_block.push_str(&format!(
                "[role_id={}] {}: {}{}\n",
                sid, name, emotion, content
            ));
        }
    }

    // --- 当前发言者提示 ---
    let current_hint = match view.current_speaker {
        Some(0) => "当前发言者是「玩家」。请选择下一个发言的 NPC 角色。\n".to_string(),
        Some(rid) => {
            let name = view
                .current_speaker_name
                .clone()
                .unwrap_or_else(|| format!("角色{}", rid));
            format!(
                "当前发言者是「{}」(role_id={})，刚刚说完话。请判断：\n- 如果对话应该继续（比如另一个角色有强烈反应或话题未完），选择下一个发言的 NPC\n- 如果应该交还给玩家，选择 role_id=0\n",
                name, rid
            )
        },
        None => String::new(),
    };

    // system 只放任务说明，数据载荷放 user——只发单条 system 消息时，
    // Codex（Responses API）会转换出空 input 被 400 拒绝
    // （"One of input or ... must be provided"）。
    AgentTask {
        system_prompt: "你是一个多人对话的导演（上帝视角）。你的任务是：根据当前场景中的角色列表和最近的对话历史，判断下一个应该发言的角色。做出判断后，调用 select_next_speaker 工具来选择下一个发言者。".to_string(),
        payload: format!("{}\n{}\n{}", role_info_block, dialog_block, current_hint),
        tool_names: vec![SELECT_NEXT_SPEAKER.to_string()],
        // 单发决策：选完即止，结果不再回填给 LLM。
        max_rounds: 1,
    }
}

// ============================================================
// 结果解读
// ============================================================

/// 从运行产物解读决策。
///
/// 执行器会把一切错误吞成 `{"ok":false,...}`，而调用方依赖「决策失败即终止本轮
/// 生成」的语义（见 `MessageGenerator::process_message` 的 `?`），所以这里必须把
/// 失败重新提升为 `Err`，否则会静默退化成「沿用旧发言者」。
fn parse_output(output: &AgentOutput) -> Result<SpeakerDecision> {
    let Some(call) = output
        .calls
        .iter()
        .find(|call| call.name == SELECT_NEXT_SPEAKER)
    else {
        let content_info = if output.text.trim().is_empty() {
            String::new()
        } else {
            format!("，返回文本: {}", output.text)
        };
        return Err(anyhow!("上帝Agent 未调用工具{}", content_info));
    };

    if !call.ok {
        return Err(anyhow!(
            "上帝Agent 选择发言者失败: {}",
            call.error_message()
        ));
    }

    let value: Value = serde_json::from_str(&call.result).unwrap_or(Value::Null);
    let role_id = value
        .get("role_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow!("解析 select_next_speaker 结果失败: {}", call.result))?
        as i32;

    Ok(SpeakerDecision {
        role_id,
        name: value
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        reason: value
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("（无理由）")
            .to_string(),
    })
}
