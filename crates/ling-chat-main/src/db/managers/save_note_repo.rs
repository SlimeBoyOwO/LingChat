use anyhow::{Result, anyhow};
use ling_chat_notes::Note;
use sea_orm::*;
use std::collections::HashMap;

use crate::db::entities::save_note;

/// 存档级笔记仓库：同档内所有角色共享，不区分角色。
pub struct SaveNoteRepo;

impl SaveNoteRepo {
    pub async fn list(db: &DatabaseConnection, save_id: i32) -> Result<Vec<Note>> {
        let rows = save_note::Entity::find()
            .filter(save_note::Column::SaveId.eq(save_id))
            .order_by_asc(save_note::Column::CreatedAt)
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(rows.into_iter().map(Note::from).collect())
    }

    /// 读取全部存档的笔记并按 save_id 分组，供后台预热一次性遍历。
    pub async fn list_all_grouped(db: &DatabaseConnection) -> Result<HashMap<i32, Vec<Note>>> {
        let rows = save_note::Entity::find()
            .order_by_asc(save_note::Column::CreatedAt)
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        let mut grouped: HashMap<i32, Vec<Note>> = HashMap::new();
        for row in rows {
            let save_id = row.save_id;
            grouped.entry(save_id).or_default().push(Note::from(row));
        }
        Ok(grouped)
    }

    pub async fn insert(db: &DatabaseConnection, save_id: i32, note: &Note) -> Result<()> {
        Self::insert_many(db, save_id, std::slice::from_ref(note)).await
    }

    /// 事务内批量写入；暂存笔记首次落档走这里，失败时调用方保留暂存待重试。
    pub async fn insert_many(
        db: &DatabaseConnection,
        save_id: i32,
        notes: &[Note],
    ) -> Result<()> {
        if notes.is_empty() {
            return Ok(());
        }
        let txn = db.begin().await.map_err(|e| anyhow!("{e}"))?;
        Self::insert_all(&txn, save_id, notes).await?;
        txn.commit().await.map_err(|e| anyhow!("{e}"))
    }

    pub async fn update(
        db: &DatabaseConnection,
        save_id: i32,
        id: &str,
        content: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<()> {
        let row = save_note::Entity::find_by_id((save_id, id.to_string()))
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .ok_or_else(|| anyhow!("笔记 {id} 不存在"))?;
        let mut active: save_note::ActiveModel = row.into();
        if let Some(content) = content {
            active.content = Set(content);
        }
        if let Some(tags) = tags {
            active.tags = Set(tags_to_json(&tags)?);
        }
        active.update(db).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn delete(db: &DatabaseConnection, save_id: i32, id: &str) -> Result<()> {
        let result = save_note::Entity::delete_many()
            .filter(save_note::Column::Id.eq(id.to_string()))
            .filter(save_note::Column::SaveId.eq(save_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        if result.rows_affected == 0 {
            return Err(anyhow!("笔记 {id} 不存在"));
        }
        Ok(())
    }

    pub async fn delete_by_save(db: &DatabaseConnection, save_id: i32) -> Result<()> {
        save_note::Entity::delete_many()
            .filter(save_note::Column::SaveId.eq(save_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    /// 用给定笔记整体替换某存档的笔记（新建/覆盖存档时保持「存档=快照」）。
    pub async fn replace_all(
        db: &DatabaseConnection,
        save_id: i32,
        notes: &[Note],
    ) -> Result<()> {
        let txn = db.begin().await.map_err(|e| anyhow!("{e}"))?;
        save_note::Entity::delete_many()
            .filter(save_note::Column::SaveId.eq(save_id))
            .exec(&txn)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Self::insert_all(&txn, save_id, notes).await?;
        txn.commit().await.map_err(|e| anyhow!("{e}"))
    }

    async fn insert_all(
        txn: &DatabaseTransaction,
        save_id: i32,
        notes: &[Note],
    ) -> Result<()> {
        for note in notes {
            save_note::ActiveModel {
                id: Set(note.id.clone()),
                save_id: Set(save_id),
                content: Set(note.content.clone()),
                tags: Set(tags_to_json(&note.tags)?),
                created_at: Set(note.created_at.clone()),
            }
            .insert(txn)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        }
        Ok(())
    }
}

fn tags_to_json(tags: &[String]) -> Result<String> {
    serde_json::to_string(tags).map_err(|e| anyhow!("序列化笔记标签失败: {e}"))
}

impl From<save_note::Model> for Note {
    fn from(row: save_note::Model) -> Self {
        Self {
            id: row.id,
            content: row.content,
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            created_at: row.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::Migrator;
    use sea_orm_migration::MigratorTrait;

    async fn test_db() -> (DatabaseConnection, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save_note_test.db");
        let db = Database::connect(format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .unwrap();
        Migrator::up(&db, None).await.unwrap();
        (db, dir)
    }

    fn note(content: &str, tags: &[&str]) -> Note {
        Note::new(
            content.to_string(),
            tags.iter().map(|tag| (*tag).to_string()).collect(),
        )
    }

    #[tokio::test]
    async fn crud_roundtrip() {
        let (db, _dir) = test_db().await;

        assert!(SaveNoteRepo::list(&db, 1).await.unwrap().is_empty());

        let first = note("对花生过敏", &["健康", "食物"]);
        let second = note("喜欢猫", &[]);
        SaveNoteRepo::insert(&db, 1, &first).await.unwrap();
        SaveNoteRepo::insert_many(&db, 1, std::slice::from_ref(&second))
            .await
            .unwrap();
        // 另一存档互不可见。
        SaveNoteRepo::insert(&db, 2, &note("别的档", &[]))
            .await
            .unwrap();

        let listed = SaveNoteRepo::list(&db, 1).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, first.id);
        assert_eq!(listed[0].tags, vec!["健康".to_string(), "食物".to_string()]);
        assert_eq!(listed[1].id, second.id);
        assert_eq!(SaveNoteRepo::list(&db, 2).await.unwrap().len(), 1);

        SaveNoteRepo::update(
            &db,
            1,
            &first.id,
            Some("其实喜欢狗".into()),
            Some(vec!["偏好".into()]),
        )
        .await
        .unwrap();
        let updated = SaveNoteRepo::list(&db, 1).await.unwrap();
        assert_eq!(updated[0].content, "其实喜欢狗");
        assert_eq!(updated[0].tags, vec!["偏好".to_string()]);

        // 跨存档按 id 更新/删除必须失败。
        assert!(
            SaveNoteRepo::update(&db, 2, &first.id, Some("x".into()), None)
                .await
                .is_err()
        );
        assert!(SaveNoteRepo::delete(&db, 2, &first.id).await.is_err());

        SaveNoteRepo::delete(&db, 1, &first.id).await.unwrap();
        assert_eq!(SaveNoteRepo::list(&db, 1).await.unwrap().len(), 1);

        // 快照复制：同一条笔记 id 可同时存在于两个存档，replace_all 清掉目标档原有内容。
        let snapshot = SaveNoteRepo::list(&db, 1).await.unwrap();
        SaveNoteRepo::replace_all(&db, 2, &snapshot).await.unwrap();
        let copied = SaveNoteRepo::list(&db, 2).await.unwrap();
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[0].id, second.id);
        assert_eq!(copied[0].content, "喜欢猫");

        // 空列表整体替换 = 清空该档。
        SaveNoteRepo::replace_all(&db, 2, &[]).await.unwrap();
        assert!(SaveNoteRepo::list(&db, 2).await.unwrap().is_empty());

        SaveNoteRepo::delete_by_save(&db, 1).await.unwrap();
        assert!(SaveNoteRepo::list(&db, 1).await.unwrap().is_empty());
    }
}
