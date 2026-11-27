//! 导演现场上下文装配与历史裁剪。

use chrono::Local;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::core::tail_lines;
use crate::ai_service::types::{GameLine, LlmMessage};
use crate::db::entities::line::LineAttribute;

use super::{ContextMessage, ContextSection, ContextStats, DirectorContext, DirectorMessage};

const DIRECTOR_PERSONA: &str = "你是这场对话的导演，站在舞台之外观察现场，负责安排剧情走向。\n\
玩家会在这里告诉你想看到什么，你可以据此调整场景、氛围与角色的处境。\n\
你看得到下面列出的现场信息与最近的对话，但不要代替角色说话，也不要替玩家做决定。";
const LINE_CHAR_LIMIT: usize = 240;
const ROLE_INFO_LIMIT: usize = 200;
const RECENT_BLOCK_LIMIT: usize = 6000;
const SYSTEM_CHAR_LIMIT: usize = 10000;
const HISTORY_CHAR_LIMIT: usize = 12000;
const MAX_LISTED_ROLES: usize = 12;

/// 装配好的导演 system 与分段展示数据。
pub struct AssembledSystem {
    pub prompt: String,
    pub sections: Vec<ContextSection>,
    pub truncated: bool,
}

/// 按现场与历史构造一次模型请求及可视化快照。
pub fn compose_request(
    messages: &[DirectorMessage],
    assembled: AssembledSystem,
    source: &str,
    provider: Option<String>,
    model: Option<String>,
) -> (Vec<LlmMessage>, DirectorContext) {
    let history: Vec<LlmMessage> = messages.iter().map(DirectorMessage::to_llm).collect();
    let sanitized = sanitize_tool_pairing(&history);
    let (kept, dropped_messages) = trim_history(&sanitized, HISTORY_CHAR_LIMIT);
    let history_chars: usize = kept.iter().map(|m| m.content.chars().count()).sum();

    let mut request = Vec::with_capacity(kept.len() + 1);
    request.push(LlmMessage::system(assembled.prompt.clone()));
    request.extend(kept);

    let stats = ContextStats {
        system_chars: assembled.prompt.chars().count(),
        history_chars,
        message_count: request.len(),
        dropped_messages,
        truncated: assembled.truncated || dropped_messages > 0,
    };
    let context = DirectorContext {
        source: source.to_string(),
        built_at: Local::now().to_rfc3339(),
        provider,
        model,
        system_prompt: assembled.prompt,
        sections: assembled.sections,
        messages: request.iter().map(to_context_message).collect(),
        stats,
    };
    (request, context)
}

fn to_context_message(m: &LlmMessage) -> ContextMessage {
    ContextMessage {
        role: m.role.clone(),
        content: (!m.content.is_empty()).then(|| m.content.clone()),
        tool_calls: m.tool_calls.clone(),
        tool_call_id: m.tool_call_id.clone(),
    }
}

/// 工具调用与结果必须完整配对，才能进入历史。
fn sanitize_tool_pairing(messages: &[LlmMessage]) -> Vec<LlmMessage> {
    let mut out = Vec::with_capacity(messages.len());
    let mut i = 0;
    while i < messages.len() {
        let message = &messages[i];
        if message.role == "tool" {
            i += 1;
            continue;
        }
        let Some(calls) = &message.tool_calls else {
            out.push(message.clone());
            i += 1;
            continue;
        };

        let mut j = i + 1;
        let mut results = Vec::new();
        while j < messages.len() && messages[j].role == "tool" {
            results.push(messages[j].clone());
            j += 1;
        }
        let paired = !calls.is_empty()
            && calls.iter().all(|call| {
                results
                    .iter()
                    .any(|r| r.tool_call_id.as_deref() == Some(call.id.as_str()))
            });
        if paired {
            out.push(message.clone());
            out.extend(results.into_iter().filter(|r| {
                calls
                    .iter()
                    .any(|call| r.tool_call_id.as_deref() == Some(call.id.as_str()))
            }));
        }
        i = j;
    }
    out
}

fn trim_history(messages: &[LlmMessage], budget: usize) -> (Vec<LlmMessage>, usize) {
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let start = i;
        i += 1;
        if messages[start].tool_calls.is_some() {
            while i < messages.len() && messages[i].role == "tool" {
                i += 1;
            }
        }
        blocks.push((start, i));
    }
    if blocks.is_empty() {
        return (Vec::new(), 0);
    }

    let mut total = 0usize;
    let mut keep_from = blocks.len();
    for (idx, (start, end)) in blocks.iter().enumerate().rev() {
        let len: usize = messages[*start..*end]
            .iter()
            .map(|m| m.content.chars().count())
            .sum();
        if total + len > budget && keep_from < blocks.len() {
            break;
        }
        total += len;
        keep_from = idx;
    }

    let dropped: usize = blocks
        .iter()
        .take(keep_from)
        .map(|(start, end)| end - start)
        .sum();
    (messages[blocks[keep_from].0..].to_vec(), dropped)
}

/// 装配人设、现场、在场角色与限定长度的近期对话。
pub fn assemble(gs: &GameStatus, window: usize) -> AssembledSystem {
    let mut remaining = SYSTEM_CHAR_LIMIT;
    let mut truncated = false;
    let mut sections = Vec::new();

    push_section(
        &mut sections,
        "persona",
        "导演人设",
        DIRECTOR_PERSONA.to_string(),
        &mut remaining,
        &mut truncated,
    );
    push_section(
        &mut sections,
        "scene",
        "当前现场",
        build_scene(gs),
        &mut remaining,
        &mut truncated,
    );
    push_section(
        &mut sections,
        "roles",
        "在场角色",
        build_roles(gs),
        &mut remaining,
        &mut truncated,
    );

    let (recent, recent_truncated) = build_recent(gs, window, remaining.min(RECENT_BLOCK_LIMIT));
    truncated |= recent_truncated;
    push_section(
        &mut sections,
        "recent",
        "最近对话",
        recent,
        &mut remaining,
        &mut truncated,
    );

    let prompt = sections
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    AssembledSystem {
        prompt,
        sections,
        truncated,
    }
}

fn push_section(
    sections: &mut Vec<ContextSection>,
    key: &str,
    title: &str,
    content: String,
    remaining: &mut usize,
    truncated: &mut bool,
) {
    if *remaining == 0 {
        *truncated = true;
        return;
    }
    let len = content.chars().count();
    let body = if len > *remaining {
        *truncated = true;
        let body = truncate_chars(&content, *remaining);
        *remaining = 0;
        body
    } else {
        *remaining -= len;
        content
    };
    sections.push(ContextSection {
        key: key.to_string(),
        title: title.to_string(),
        content: body,
    });
}

fn build_scene(gs: &GameStatus) -> String {
    let mode = match gs.script_status.as_ref() {
        Some(script) => format!("剧本《{}》", script.name),
        None => "自由对话".to_string(),
    };
    let time = gs
        .last_dialog_time
        .as_ref()
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "未记录".to_string());
    let scene = gs
        .current_scene_id
        .clone()
        .unwrap_or_else(|| "未选择".to_string());
    let speaker = gs
        .current_role_id
        .and_then(|id| role_label(gs, id))
        .unwrap_or_else(|| "无".to_string());

    let mut out = String::from("【当前现场】\n");
    out.push_str(&format!("模式：{mode}\n"));
    out.push_str(&format!("时间：{time}\n"));
    out.push_str(&format!("场景：{scene}\n"));
    out.push_str(&format!("背景：{}\n", present_value(&gs.background)));
    out.push_str(&format!(
        "背景音乐：{}\n",
        present_value(&gs.background_music)
    ));
    out.push_str(&format!("氛围：{}\n", present_value(&gs.background_effect)));
    out.push_str(&format!("天气：{}\n", present_value(&gs.weather_effect)));
    out.push_str(&format!("当前发言者：{speaker}"));
    out
}

fn build_roles(gs: &GameStatus) -> String {
    let mut out = String::from("【在场角色】\n");
    let player = &gs.player;
    let player_name = player.user_name.trim();
    if player_name.is_empty() {
        out.push_str("玩家：未知\n");
    } else {
        let subtitle = player.user_subtitle.trim();
        if subtitle.is_empty() {
            out.push_str(&format!("玩家：{player_name}\n"));
        } else {
            out.push_str(&format!("玩家：{player_name}（{subtitle}）\n"));
        }
    }

    let mut ids: Vec<i32> = gs
        .present_role_ids
        .iter()
        .copied()
        .filter(|id| *id != 0)
        .collect();
    ids.sort_unstable();
    if ids.is_empty() {
        out.push_str("（暂无其他在场角色）");
        return out;
    }

    for id in ids.iter().take(MAX_LISTED_ROLES) {
        let Some(role) = gs.role_manager.get_loaded(*id) else {
            out.push_str(&format!("- #{id}（角色未加载）\n"));
            continue;
        };
        let name = role
            .display_name
            .clone()
            .unwrap_or_else(|| role.settings.ai_name.clone());
        let subtitle = role.settings.ai_subtitle.as_deref().unwrap_or("").trim();
        let info = role.settings.info.as_deref().unwrap_or("").trim();
        out.push_str(&format!("- {name}"));
        if !subtitle.is_empty() {
            out.push_str(&format!("（{subtitle}）"));
        }
        if !gs.onstage_role_ids.contains(id) {
            out.push_str("（不在舞台）");
        }
        if !info.is_empty() {
            out.push_str(&format!("：{}", truncate_chars(info, ROLE_INFO_LIMIT)));
        }
        out.push('\n');
    }
    if ids.len() > MAX_LISTED_ROLES {
        out.push_str(&format!(
            "（另有 {} 位在场角色未列出）",
            ids.len() - MAX_LISTED_ROLES
        ));
    }
    out
}

/// 最近对话段，从最新往回取，装不下就停在更旧的一条之前。
fn build_recent(gs: &GameStatus, window: usize, budget: usize) -> (String, bool) {
    let lines = tail_lines(gs, window);
    let mut kept: Vec<String> = Vec::new();
    let mut used = 0usize;
    let mut truncated = false;
    for line in lines.iter().rev() {
        let text = format_line(gs, line);
        let len = text.chars().count() + 1;
        if used + len > budget {
            truncated = true;
            break;
        }
        used += len;
        kept.push(text);
    }
    kept.reverse();

    if kept.is_empty() {
        return ("【最近对话】\n（还没有对话）".to_string(), truncated);
    }
    (format!("【最近对话】\n{}", kept.join("\n")), truncated)
}

fn format_line(gs: &GameStatus, line: &GameLine) -> String {
    let content = truncate_chars(&line.base.content, LINE_CHAR_LIMIT);
    match line.attribute() {
        LineAttribute::Assistant => {
            let role_id = line.sender_role_id().unwrap_or(0);
            let name = role_label(gs, role_id)
                .or_else(|| line.base.display_name.clone())
                .unwrap_or_else(|| "角色".to_string());
            let emotion = line
                .base
                .predicted_emotion
                .as_deref()
                .or(line.base.original_emotion.as_deref())
                .map(|e| format!("({e})"))
                .unwrap_or_default();
            format!("[{role_id}] {name}{emotion}: {content}")
        },
        LineAttribute::User => {
            let role_id = line.sender_role_id().unwrap_or(0);
            if role_id == 0 {
                format!("[旁白] {content}")
            } else {
                let name = line
                    .base
                    .display_name
                    .clone()
                    .unwrap_or_else(|| gs.player.user_name.clone());
                format!("[{role_id}] {name}: {content}")
            }
        },
        LineAttribute::System => format!("[系统] {content}"),
        LineAttribute::Tool => format!("[工具] {content}"),
    }
}

fn role_label(gs: &GameStatus, role_id: i32) -> Option<String> {
    if role_id == 0 {
        return None;
    }
    let role = gs.role_manager.get_loaded(role_id)?;
    Some(
        role.display_name
            .clone()
            .unwrap_or_else(|| role.settings.ai_name.clone()),
    )
}

fn present_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        "无".to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate_chars(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_string();
    }
    let mut out: String = s.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::{FunctionCall, ToolCall};

    #[test]
    fn 历史裁剪保持工具调用配对() {
        let call = ToolCall {
            id: "call_1".to_string(),
            type_: "function".to_string(),
            function: FunctionCall {
                name: "director_list_events".to_string(),
                arguments: "{}".to_string(),
            },
        };
        let history = vec![
            LlmMessage::user("旧".repeat(HISTORY_CHAR_LIMIT)),
            LlmMessage::tool(vec![call]),
            LlmMessage::tool_result("call_1", "{}"),
            LlmMessage::user("最新"),
        ];
        let (kept, dropped) = trim_history(&history, 20);
        assert_eq!(kept, history[1..].to_vec());
        assert_eq!(dropped, 1);
    }
}
