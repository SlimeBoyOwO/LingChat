//! `memory_archive_profiles` 表：旧版记忆导入后的「剧本×角色」档案目录。
//!
//! 一行对应一个「剧本 × 角色」：记录该档案的剧本 key、角色 id/名，以及实际
//! 存放记忆内容的数据表名（`memory_archive_{role_id}`）。数据表结构见
//! `api::memory_archive::memory_import_legacy` 的注释（动态创建，无静态实体）。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "memory_archive_profiles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 剧本 key（`role.script_key`；全局角色为空）。
    pub script_key: Option<String>,
    /// 角色在剧本内的 key（`role.script_role_key`）。
    pub script_role_key: Option<String>,
    /// 角色主键（`role.id`）。
    pub role_id: i32,
    /// 角色名（`role.name`）。
    #[sea_orm(column_type = "Text")]
    pub role_name: String,
    /// 该档案的实际数据表名（`memory_archive_{role_id}`）。
    #[sea_orm(column_type = "Text")]
    pub table_name: String,
    /// 创建时间（Unix 毫秒）。
    pub created_at: i64,
    /// 最后更新时间（Unix 毫秒）。
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
