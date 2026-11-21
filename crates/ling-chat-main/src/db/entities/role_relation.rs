use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 角色之间的认知关系：`role_id` 如何看待 `target_role_id`。
///
/// 单向多对多；玩家身份也是 role，因此同一张表同时承载
/// 角色→玩家、玩家→角色、角色↔角色三种方向。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "role_relation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub target_role_id: i32,
    pub text: String,
    /// 该角色对目标（通常是玩家角色）的初始好感度覆写；None 表示用角色卡默认值。
    pub initial_affection: Option<i32>,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
