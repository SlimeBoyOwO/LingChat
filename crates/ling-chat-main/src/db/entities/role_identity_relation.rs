use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 角色与玩家身份的多对多关系：该角色对某身份的认知（角色侧关系）。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "role_identity_relation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub identity_id: String,
    pub text: String,
    /// 该角色对这个身份的初始好感度覆写；None 表示用角色卡默认值。
    pub initial_affection: Option<i32>,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
