//! Static utility functions for prompt 装饰.
//!
//! 用于 prompt 装饰的静态工具函数，使其符合本项目的特定需求。

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::{GameRole, LineAttributeExt, LineBase, ScriptStatus};
use crate::db::entities::line::LineAttribute;

// ============================================================
// Placeholder replacement
// ============================================================

/// Replace `%player%` with the player's user_name.
pub fn replace_placeholder(text: &str, game_status: &GameStatus) -> String {
    text.replace("%player%", &game_status.player.user_name)
}

// ============================================================
// User message builder
// ============================================================

/// 系统消息提示词转换器，假如有系统消息或者剧情提示，通过下面这个函数进行提示词转换。
/// 提示角色
pub enum PromptRole {
    /// 系统提示，如时间，错误报告等
    System,
    /// 旁白，剧本提示中的旁白信息
    Narrator,
    /// 剧情提示，剧本模式中用于提示AI下一步的信息
    Plot,
}

impl PromptRole {
    /// 根据角色类型组装最终提示
    /// - `prompt`: 提示词内容
    pub fn build_prompt(&self, prompt: &str) -> String {
        match self {
            PromptRole::System => {
                format!("{{旁白: （{}）}}", prompt)
            }
            PromptRole::Narrator => {
                format!("{{旁白: {}}}", prompt)
            }
            PromptRole::Plot => {
                format!("{{旁白: （接下来的剧情演绎提示：{}）}}", prompt)
            }
        }
    }
}
