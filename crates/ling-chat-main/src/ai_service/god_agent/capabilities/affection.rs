//! 好感度评估能力：状态快照 + 评估 prompt + 结果日志。
//!
//! 情感维度的调整、存档落点、事件广播与旁白台词由
//! `tools::builtin::god_agent::UpdateAffection` 在执行时完成。

use std::collections::HashMap;

use anyhow::Result;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::player_identity::{
    RelationEndpoint, resolve_relation, runtime_relations,
};
use crate::ai_service::game_system::status_ops::{RecentMemory, RecentMemoryAnchor};
use crate::ai_service::tools::agent::{AgentOutput, AgentTask};
use crate::ai_service::tools::builtin::god_agent::UPDATE_AFFECTION;
use crate::ai_service::tools::game_status_handle;
use crate::ai_service::types::{AffectionVector, GameLine, NegativeVector};
use crate::utils::prompt::PromptRole;

use crate::ai_service::god_agent::core::{GodAgentCore, tail_lines};

// ============================================================
// 状态快照
// ============================================================

/// 在场 NPC 的好感度评估输入视图（锁内取，避免持锁调 LLM）。
pub struct NpcAffectionView {
    pub role_id: i32,
    pub name: String,
    pub subtitle: String,
    pub info: String,
    pub current: AffectionVector,
    /// 当前负面情绪六维强度（评估参考：安抚后应减回去）。
    pub negative: NegativeVector,
    /// 该角色对玩家身份的认知（说话者侧关系，可为空）。
    pub player_relation: Option<String>,
}

impl NpcAffectionView {
    /// 锁内快照：在场且已加载的 NPC，以及最近若干条台词。
    pub fn capture_all(
        gs: &GameStatus,
        recent_window: usize,
    ) -> (Vec<NpcAffectionView>, Vec<GameLine>) {
        let npcs = gs
            .present_role_ids
            .iter()
            .filter(|&&id| id != 0)
            .filter_map(|&id| gs.role_manager.get_loaded(id))
            .filter_map(|r| {
                let role_id = r.role_id?;
                let folder = r.settings.character_folder.clone();
                let player_relation = resolve_relation(
                    &RelationEndpoint::Ai(folder.clone()),
                    &RelationEndpoint::Me(gs.player.identity_id.clone().unwrap_or_default()),
                    &runtime_relations::load_effective(&crate::api::data_dir(), &folder, gs),
                    &HashMap::new(),
                    None,
                )
                .map(|view| view.text);
                Some(NpcAffectionView {
                    role_id,
                    name: r
                        .display_name
                        .clone()
                        .unwrap_or_else(|| format!("角色{role_id}")),
                    subtitle: r.settings.ai_subtitle.clone().unwrap_or_default(),
                    info: r.settings.info.clone().unwrap_or_default(),
                    current: r.affection,
                    negative: r.negative,
                    player_relation,
                })
            })
            .collect();

        (npcs, tail_lines(gs, recent_window))
    }
}

// ============================================================
// 入口
// ============================================================

/// 评估最近对话对在场 NPC 好感度的影响。
///
/// 调整、落档与广播由 `update_affection` 工具完成；工具返回的旁白文本交给近期
/// 记忆队列，锚定到 `anchor` 指向的那一轮。
pub async fn evaluate_affection(
    core: &GodAgentCore,
    npcs: &[NpcAffectionView],
    lines: &[GameLine],
    anchor: Option<RecentMemoryAnchor>,
    app: &AppHandle,
) -> Result<()> {
    if npcs.is_empty() {
        return Ok(());
    }

    let task = build_task(npcs, lines);
    let output = core.run_task(&task, app).await?;
    queue_recent_memories(&output, anchor, app).await;
    log_output(&output);
    Ok(())
}

/// 把评估产出的好感度旁白交给近期记忆队列，等生成锁空闲再落位。
async fn queue_recent_memories(
    output: &AgentOutput,
    anchor: Option<RecentMemoryAnchor>,
    app: &AppHandle,
) {
    let memories: Vec<RecentMemory> = output
        .calls
        .iter()
        .filter(|call| call.name == UPDATE_AFFECTION && call.ok)
        .filter_map(|call| {
            let line_text = serde_json::from_str::<serde_json::Value>(&call.result)
                .ok()?
                .get("line_text")
                .and_then(|text| text.as_str())
                .map(str::to_string)?;
            Some(RecentMemory {
                text: line_text,
                role: PromptRole::Narrator,
                anchor: anchor.clone(),
            })
        })
        .collect();
    if memories.is_empty() {
        return;
    }

    {
        let game_status = game_status_handle(app).await;
        let mut gs = game_status.lock().await;
        for memory in memories {
            gs.queue_recent_memory(memory);
        }
    }

    // 锁被占时（下一轮正在生成）等这一轮结束再落位，位置仍是上一轮 user 块。
    let state = app.state::<AppState>();
    let gen_lock = state.generation_lock.clone();
    let _guard = gen_lock.lock().await;
    let db = state.db.clone();
    let game_status = game_status_handle(app).await;
    let mut gs = game_status.lock().await;
    gs.flush_recent_memories(&db).await;
}

// ============================================================
// prompt 构建
// ============================================================

fn build_task(npcs: &[NpcAffectionView], lines: &[GameLine]) -> AgentTask {
    let mut npc_block = String::from("【在场角色与当前情感状态】\n");
    for npc in npcs {
        let dims = AffectionVector::DIMENSIONS
            .iter()
            .map(|(key, label)| format!("{} {}", label, npc.current.get(key).unwrap_or(0)))
            .collect::<Vec<_>>()
            .join("、");
        let neg_dims = NegativeVector::DIMENSIONS
            .iter()
            .map(|(key, label)| format!("{} {}", label, npc.negative.get(key).unwrap_or(0)))
            .collect::<Vec<_>>()
            .join("、");
        npc_block.push_str(&format!(
            "- role_id={}: {}\n  简介: {}\n  设定: {}\n  当前情感（数值可超过 100，负数为疏离）: {}\n  当前负面情绪（0 为无，越高越强烈）: {}\n",
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
            dims,
            neg_dims,
        ));
        if let Some(relation) = &npc.player_relation {
            npc_block.push_str(&format!("  对玩家的认知（关系）: {}\n", relation));
        }
    }

    let mut dialog_block = String::from("【最近对话记录（由旧到新）】\n");
    if lines.is_empty() {
        dialog_block.push_str("（无对话记录）\n");
    } else {
        for line in lines {
            let name = line.base.display_name.as_deref().unwrap_or("未知");
            let content = &line.base.content;
            dialog_block.push_str(&format!("{}: {}\n", name, content));
        }
    }

    // system 只放任务说明与评估原则，数据载荷放 user——只发单条 system
    // 消息时，Codex（Responses API）会转换出空 input 被 400 拒绝。
    AgentTask {
        system_prompt: "你是一个情感观察员（上帝视角）。请阅读用户给出的最近对话，评估这段对话对在场角色情感状态的影响。\n\
             \n\
             好感维度含义：\n\
             - 好感 fondness：整体喜欢程度，影响语气甜度\n\
             - 信赖 trust：愿意倾诉与说真心话的程度\n\
             - 亲密 intimacy：对肢体接触/近距离互动的接受度\n\
             - 默契 rapport：接梗、理解言外之意的程度\n\
             - 兴趣 interest：对玩家话题的好奇与主动程度\n\
             - 思念 longing：分别时的挂念强度（即将分别、久别时增加，重逢或相处愉快时回落）\n\
             \n\
             负面情绪维度含义（0 为无，越高越强烈，下限 0）：\n\
             - 愤怒 anger：被冒犯时的火气\n\
             - 受伤 hurt：被刺痛、委屈\n\
             - 失望 disappointment：期待落空\n\
             - 冷漠 indifference：敷衍、不在乎的态度\n\
             - 嫉妒 jealousy：玩家关注别人时的吃醋\n\
             - 疏远 estrangement：想保持距离\n\
             \n\
             评估原则：日常正面互动 +1~+2，明显打动/冒犯 ±3，非常深刻或严重伤害 ±4~±5；\
             没有受到这段对话影响的维度不要调整；变化要符合角色性格，保守为主、宁少勿多。\n\
             负面情绪：被冒犯/忽视/伤害时用 negative_deltas 增加对应维度；\
             被安抚/取悦/温柔对待时减少对应维度（往 0 消解）；没有变化则不填。\n\
             请对每个有情感变化的在场角色调用一次 update_affection 工具。"
            .to_string(),
        payload: format!("{}\n{}", npc_block, dialog_block),
        tool_names: vec![UPDATE_AFFECTION.to_string()],
        // 单发决策：每个受影响的角色各调一次工具，结果不再回填给 LLM。
        max_rounds: 1,
    }
}

// ============================================================
// 结果日志
// ============================================================

/// 记录一次评估的结果。
///
/// 评估跑在后台 spawn 里，失败只记日志、不向上传播——避免一次 LLM 抖动影响
/// 已经呈现给用户的回复。「无事可做」的跳过不算失败，不记。
fn log_output(output: &AgentOutput) {
    if output.calls.is_empty() {
        tracing::warn!("上帝Agent 好感度评估未返回 tool_calls");
        return;
    }
    for call in &output.calls {
        if call.name != UPDATE_AFFECTION || call.ok || call.skipped() {
            continue;
        }
        tracing::warn!("[Affection] 调整未应用: {}", call.error_message());
    }
}
