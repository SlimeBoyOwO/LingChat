//! 向量记忆表：工具驱动的长期记忆（memory_store / vector_search / memory_delete）。
//!
//! 每条记忆按固定长度切块后逐块入库，同一记忆的多个分块共享 `memory_id`；
//! `embedding` 存 L2 归一化的 Float32 小端字节，检索时在内存里算余弦相似度。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "memory_embeddings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 所属角色（记忆按角色隔离）。
    pub role_id: i32,
    /// 同一条记忆的所有分块共享的 UUID。
    pub memory_id: String,
    /// 分块序号（0 起）。
    pub chunk_index: i32,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    /// 逗号分隔的标签（可空）。
    #[sea_orm(column_type = "Text", nullable)]
    pub tags: Option<String>,
    /// Float32 小端字节，L2 归一化。
    #[sea_orm(column_type = "Blob")]
    pub embedding: Vec<u8>,
    /// 嵌入维度（= embedding.len() / 4）。
    pub dim: i32,
    /// 产生该向量的嵌入模型名（换模型后旧向量不参与检索）。
    pub model: String,
    /// 创建时间（Unix 毫秒）。
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
