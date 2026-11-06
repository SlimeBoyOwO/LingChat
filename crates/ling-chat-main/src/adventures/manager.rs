use anyhow::{Result, anyhow};
use chrono::{NaiveDateTime, Utc};
use sea_orm::*;
use std::collections::HashMap;

use crate::db::entities::adventure_unlock;

/// 剧本目录名 → (解锁时间, 完成时间)。`completed_at` 为 `None` 表示已解锁未完成。
pub type AdventureStates = HashMap<String, (Option<NaiveDateTime>, Option<NaiveDateTime>)>;

/// adventure_unlock 表的无状态仓库.
/// 遵循 SaveRepo 模式 — 函数接收 &DatabaseConnection 参数。
pub struct AdventureManager;

impl AdventureManager {
    /// 检查冒险是否已解锁（行是否存在）。
    pub async fn is_unlocked(db: &DatabaseConnection, adventure_folder: &str) -> Result<bool> {
        let exists = adventure_unlock::Entity::find()
            .filter(adventure_unlock::Column::AdventureFolder.eq(adventure_folder))
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .is_some();
        Ok(exists)
    }

    /// 解锁一个冒险，如果不存在的话就新增一行
    pub async fn unlock_adventure(
        db: &DatabaseConnection,
        adventure_folder: &str,
        character_folder: &str,
    ) -> Result<()> {
        //  幂等：如已完成则跳过
        if Self::is_unlocked(db, adventure_folder).await? {
            return Ok(());
        }

        let now = Utc::now().naive_utc();
        let active = adventure_unlock::ActiveModel {
            adventure_folder: Set(adventure_folder.to_string()),
            character_folder: Set(character_folder.to_string()),
            unlocked_at: Set(Some(now)),
            completed_at: Set(None),
            ..Default::default()
        };
        active
            .insert(db)
            .await
            .map_err(|e| anyhow!("插入冒险解锁记录失败: {e}"))?;
        Ok(())
    }

    /// 一次性读出全部解锁记录：键为剧本目录名，值为 `(unlocked_at, completed_at)`。
    /// 列表接口逐个查表太碎，这里给上层做状态判定的批量入口。
    pub async fn load_all_states(db: &DatabaseConnection) -> Result<AdventureStates> {
        let rows = adventure_unlock::Entity::find()
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(rows
            .into_iter()
            .map(|r| (r.adventure_folder, (r.unlocked_at, r.completed_at)))
            .collect())
    }

    /// Check if an adventure has been globally completed (completed_at is set).
    pub async fn is_globally_completed(
        db: &DatabaseConnection,
        adventure_folder: &str,
    ) -> Result<bool> {
        let row = adventure_unlock::Entity::find()
            .filter(adventure_unlock::Column::AdventureFolder.eq(adventure_folder))
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(row.and_then(|r| r.completed_at).is_some())
    }

    /// Mark an adventure as globally completed.
    pub async fn mark_global_completed(
        db: &DatabaseConnection,
        adventure_folder: &str,
    ) -> Result<()> {
        let model = adventure_unlock::Entity::find()
            .filter(adventure_unlock::Column::AdventureFolder.eq(adventure_folder))
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .ok_or_else(|| anyhow!("冒险 {} 尚未解锁，无法标记为完成", adventure_folder))?;

        // Idempotent: skip if already completed
        if model.completed_at.is_some() {
            return Ok(());
        }

        let mut active: adventure_unlock::ActiveModel = model.into();
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|e| anyhow!("更新冒险完成状态失败: {e}"))?;
        Ok(())
    }

    /// Reset an adventure: delete the row entirely (lock + completion both removed).
    pub async fn reset_adventure(db: &DatabaseConnection, adventure_folder: &str) -> Result<()> {
        let model = adventure_unlock::Entity::find()
            .filter(adventure_unlock::Column::AdventureFolder.eq(adventure_folder))
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .ok_or_else(|| anyhow!("冒险 {} 不存在", adventure_folder))?;

        adventure_unlock::Entity::delete_by_id(model.id)
            .exec(db)
            .await
            .map_err(|e| anyhow!("删除冒险记录失败: {e}"))?;
        Ok(())
    }
}
