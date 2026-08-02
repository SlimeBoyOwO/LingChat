//! Skill Agent 消息表：OpenAI 格式消息，按会话隔离。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "skill_agent_message")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 所属会话 FK。
    pub conversation_id: i32,
    /// user | assistant | tool | system
    pub role: String,
    /// 消息文本（assistant 仅返回工具调用时可为空）。
    #[sea_orm(column_type = "Text", nullable)]
    pub content: Option<String>,
    /// assistant 的工具调用数组（OpenAI 格式 JSON）。
    #[sea_orm(column_type = "Text", nullable)]
    pub tool_calls: Option<String>,
    /// tool 结果对应的工具调用 id。
    pub tool_call_id: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::skill_agent_conversation::Entity",
        from = "Column::ConversationId",
        to = "super::skill_agent_conversation::Column::Id"
    )]
    Conversation,
}

impl Related<super::skill_agent_conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Conversation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
