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
    #[sea_orm(string_value = "user")]
    User,
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
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
