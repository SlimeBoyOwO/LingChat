use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "memory_chunk")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub document_id: String,
    pub ordinal: i64,
    pub version: String,
    /// 正文中的 UTF-8 字节范围，左闭右开；正文和来源从所属文档还原。
    pub start_byte: i64,
    pub end_byte: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::memory_document::Entity",
        from = "Column::DocumentId",
        to = "super::memory_document::Column::Id"
    )]
    Document,
    #[sea_orm(has_one = "super::memory_embedding::Entity")]
    Embedding,
}

impl Related<super::memory_document::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Document.def()
    }
}

impl Related<super::memory_embedding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Embedding.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
