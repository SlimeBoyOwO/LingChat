use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 身份索引表：文件是真源（身份卡目录），本表只做索引，不含外键。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "player_identity")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub identity_id: String,
    pub name: String,
    pub subtitle: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
