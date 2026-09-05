use crate::ai_service::types::{GameLine, GameMemoryBank};
use crate::db::entities::line::LineAttribute;

use super::MemorySectionLimits;

/// Keep UTF-8 intact when restricting prompt input by character count.
pub(crate) fn truncate_to_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 || text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

/// One visibility rule is shared by threshold counting and recent-window slicing.
pub(crate) fn line_visible_to_role(line: &GameLine, role_id: i32) -> bool {
    !matches!(line.attribute(), LineAttribute::System)
        && !line.content().trim().is_empty()
        && (line.sender_role_id() == Some(role_id) || line.perceived_role_ids.contains(&role_id))
}

pub(crate) fn render_system_memory(bank: &GameMemoryBank, limits: MemorySectionLimits) -> String {
    format!(
        "\n\n====== 记忆库 (Memory Bank) ======\n\
         【taの信息】：{}\n\
         【重要约定】：{}\n\
         【长期经历】：{}\n\
         =================================\n",
        truncate_to_chars(&bank.data.user_info, limits.user_info),
        truncate_to_chars(&bank.data.promises, limits.promises),
        truncate_to_chars(&bank.data.long_term, limits.long_term),
    )
}

pub(crate) fn render_short_term(bank: &GameMemoryBank, limits: MemorySectionLimits) -> String {
    let short = truncate_to_chars(bank.data.short_term.trim(), limits.short_term);
    if short.is_empty() || short == "暂无近期对话摘要。" {
        String::new()
    } else {
        format!("【近期回顾】{}\n\n", short)
    }
}
