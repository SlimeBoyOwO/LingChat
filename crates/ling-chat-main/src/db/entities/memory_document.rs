use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "memory_document")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub role_id: i32,
    pub save_id: Option<i32>,
    pub kind: String,
    pub version: String,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    pub chunking_version: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::role::Entity",
        from = "Column::RoleId",
        to = "super::role::Column::Id"
    )]
    Role,
    #[sea_orm(
        belongs_to = "super::save::Entity",
        from = "Column::SaveId",
        to = "super::save::Column::Id"
    )]
    Save,
    #[sea_orm(has_many = "super::memory_chunk::Entity")]
    Chunk,
}

impl Related<super::role::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Role.def()
    }
}

impl Related<super::save::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Save.def()
    }
}

impl Related<super::memory_chunk::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Chunk.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
