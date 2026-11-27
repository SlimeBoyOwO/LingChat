use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 存档信纸：一行一页，正文在 data/letters/{id}.md。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "save_letter")]
pub struct Model {
    /// uuid，同时是正文文件名。
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub save_id: i32,
    /// 页序，从 0 开始。
    pub page: i32,
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
