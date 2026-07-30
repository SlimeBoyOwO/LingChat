use anyhow::anyhow;
use sea_orm::*;

use crate::db::entities::memory_bank;

/// MemoryBank 仓库 — 对标 Python `MemoryManager`。
///
/// 与 `SaveRepo` 中的简易 MemoryBank 方法互补：`SaveRepo` 提供存档流程中
/// 的批量 upsert/get/delete；`MemoryRepo` 提供更完整的单条 CRUD 和按角色粒度的操作。
pub struct MemoryRepo;

impl MemoryRepo {
    #[allow(dead_code)]
    pub async fn add_memory(
        db: &DatabaseConnection,
        save_id: i32,
        info: &str,
        role_id: Option<i32>,
    ) -> Result<memory_bank::Model, anyhow::Error> {
        let active = memory_bank::ActiveModel {
            save_id: Set(save_id),
            info: Set(info.to_string()),
            role_id: Set(role_id),
            ..Default::default()
        };
        active.insert(db).await.map_err(|e| anyhow!(e))
    }

    pub async fn get_memories(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: Option<i32>,
    ) -> Result<Vec<memory_bank::Model>, anyhow::Error> {
        let mut stmt = memory_bank::Entity::find().filter(memory_bank::Column::SaveId.eq(save_id));
        if let Some(rid) = role_id {
            stmt = stmt.filter(memory_bank::Column::RoleId.eq(rid));
        }
        stmt.all(db).await.map_err(|e| anyhow!(e))
    }

    pub async fn get_latest_memory(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: i32,
    ) -> Result<Option<memory_bank::Model>, anyhow::Error> {
        memory_bank::Entity::find()
            .filter(memory_bank::Column::SaveId.eq(save_id))
            .filter(memory_bank::Column::RoleId.eq(role_id))
            .order_by_desc(memory_bank::Column::Id)
            .one(db)
            .await
            .map_err(|e| anyhow!(e))
    }

    pub async fn upsert_memory(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: i32,
        info: &str,
        memory_id: Option<i32>,
    ) -> Result<memory_bank::Model, anyhow::Error> {
        let existing: Option<memory_bank::Model> = if let Some(mid) = memory_id {
            memory_bank::Entity::find_by_id(mid)
                .one(db)
                .await
                .map_err(|e| anyhow!(e))?
        } else {
            Self::get_latest_memory(db, save_id, role_id).await?
        };

        match existing {
            Some(model) => {
                let mut active: memory_bank::ActiveModel = model.into();
                active.info = Set(info.to_string());
                active.role_id = Set(Some(role_id));
                active.update(db).await.map_err(|e| anyhow!(e))
            }
            None => {
                let active = memory_bank::ActiveModel {
                    save_id: Set(save_id),
                    role_id: Set(Some(role_id)),
                    info: Set(info.to_string()),
                    ..Default::default()
                };
                active.insert(db).await.map_err(|e| anyhow!(e))
            }
        }
    }

    #[allow(dead_code)]
    pub async fn update_memory(
        db: &DatabaseConnection,
        memory_id: i32,
        new_info: Option<&str>,
        new_role_id: Option<i32>,
    ) -> Result<Option<memory_bank::Model>, anyhow::Error> {
        let model = memory_bank::Entity::find_by_id(memory_id)
            .one(db)
            .await
            .map_err(|e| anyhow!(e))?;
        let Some(model) = model else {
            return Ok(None);
        };
        let mut active: memory_bank::ActiveModel = model.into();
        if let Some(info) = new_info {
            active.info = Set(info.to_string());
        }
        if let Some(rid) = new_role_id {
            active.role_id = Set(Some(rid));
        }
        let updated = active.update(db).await.map_err(|e| anyhow!(e))?;
        Ok(Some(updated))
    }

    #[allow(dead_code)]
    pub async fn delete_memory(
        db: &DatabaseConnection,
        memory_id: i32,
    ) -> Result<bool, anyhow::Error> {
        let result = memory_bank::Entity::delete_by_id(memory_id)
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        Ok(result.rows_affected > 0)
    }

    /// 按存档+角色批量删除。若 role_id 为 None 则不执行（安全检查，对标 Python）。
    #[allow(dead_code)]
    pub async fn delete_memories_by_role(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: Option<i32>,
    ) -> Result<u64, anyhow::Error> {
        let Some(rid) = role_id else {
            return Ok(0);
        };
        let result = memory_bank::Entity::delete_many()
            .filter(memory_bank::Column::SaveId.eq(save_id))
            .filter(memory_bank::Column::RoleId.eq(rid))
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        Ok(result.rows_affected)
    }

    #[allow(dead_code)]
    pub async fn delete_memories_by_save(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<u64, anyhow::Error> {
        let result = memory_bank::Entity::delete_many()
            .filter(memory_bank::Column::SaveId.eq(save_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        Ok(result.rows_affected)
    }
}
