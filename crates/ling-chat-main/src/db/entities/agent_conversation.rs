//! Agent 会话表：对话隔离单位，剧本编辑器助手与游戏内导演共用。
//!
//! `kind` 区分用途：`skill` 是剧本编辑器助手，`director` 是自由剧情模式的导演。
//! `save_id` 只在 `director` 会话上有值，指向所属存档（应用层引用，无 SQL 外键）。
//! 当前导演 = 该存档下 `kind = 'director'` 且 `updated_at` 最新的一条。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 剧本编辑器助手会话。
pub const KIND_SKILL: &str = "skill";
/// 游戏内导演会话。
pub const KIND_DIRECTOR: &str = "director";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_conversation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 会话标题（可自动生成，如「新对话」）。
    pub title: Option<String>,
    /// 创建会话时打开的剧本 key（`character/<角色>/<剧本>` 等）。
    pub script_key: Option<String>,
    /// 会话用途，取 [`KIND_SKILL`] 或 [`KIND_DIRECTOR`]。
    pub kind: String,
    /// 所属存档；仅导演会话有值。
    pub save_id: Option<i32>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::agent_message::Entity")]
    Message,
    #[sea_orm(
        belongs_to = "super::save::Entity",
        from = "Column::SaveId",
        to = "super::save::Column::Id"
    )]
    Save,
}

impl Related<super::agent_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Message.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
