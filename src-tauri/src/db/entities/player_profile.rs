use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 玩家人设卡（纯 DB 多卡：一行 = 一张人设卡）。
///
/// `card_id` 沿用原「目录名」语义：可读、稳定、可用于剧本 `persona_id` 指定。
/// 全表至多一行 `is_active = true`（由迁移里的部分唯一索引保证）。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "player_profile")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub card_id: String,
    pub user_name: String,
    pub user_subtitle: Option<String>,
    pub user_prompt: Option<String>,
    /// 简介 / 一句话人设
    pub info: Option<String>,
    /// 说话风格示例
    pub system_prompt_example: Option<String>,
    /// 是否当前激活人设
    pub is_active: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
