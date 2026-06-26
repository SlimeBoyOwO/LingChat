use anyhow::{anyhow, Result};
use chrono::Utc;
use sea_orm::*;

use crate::db::entities::adventure_unlock;

/// Stateless repository for the `adventure_unlock` table.
/// Follows the `SaveRepo` pattern — functions take `&DatabaseConnection`.
pub struct AdventureManager;

impl AdventureManager {
    /// Check if an adventure has been unlocked (row exists).
    pub async fn is_unlocked(db: &DatabaseConnection, adventure_folder: &str) -> Result<bool> {
        let exists = adventure_unlock::Entity::find()
            .filter(adventure_unlock::Column::AdventureFolder.eq(adventure_folder))
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .is_some();
        Ok(exists)
    }

    /// Unlock an adventure: insert a row if it doesn't exist.
    pub async fn unlock_adventure(
        db: &DatabaseConnection,
        adventure_folder: &str,
        character_folder: &str,
    ) -> Result<()> {
        // Idempotent: skip if already unlocked
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
