use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "adventure_unlock")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub adventure_folder: String,
    pub character_folder: String,
    pub unlocked_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
