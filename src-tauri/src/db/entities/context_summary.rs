//! 上下文压缩摘要表：按存档保存 kimi 式交接笔记摘要。
//!
//! 与 MemoryBank（按角色、4 段式长期记忆）相互独立：本表只服务上下文窗口管理，
//! 一个存档一行，记录「第 cutoff_count 条台词之前的内容已被压缩为 summary」。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "context_summary")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub save_id: i32,
    #[sea_orm(column_type = "Text")]
    pub summary: String,
    /// 摘要覆盖到的台词条数（line_list 下标语义）
    pub cutoff_count: i32,
    pub updated_at: DateTime,
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
