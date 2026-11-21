use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum RoleType {
    #[sea_orm(string_value = "main")]
    Main,
    #[sea_orm(string_value = "npc")]
    Npc,
    #[sea_orm(string_value = "system")]
    System,
    /// 历史哨兵：id=0，承接旧台词与旁白。
    #[sea_orm(string_value = "user")]
    User,
    /// 玩家身份卡对应的角色；没有角色卡目录，只作为存档绑定与关系两端。
    #[sea_orm(string_value = "player")]
    Player,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "role")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub script_key: Option<String>,
    pub script_role_key: Option<String>,
    pub name: String,
    pub role_type: RoleType,
    pub resource_folder: Option<String>,
    /// 玩家角色的身份卡稳定 id；其他类型为空。
    #[serde(default)]
    pub identity_id: Option<String>,
    /// 身份卡里的 relations 是否已导入 role_relation；只对玩家角色有意义。
    #[serde(default)]
    pub identity_relations_seeded: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
