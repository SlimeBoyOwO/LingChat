//! 提示行的唯一构造点。
//!
//! 提示行指模型上下文里「不是谁说的话」的那一类：内容走 [`PromptRole::build_prompt`]
//! 的信封（`{旁白: …}`），属性固定 User，展示名默认取角色的界面名（旁白 / 系统）。
//!
//! 三条不变量（错一条这类行就废了，详见 `GameStatus::add_prompt_line`）：
//! - 属性必须是 User：System 属性会被记忆构建丢弃；
//! - `sender_role_id` 默认保持 None：不计入玩家消息序号，也不算「真实对话」；
//! - 内容必须带信封：`{旁白:` 前缀是判定这类行的唯一真相源。

use crate::ai_service::types::{GameLine, LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

/// 一条待落表的提示行。
pub struct PromptLine {
    role: PromptRole,
    content: String,
    display_name: Option<String>,
    sender_role_id: Option<i32>,
}

impl PromptLine {
    /// 由角色类型包出信封。
    pub fn new(role: PromptRole, text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            content: role.build_prompt(&text),
            role,
            display_name: None,
            sender_role_id: None,
        }
    }

    /// 内容已带信封。用于上游把格式化后的字符串跨异步 / 类型边界传下来的路径，
    /// 这些地方本轮不改契约，只统一落表时的行形状。
    pub fn formatted(role: PromptRole, content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            role,
            display_name: None,
            sender_role_id: None,
        }
    }

    /// 覆盖展示名。默认是 [`PromptRole::display_name`]；信封与标签不一致的场合
    /// （现在有几处旁白信封挂「系统」标签）必须显式给出，否则前端气泡标题会变。
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// 指定发送者。提示行默认不带归属。
    pub fn sender(mut self, role_id: Option<i32>) -> Self {
        self.sender_role_id = role_id;
        self
    }

    pub fn into_base(self) -> LineBase {
        let display_name = self
            .display_name
            .unwrap_or_else(|| self.role.display_name().to_string());
        LineBase {
            content: self.content,
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(display_name),
            sender_role_id: self.sender_role_id,
            ..Default::default()
        }
    }

    /// 直接构造成带感知集合的台词行（锚定插入用）。
    pub fn into_game_line(self, perceived_role_ids: Vec<i32>) -> GameLine {
        GameLine::from_base(self.into_base(), perceived_role_ids)
    }
}
