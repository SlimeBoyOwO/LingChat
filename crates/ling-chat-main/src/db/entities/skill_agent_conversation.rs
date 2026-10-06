//! Skill Agent 会话表：对话隔离单位。
//!
//! 每次「新建对话」对应一行，记录创建时打开的剧本 key（仅记录 key，
//! 不存剧本内容快照 —— agent 运行时通过工具实时读取最新剧本内容）。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "skill_agent_conversation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 会话标题（可自动生成，如「新对话」）。
    pub title: Option<String>,
    /// 创建会话时打开的剧本 key（`character/<角色>/<剧本>` 等）。
    pub script_key: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::skill_agent_message::Entity")]
    Message,
}

impl Related<super::skill_agent_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Message.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
