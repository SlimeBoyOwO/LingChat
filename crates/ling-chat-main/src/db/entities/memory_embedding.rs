use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "memory_embedding")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub chunk_id: String,
    pub chunk_version: String,
    pub model_version: String,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::memory_chunk::Entity",
        from = "Column::ChunkId",
        to = "super::memory_chunk::Column::Id"
    )]
    Chunk,
}

impl Related<super::memory_chunk::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Chunk.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
