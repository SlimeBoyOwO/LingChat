use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "memory_bank")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Text")]
    pub info: String,
    pub save_id: i32,
    pub role_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::save::Entity",
        from = "Column::SaveId",
        to = "super::save::Column::Id"
    )]
    Save,
    #[sea_orm(
        belongs_to = "super::role::Entity",
        from = "Column::RoleId",
        to = "super::role::Column::Id"
    )]
    Role,
}

impl Related<super::save::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Save.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
