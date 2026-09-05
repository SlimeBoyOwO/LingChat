use anyhow::anyhow;
use sea_orm::*;

use crate::ai_service::types::GameMemoryBank;
use crate::db::entities::memory_bank;

/// MemoryBank 的唯一数据库适配器。
///
/// 生产调用必须按 `(save_id, role_id)` 读写；不公开裸 row id 或原始 JSON
/// 写入口，避免绕过 MemoryBank 的类型和归属边界。
pub struct MemoryRepo;

impl MemoryRepo {
    pub(crate) async fn get_memories(
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

    pub(crate) async fn get_latest_memory(
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

    /// Load the newest compatible row for every role in a save. JSON is decoded
    /// only after choosing the maximum row id, so a malformed superseded row
    /// cannot shadow a newer valid MemoryBank.
    pub(crate) async fn load_for_save(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<std::collections::HashMap<i32, GameMemoryBank>, anyhow::Error> {
        let rows = Self::get_memories(db, save_id, None).await?;
        let mut newest: std::collections::HashMap<i32, memory_bank::Model> =
            std::collections::HashMap::new();
        for row in rows {
            let Some(role_id) = row.role_id else { continue };
            if newest
                .get(&role_id)
                .is_none_or(|current| row.id > current.id)
            {
                newest.insert(role_id, row);
            }
        }

        let mut banks = std::collections::HashMap::with_capacity(newest.len());
        for (role_id, row) in newest {
            let bank = serde_json::from_str(&row.info).map_err(|error| {
                anyhow!(
                    "MemoryBank 数据损坏: save_id={}, role_id={}, memory_bank.id={}, error={}",
                    save_id,
                    role_id,
                    row.id,
                    error
                )
            })?;
            banks.insert(role_id, bank);
        }
        Ok(banks)
    }

    /// Serialize and write one immutable runtime snapshot. This is the sole
    /// production MemoryBank write API; callers never pass a row id.
    pub(crate) async fn upsert_for_save_role(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: i32,
        bank: &GameMemoryBank,
    ) -> Result<(), anyhow::Error> {
        let info = serde_json::to_string(bank).map_err(|error| {
            anyhow!(
                "MemoryBank 序列化失败: save_id={}, role_id={}, error={}",
                save_id,
                role_id,
                error
            )
        })?;
        Self::upsert_serialized_for_save_role(db, save_id, role_id, &info)
            .await
            .map(|_| ())
    }

    /// Private serialization boundary used only after typed input has been
    /// produced by `upsert_for_save_role`.
    async fn upsert_serialized_for_save_role(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: i32,
        info: &str,
    ) -> Result<memory_bank::Model, anyhow::Error> {
        match Self::get_latest_memory(db, save_id, role_id).await? {
            Some(model) => {
                let mut active: memory_bank::ActiveModel = model.into();
                active.info = Set(info.to_string());
                active.role_id = Set(Some(role_id));
                active.update(db).await.map_err(|e| anyhow!(e))
            },
            None => {
                let active = memory_bank::ActiveModel {
                    save_id: Set(save_id),
                    role_id: Set(Some(role_id)),
                    info: Set(info.to_string()),
                    ..Default::default()
                };
                active.insert(db).await.map_err(|e| anyhow!(e))
            },
        }
    }

    /// 按 role_id 删除所有存档下的相关记忆（不限 save_id）。
    /// 用于删除整个角色时的兜底清理，应对已被 save.main_role_id 解绑的孤儿记忆行。
    #[allow(dead_code)]
    pub(crate) async fn delete_all_memories_by_role_id(
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<u64, anyhow::Error> {
        let result = memory_bank::Entity::delete_many()
            .filter(memory_bank::Column::RoleId.eq(role_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        Ok(result.rows_affected)
    }

    pub(crate) async fn delete_for_save(
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
