use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 存档 → 「我的身份」的绑定。一个存档一条，中途不更换。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "save_identity")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub save_id: i32,
    /// 身份卡的不可变 id（不是文件夹名）
    pub identity_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
