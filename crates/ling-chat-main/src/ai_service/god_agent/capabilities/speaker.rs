//! 发言者选择能力：状态快照 + 决策 prompt + 结果解读。
//!
//! 发言权的实际切换由 `tools::builtin::god_agent::SelectNextSpeaker` 在执行时完成，
//! 所以这里的产物只有「谁被选中」这一条信息。

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use serde_json::Value;
use tauri::AppHandle;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::player_identity::{
    RelationEndpoint, effective_user_alias, resolve_relation, runtime_relations,
};
use crate::ai_service::tools::agent::{AgentOutput, AgentTask};
use crate::ai_service::tools::builtin::god_agent::SELECT_NEXT_SPEAKER;
use crate::ai_service::types::GameLine;

use crate::ai_service::god_agent::core::{
    GodAgentCore, PeerMember, PeerNote, peer_notes, tail_lines,
};

// ============================================================
// 状态快照
// ============================================================

/// 一个在场 NPC 的展示信息（含该角色对玩家的称呼 / 认知 / 好感，供选人判断）。
pub struct NpcInfo {
    pub role_id: i32,
    pub name: String,
    pub subtitle: String,
    pub info: String,
    pub player_alias: String,
    /// 玩家对 ta 的认知（身份卡 relations 里 `ai:<folder>` 的值，如"妹妹"）。
    pub player_view_of_npc: Option<String>,
    pub player_relation: Option<String>,
    pub player_affection_tier: Option<String>,
    /// 该角色对在场其他人的认知（说话者侧优先、对方侧回退；没写的跳过）。
    pub peer_relations: Vec<PeerNote>,
}

/// 选人任务的输入视图（锁内取，避免持锁调 LLM）。
pub struct SpeakerView {
    pub npcs: Vec<NpcInfo>,
    /// 玩家身份名（旁白与系统提示里使用的公共名）。
    pub player_identity_name: String,
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
        let data_dir = crate::api::data_dir();
        let identity_name = gs.player.user_name.clone();
        let identity_id = gs.player.identity_id.clone();

        // 两遍：先收在场成员与各自的有效关系表（每角色读盘一次），再两两解析角色间认知。
        let mut npcs: Vec<NpcInfo> = Vec::new();
        let mut members: Vec<PeerMember> = Vec::new();
        for &role_id in gs.present_role_ids.iter().filter(|&&id| id != 0) {
            let Some(role) = gs.role_manager.get_loaded(role_id) else {
                continue;
            };
            let folder = role.settings.character_folder.clone();
            let relations = runtime_relations::load_effective(&data_dir, &folder, gs);
            let name = role
                .display_name
                .clone()
                .unwrap_or_else(|| format!("角色{role_id}"));
            let player_alias = effective_user_alias(&role.settings, &identity_name);
            // 玩家对该角色的认知（身份卡 relations，内存快照），如"妹妹"。
            let player_view_of_npc = gs
                .player
                .relations
                .get(&RelationEndpoint::Ai(folder.clone()).key())
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty());
            let player_relation = resolve_relation(
                &RelationEndpoint::Ai(folder.clone()),
                &RelationEndpoint::Me(identity_id.clone().unwrap_or_default()),
                &relations,
                &HashMap::new(),
                None,
            )
            .map(|view| view.text);
            let player_affection_tier = crate::ai_service::affection::resolve_var(
                &gs.global_variables,
                role_id,
                identity_id.as_deref(),
            )
            .and_then(crate::ai_service::affection::state_from_value)
            .map(|state| {
                crate::ai_service::affection::tier_label(state.vector.average()).to_string()
            });
            npcs.push(NpcInfo {
                role_id,
                name: name.clone(),
                subtitle: role.settings.ai_subtitle.clone().unwrap_or_default(),
                info: role.settings.info.clone().unwrap_or_default(),
                player_alias,
                player_view_of_npc,
                player_relation,
                player_affection_tier,
                peer_relations: Vec::new(),
            });
            members.push(PeerMember {
                folder,
                name,
                relations,
            });
        }
        for (idx, npc) in npcs.iter_mut().enumerate() {
            npc.peer_relations = peer_notes(&members, idx);
        }

        Self {
            npcs,
            player_identity_name: identity_name,
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

    // --- 玩家身份（称呼 / 认知 / 好感）---
    let mut player_block = format!(
        "【玩家身份（这场对话中的「我」）】\n- 身份名：{}\n",
        view.player_identity_name
    );
    for npc in &view.npcs {
        let alias = if npc.player_alias.is_empty() {
            &view.player_identity_name
        } else {
            &npc.player_alias
        };
        let mut line = format!("- {}：私下称呼 ta「{}」", npc.name, alias);
        if let Some(view) = &npc.player_view_of_npc {
            line.push_str(&format!("；玩家对 ta 的认知：{}", view));
        }
        if let Some(relation) = &npc.player_relation {
            line.push_str(&format!("；对 ta 的认知（关系）：{}", relation));
        }
        if let Some(tier) = &npc.player_affection_tier {
            line.push_str(&format!("；对 ta 的好感：{}", tier));
        }
        line.push('\n');
        if !npc.peer_relations.is_empty() {
            let parts: Vec<String> = npc
                .peer_relations
                .iter()
                .map(|(name, text, counterparty)| {
                    if *counterparty {
                        format!("{name}——{text}（是 ta 的说法，未必是事实）")
                    } else {
                        format!("{name}——{text}")
                    }
                })
                .collect();
            line.push_str(&format!("  对场内其他人的认知：{}", parts.join("；")));
            line.push('\n');
        }
        player_block.push_str(&line);
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
        system_prompt: "你是一个多人对话的导演（上帝视角）。你的任务是：根据当前场景中的角色列表、各角色的关系与认知数据和最近的对话历史，判断下一个应该发言的角色；做出判断后，调用 select_next_speaker 工具。\n\
            分配发言权时的判断优先级（务必按此顺序，不要只凭台词文本或角色设定风格来分配）：\n\
            1. 玩家点名或称呼了谁——包括外号与亲属称谓等别称（如「妹妹」），以「玩家身份」表里的「玩家对 ta 的认知」和「私下称呼」为准；不确定归属时不要猜测。\n\
            2. 谁与玩家、或与上一句发言者的关系与情感使 ta 最有接话动机——结合每个角色的「对 ta 的认知（关系）」「对场内其他人的认知」「对 ta 的好感」判断：关系越近、情绪越强（暗恋、别扭、竞争、敌意等）越应优先。\n\
            3. 话题内容只决定接话之后说什么，不决定由谁来说。\n\
            补充：各角色对玩家的私下称呼可能不同，对话记录里玩家行以身份名标注——不同称呼指的是同一个玩家，不要当成不同的人；台词文本的表面措辞不代表归属，关系数据才是分配的主要依据。".to_string(),
        payload: format!(
            "{}\n{}\n{}\n{}",
            role_info_block, player_block, dialog_block, current_hint
        ),
        tool_names: vec![SELECT_NEXT_SPEAKER.to_string()],
        // 单发决策：选完即止，结果不再回填给 LLM。
        max_rounds: 1,
    }
}

/// 从运行产物解读决策。
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
