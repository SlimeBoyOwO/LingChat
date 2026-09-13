//! 对话台词嵌入向量持久化（`embedding` 表）。按存档（save_id）读写。

use anyhow::Result;
use chrono::Utc;
use sea_orm::*;

use crate::db::entities::embedding;

pub struct EmbeddingRepo;

impl EmbeddingRepo {
    /// 列出某个存档的全部嵌入（含向量，供读档载回索引/去重）。
    pub async fn list_by_save(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<Vec<embedding::Model>> {
        embedding::Entity::find()
            .filter(embedding::Column::SaveId.eq(save_id))
            .all(db)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// 批量写入新的嵌入记录。返回写入条数。
    ///
    /// 存档存在性校验与写入在同一事务内完成，避免「校验后存档被删除」的竞态
    /// 产生指向不存在存档的孤儿 embedding 行；任一行失败则整体回滚。
    pub async fn insert_many(
        db: &DatabaseConnection,
        save_id: i32,
        rows: &[(String, String, Vec<f32>)],
    ) -> Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }
        let txn = db.begin().await.map_err(|e| anyhow::anyhow!("{e}"))?;

        let exists = crate::db::entities::save::Entity::find_by_id(save_id)
            .count(&txn)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?
            > 0;
        if !exists {
            txn.rollback().await.map_err(|e| anyhow::anyhow!("{e}"))?;
            return Err(anyhow::anyhow!("存档 {save_id} 不存在"));
        }

        let now = Utc::now().naive_utc();
        let mut inserted = 0usize;
        for (text, source, vector) in rows {
            let active = embedding::ActiveModel {
                save_id: Set(save_id),
                source: Set(source.clone()),
                text: Set(text.clone()),
                dim: Set(vector.len() as i32),
                vector: Set(embedding::encode_vector(vector)),
                created_at: Set(now),
                ..Default::default()
            };
            active
                .insert(&txn)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            inserted += 1;
        }
        txn.commit().await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(inserted)
    }

    /// 读取某存档的片段候选（text, source, vector 已解码），供
    /// 「一键整理当前对话」与新台词做去重，避免重复入库。
    pub async fn list_decoded(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<Vec<(String, String, Vec<f32>)>> {
        let rows = Self::list_by_save(db, save_id).await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.text, r.source, embedding::decode_vector(&r.vector)))
            .collect())
    }

    /// 某个存档当前已归档的嵌入条数。
    pub async fn count_by_save(db: &DatabaseConnection, save_id: i32) -> Result<u64> {
        embedding::Entity::find()
            .filter(embedding::Column::SaveId.eq(save_id))
            .count(db)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// 删除某个存档的全部嵌入（删除存档时级联清理）。
    pub async fn delete_by_save(db: &DatabaseConnection, save_id: i32) -> Result<()> {
        embedding::Entity::delete_many()
            .filter(embedding::Column::SaveId.eq(save_id))
            .exec(db)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}
