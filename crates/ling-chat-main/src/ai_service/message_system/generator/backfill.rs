//! 工具回填：保留前导台词行，并把载体行与工具结果插到对应前导组之后。
//!
//! 前导台词行由 consumer 在工具执行前逐句写入台词表。回填阶段要做三件事：
//! 1. 按句子序号重排本轮前导行（并发 consumer 的落库顺序不保证等于句子顺序）；
//! 2. 给每轮前导行、载体行与工具结果行打上同一个 `round_id`，让记忆构建器
//!    能把这一组结构化台词打包成一条带 tool_calls 的 assistant 消息，
//!    同时把上一轮遗留的回复挡在这条消息之外；
//! 3. 在每轮前导组之后插入「空内容载体行 + tool 结果行」。

use std::collections::HashMap;

use crate::ai_service::types::{GameLine, LineAttributeExt, LineBase, LlmMessage};
use crate::db::entities::line::LineAttribute;

/// 回填计划。所有位置都是传入 `lines` 快照的原始下标。
pub(super) struct ToolBackfillPlan {
    /// 同槽位替换：把前导组按句子序号重排。
    pub reorders: Vec<(usize, GameLine)>,
    /// 前导行打标：(原始行下标, 该轮 round_id)。
    pub round_ids: Vec<(usize, String)>,
    /// 插入块：(原始行下标, 载体行 + tool 结果行)。
    pub blocks: Vec<(usize, Vec<GameLine>)>,
}

/// 生成回填计划。
///
/// `tool_rounds` 是每轮工具调用的消息块（首条为 assistant），`preamble_groups`
/// 是每道栅栏之前发出的句子序号。两者按顺序配对；栅栏中止导致工具轮未入 sink 时，
/// 尾部多余的组会被忽略。
pub(super) fn plan_tool_backfill(
    lines: &[GameLine],
    turn_start: usize,
    ledger: &[(usize, usize)],
    preamble_groups: &[Vec<usize>],
    tool_rounds: &[Vec<LlmMessage>],
    perceived: &[i32],
) -> ToolBackfillPlan {
    let by_sentence: HashMap<usize, usize> = ledger.iter().copied().collect();

    // 每组解析出实际落成行的句子，并按句子序号排序
    let groups: Vec<Vec<(usize, usize)>> = preamble_groups
        .iter()
        .map(|group| {
            let mut rows: Vec<(usize, usize)> = group
                .iter()
                .filter_map(|sentence| by_sentence.get(sentence).map(|line| (*sentence, *line)))
                .filter(|(_, line)| *line < lines.len())
                .collect();
            rows.sort_unstable_by_key(|(sentence, _)| *sentence);
            rows
        })
        .collect();

    let mut reorders = Vec::new();
    for rows in &groups {
        if rows.len() < 2 {
            continue;
        }
        let mut slots: Vec<usize> = rows.iter().map(|(_, line)| *line).collect();
        slots.sort_unstable();
        slots.dedup();

        // 只在「物理行号连续、全是同一发送者的 assistant 行」时重排，
        // 避免误伤播放器、工具行、其他角色行或插在中间的系统行。
        let sender = lines[slots[0]].sender_role_id();
        let contiguous = slots.len() == rows.len()
            && slots.windows(2).all(|pair| pair[1] == pair[0] + 1)
            && slots.iter().all(|&slot| {
                matches!(lines[slot].attribute(), LineAttribute::Assistant)
                    && lines[slot].sender_role_id() == sender
            });
        if !contiguous {
            tracing::debug!("工具回填：前导行不连续或发送者不一致，跳过重排");
            continue;
        }
        let already_ordered = rows
            .iter()
            .zip(slots.iter())
            .all(|((_, line), slot)| line == slot);
        if already_ordered {
            continue;
        }
        for (&slot, (_, line)) in slots.iter().zip(rows.iter()) {
            reorders.push((slot, lines[*line].clone()));
        }
    }

    let mut round_ids = Vec::new();
    let mut blocks = Vec::new();
    let mut cursor = turn_start.min(lines.len());
    for (round, rows) in tool_rounds.iter().zip(groups.iter()) {
        let Some(carrier) = round.first().filter(|message| message.role == "assistant") else {
            continue;
        };
        let last_row = rows.iter().map(|(_, line)| *line).max();
        let pos = match last_row {
            Some(line) => {
                if line + 1 < cursor {
                    tracing::warn!(
                        last_row = line,
                        cursor,
                        "工具回填：前导行晚于后续轮次，块位置回退到 cursor"
                    );
                }
                (line + 1).max(cursor)
            },
            None => cursor,
        };
        cursor = pos;

        // 同一轮的前导、载体与工具结果共享一个标识，记忆构建器据此精确分组。
        let round_id = uuid::Uuid::new_v4().to_string();
        for (_, line) in rows {
            round_ids.push((*line, round_id.clone()));
        }

        // 组内有结构化行时载体只做协议占位；整组没落行时保留原文，避免丢文本。
        let content = if rows.is_empty() {
            carrier.content.clone()
        } else {
            String::new()
        };
        let tool_call = carrier
            .tool_calls
            .as_ref()
            .map(|calls| serde_json::to_string(calls).unwrap_or_default());

        let mut block = Vec::with_capacity(round.len());
        block.push(GameLine::from_base(
            LineBase {
                content,
                tool_call,
                round_id: Some(round_id.clone()),
                attribute: LineAttributeExt(LineAttribute::Assistant),
                sender_role_id: None,
                display_name: None,
                ..Default::default()
            },
            perceived.to_vec(),
        ));
        for message in &round[1..] {
            if message.role != "tool" {
                continue;
            }
            block.push(GameLine::from_base(
                LineBase {
                    content: serde_json::to_string(&serde_json::json!({
                        "tool_call_id": message.tool_call_id,
                        "result": serde_json::from_str::<serde_json::Value>(&message.content)
                            .unwrap_or(serde_json::Value::String(message.content.clone())),
                    }))
                    .unwrap_or_default(),
                    round_id: Some(round_id.clone()),
                    attribute: LineAttributeExt(LineAttribute::Tool),
                    sender_role_id: None,
                    display_name: None,
                    ..Default::default()
                },
                perceived.to_vec(),
            ));
        }
        blocks.push((pos, block));
    }

    if tool_rounds.len() > groups.len() {
        tracing::warn!(
            rounds = tool_rounds.len(),
            groups = groups.len(),
            "工具回填：工具轮数多于前导组数，忽略多余轮次"
        );
    }

    ToolBackfillPlan {
        reorders,
        round_ids,
        blocks,
    }
}
