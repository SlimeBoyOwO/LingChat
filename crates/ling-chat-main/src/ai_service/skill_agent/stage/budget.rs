//! 上下文预算：估算 token、折叠旧工具结果、超限时的收尾判定。

use super::evidence::chapter_id_of_path;
use super::{Stage, StageSnapshot};
use crate::ai_service::types::LlmMessage;

/// 丢弃已被超越的章节写入轮次，避免历史里堆积整章 YAML；整轮丢弃以保持历史结构合法。
pub fn compact_history(history: Vec<LlmMessage>, snap: &StageSnapshot) -> Vec<LlmMessage> {
    if snap.stage != Stage::Forge {
        return history;
    }
    let current = snap.next_chapter();
    let mut out = Vec::with_capacity(history.len());
    let mut i = 0;
    while i < history.len() {
        if history[i].role == "assistant" && history[i].tool_calls.is_some() {
            let mut j = i + 1;
            while j < history.len() && history[j].role == "tool" {
                j += 1;
            }
            if !is_superseded_chapter_write(&history[i], snap, current) {
                out.extend_from_slice(&history[i..j]);
            }
            i = j;
        } else {
            out.push(history[i].clone());
            i += 1;
        }
    }
    out
}

fn is_superseded_chapter_write(
    msg: &LlmMessage,
    snap: &StageSnapshot,
    current: Option<&str>,
) -> bool {
    let Some(calls) = msg.tool_calls.as_ref().filter(|c| !c.is_empty()) else {
        return false;
    };
    calls.iter().all(|tc| {
        written_chapter_id(&tc.function.name, &tc.function.arguments)
            .is_some_and(|id| Some(id.as_str()) != current && snap.written.iter().any(|w| w == &id))
    })
}

/// 单次请求给模型输出留的余量上限：实际预留取 `min(128K, 窗口/8)`，见 [`input_cap`]。
pub const OUTPUT_RESERVE_TOKENS: usize = 128 * 1024;

/// 窗口读不到时的兜底 1,048,576（1M）：宁可少收束也不丢记忆，真超限由 [`BudgetOutcome::TooLong`] 兜住。
pub const DEFAULT_CONTEXT_WINDOW: usize = 1_048_576;

const TRIGGER_PERCENT: usize = 80;

const MIN_FOLD_CHARS: usize = 1200;

/// 最近几轮原文保留（正在进行的事，不能压）。
const KEEP_RECENT_TURNS: usize = 2;

/// 估算 token：CJK 按 0.85 token/字、其余按 0.3，每条消息再加 4；故意估高，估低会撞窗口。
pub fn estimate_tokens(messages: &[LlmMessage]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(msg: &LlmMessage) -> usize {
    let mut wide = 0usize;
    let mut narrow = 0usize;
    let mut count = |text: &str| {
        for ch in text.chars() {
            if is_wide_char(ch) {
                wide += 1;
            } else {
                narrow += 1;
            }
        }
    };
    count(&msg.content);
    if let Some(calls) = msg.tool_calls.as_deref() {
        for call in calls {
            count(&call.function.name);
            count(&call.function.arguments);
        }
    }
    wide * 85 / 100 + narrow * 3 / 10 + 4
}

fn is_wide_char(ch: char) -> bool {
    matches!(ch as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetOutcome {
    Untouched {
        tokens: usize,
    },
    Trimmed {
        before: usize,
        after: usize,
        folded_reads: usize,
        digested_turns: usize,
    },
    /// 丢到不能再丢仍然超硬线：调用方必须主动报错，不许发请求。
    TooLong {
        tokens: usize,
        cap: usize,
    },
}

impl BudgetOutcome {
    pub fn note(&self) -> Option<String> {
        match self {
            BudgetOutcome::Untouched { .. } => None,
            BudgetOutcome::Trimmed {
                before,
                after,
                folded_reads,
                digested_turns,
            } => {
                let mut did: Vec<String> = Vec::new();
                if *folded_reads > 0 {
                    did.push(format!("折叠 {folded_reads} 条历史只读结果"));
                }
                if *digested_turns > 0 {
                    did.push(format!("把 {digested_turns} 轮更早的对话压成摘要"));
                }
                Some(format!(
                    "上下文偏长（约 {before} → {after} token），已{}（你的原话与最近几轮都完整保留）",
                    did.join("、")
                ))
            },
            BudgetOutcome::TooLong { tokens, cap } => Some(format!(
                "这个对话的上下文满了（约 {tokens} token，上限 {cap}）：已无法再自动压缩。\
                 请**新开一个对话**继续 —— 剧本文件都在磁盘上，新会话里照样能接着改。"
            )),
        }
    }
}

/// 按窗口收束一次，到水位才动手；丢不下去返回 [`BudgetOutcome::TooLong`]。顺序（先丢最不可惜的）：
pub fn apply_context_budget(messages: &mut Vec<LlmMessage>, window: usize) -> BudgetOutcome {
    let cap = input_cap(window);
    let trigger = cap * TRIGGER_PERCENT / 100;

    let before = estimate_tokens(messages);
    if before < trigger {
        return BudgetOutcome::Untouched { tokens: before };
    }

    let protected_from = recent_turns_start(messages, KEEP_RECENT_TURNS);

    let folded_reads = fold_old_readonly_results(messages, protected_from);
    let after_fold = estimate_tokens(messages);
    if after_fold <= cap {
        return if folded_reads == 0 {
            BudgetOutcome::Untouched { tokens: before }
        } else {
            BudgetOutcome::Trimmed {
                before,
                after: after_fold,
                folded_reads,
                digested_turns: 0,
            }
        };
    }

    let digested_turns = digest_older_turns(messages, protected_from);
    let after_digest = estimate_tokens(messages);
    if after_digest <= cap {
        return BudgetOutcome::Trimmed {
            before,
            after: after_digest,
            folded_reads,
            digested_turns,
        };
    }

    BudgetOutcome::TooLong {
        tokens: after_digest,
        cap,
    }
}

pub fn budget_allows_send(outcome: &BudgetOutcome) -> bool {
    !matches!(outcome, BudgetOutcome::TooLong { .. })
}

/// 输入硬线：窗口减去输出余量（预留取 `min(128K, 窗口/8)`），必须随窗口缩小。
fn input_cap(window: usize) -> usize {
    let reserve = OUTPUT_RESERVE_TOKENS.min(window / 8);
    window.saturating_sub(reserve).max(1)
}

/// 从末尾往前数 `keep` 个"轮"的起点下标；第 0 条（system）与第一个 user 之前的内容永远在保护区。
pub(crate) fn recent_turns_start(messages: &[LlmMessage], keep: usize) -> usize {
    let mut seen = 0usize;
    for (i, m) in messages.iter().enumerate().rev() {
        if m.role == "user" {
            seen += 1;
            if seen == keep {
                return i;
            }
        }
    }
    0
}

pub(crate) fn turn_ranges(messages: &[LlmMessage]) -> Vec<(usize, usize)> {
    let starts: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(i, m)| *i > 0 && m.role == "user")
        .map(|(i, _)| i)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| (s, starts.get(n + 1).copied().unwrap_or(messages.len())))
        .collect()
}

fn call_meta(messages: &[LlmMessage]) -> std::collections::HashMap<String, (String, String)> {
    let mut out = std::collections::HashMap::new();
    for m in messages {
        let Some(calls) = m.tool_calls.as_deref() else {
            continue;
        };
        for call in calls {
            out.insert(
                call.id.clone(),
                (
                    call.function.name.clone(),
                    arg_path(&call.function.arguments),
                ),
            );
        }
    }
    out
}

fn arg_path(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|v| v.get("path").and_then(|p| p.as_str()).map(str::to_string))
        .unwrap_or_default()
}

/// 只读且随时可以再读一次的工具：结果丢了不损失能力。
fn is_rereadable_tool(name: &str) -> bool {
    matches!(name, "read_file" | "list_files")
}

fn fold_line(name: &str, path: &str, chars: usize) -> String {
    let what = if path.is_empty() {
        String::new()
    } else {
        format!(" {path}")
    };
    format!("[已折叠] {name}{what}（原 {chars} 字符）。要看细节就再 {name} 一次。")
}

/// ① 把保护区之外的旧只读结果折成一行：只换 `content` 不删消息，`assistant(tool_calls)` 与 `tool` 回应必须成对。
fn fold_old_readonly_results(messages: &mut [LlmMessage], protected_from: usize) -> usize {
    let meta = call_meta(messages);
    let mut folded = 0usize;
    for (i, msg) in messages.iter_mut().enumerate() {
        if i >= protected_from || msg.role != "tool" {
            continue;
        }
        let Some(id) = msg.tool_call_id.as_deref() else {
            continue;
        };
        let Some((name, path)) = meta.get(id) else {
            continue;
        };
        if !is_rereadable_tool(name) {
            continue;
        }
        let chars = msg.content.chars().count();
        if chars < MIN_FOLD_CHARS {
            continue;
        }
        msg.content = fold_line(name, path, chars);
        folded += 1;
    }
    folded
}

/// ② 把保护区之外的每一轮压成一行摘要：user 原话保留原文，其余合并成一条 assistant 摘要。
fn digest_older_turns(messages: &mut Vec<LlmMessage>, protected_from: usize) -> usize {
    let ranges = turn_ranges(messages);
    let mut drop = vec![false; messages.len()];
    let mut digests: Vec<(usize, String)> = Vec::new();
    for (start, end) in ranges {
        if start >= protected_from || messages[start].role != "user" {
            continue;
        }
        let Some(digest) = turn_digest(&messages[start..end]) else {
            continue;
        };
        for flag in drop.iter_mut().take(end).skip(start + 1) {
            *flag = true;
        }
        digests.push((start + 1, digest));
    }
    if digests.is_empty() {
        return 0;
    }

    let count = digests.len();
    let mut out = Vec::with_capacity(messages.len());
    for (i, msg) in messages.drain(..).enumerate() {
        if !drop[i] {
            out.push(msg);
        }
        if let Some((_, text)) = digests.iter().find(|(at, _)| *at == i) {
            out.push(LlmMessage::assistant(text.clone()));
        }
    }
    *messages = out;
    count
}

/// 一轮的摘要：用过哪些工具（带路径）+ 最后的结论开头；无动作无结论的回合返回 `None`。
fn turn_digest(turn: &[LlmMessage]) -> Option<String> {
    let mut used: Vec<String> = Vec::new();
    for msg in turn {
        let Some(calls) = msg.tool_calls.as_deref() else {
            continue;
        };
        for call in calls {
            let path = arg_path(&call.function.arguments);
            used.push(if path.is_empty() {
                call.function.name.clone()
            } else {
                format!("{}({path})", call.function.name)
            });
        }
    }
    let tail: String = turn
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && m.tool_calls.is_none())
        .map(|m| m.content.chars().take(60).collect())
        .unwrap_or_default();
    if used.is_empty() && tail.trim().is_empty() {
        return None;
    }
    let mut out = String::from("[摘要] ");
    if used.is_empty() {
        out.push_str("这一轮没有调用工具。");
    } else {
        out.push_str(&format!("这一轮用过：{}。", used.join("、")));
    }
    if !tail.trim().is_empty() {
        out.push_str(&format!("结论：{}…", tail.trim()));
    }
    Some(out)
}

fn written_chapter_id(tool: &str, arguments: &str) -> Option<String> {
    if tool != "write_file" {
        return None;
    }
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    chapter_id_of_path(args.get("path")?.as_str()?)
}
