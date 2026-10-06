use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "save")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub status: String,
    pub create_date: DateTime,
    pub update_date: DateTime,
    pub running_script_id: Option<i32>,
    pub last_message_id: Option<i32>,
    pub main_role_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::running_script::Entity",
        from = "Column::RunningScriptId",
        to = "super::running_script::Column::Id"
    )]
    RunningScript,
    #[sea_orm(
        belongs_to = "super::role::Entity",
        from = "Column::MainRoleId",
        to = "super::role::Column::Id"
    )]
    MainRole,
    #[sea_orm(has_many = "super::line::Entity")]
    Lines,
}

impl Related<super::running_script::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RunningScript.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
