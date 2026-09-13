//! 对话台词嵌入向量：按存档（save_id）存储的向量库。
//!
//! 「一键整理当前对话」把当前存档的台词逐条编码，去重后写入此表；读档时
//! （`load_save`）把该存档的全部向量载回内存语义索引（MemoryIndex），保证
//! 重启后仍可参与语义检索与去重，且向量随游戏存档一起保存/删除。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "embedding")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 所属存档。
    pub save_id: i32,
    /// 片段来源标识（`FragmentSource` 的标签，如 `conversation`）。
    pub source: String,
    /// 原始台词文本（不含前缀）。
    #[sea_orm(column_type = "Text")]
    pub text: String,
    /// 向量维度。
    pub dim: i32,
    /// f32×dim 的小端字节序列。
    #[sea_orm(column_type = "Blob")]
    pub vector: Vec<u8>,
    pub created_at: DateTime,
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

/// 把 f32 向量编码为小端字节序列（与语义记忆向量库的存储格式一致）。
pub fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// 把小端字节序列解码为 f32 向量。
pub fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
