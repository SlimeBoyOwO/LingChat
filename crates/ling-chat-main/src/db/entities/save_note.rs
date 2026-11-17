use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 存档级手动笔记：同一存档内所有角色共享，不区分角色。
///
/// 主键为 `(save_id, id)`：同一条笔记随存档复制时保留 id，方便模型继续按原 id 修改。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "save_note")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub save_id: i32,
    /// uuid，由 `ling_chat_notes::Note::new` 生成。
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    /// JSON 数组字符串。
    #[sea_orm(column_type = "Text")]
    pub tags: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::save::Entity",
        from = "Column::SaveId",
        to = "super::save::Column::Id"
    )]
    Save,
}

impl Related<super::save::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Save.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
