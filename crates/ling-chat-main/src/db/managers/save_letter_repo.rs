use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use sea_orm::*;

use crate::db::entities::save_letter;

/// 存档信纸仓库：库里只记归属与页序，正文按 id 存成 data/letters 下的 md 文件。
pub struct SaveLetterRepo;

impl SaveLetterRepo {
    /// 按页序列出某存档的信纸 id。
    pub async fn list_ids(db: &DatabaseConnection, save_id: i32) -> Result<Vec<String>> {
        let rows = save_letter::Entity::find()
            .filter(save_letter::Column::SaveId.eq(save_id))
            .order_by_asc(save_letter::Column::Page)
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(rows.into_iter().map(|row| row.id).collect())
    }

    /// 用给定 id 序列整体替换某存档的信纸，页序即数组下标。
    pub async fn replace_all(db: &DatabaseConnection, save_id: i32, ids: &[String]) -> Result<()> {
        let txn = db.begin().await.map_err(|e| anyhow!("{e}"))?;
        save_letter::Entity::delete_many()
            .filter(save_letter::Column::SaveId.eq(save_id))
            .exec(&txn)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        for (page, id) in ids.iter().enumerate() {
            save_letter::ActiveModel {
                id: Set(id.clone()),
                save_id: Set(save_id),
                page: Set(page as i32),
            }
            .insert(&txn)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        }
        txn.commit().await.map_err(|e| anyhow!("{e}"))
    }

    async fn all_ids(db: &DatabaseConnection) -> Result<HashSet<String>> {
        let ids: Vec<String> = save_letter::Entity::find()
            .select_only()
            .column(save_letter::Column::Id)
            .into_tuple()
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(ids.into_iter().collect())
    }
}

pub fn letter_path(id: &str) -> PathBuf {
    crate::api::letters_dir().join(format!("{id}.md"))
}

pub fn new_letter_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn read_letter(id: &str) -> Result<String> {
    let path = letter_path(id);
    std::fs::read_to_string(&path).map_err(|e| anyhow!("读取信纸 {} 失败: {e}", path.display()))
}

pub fn write_letter(id: &str, content: &str) -> Result<()> {
    crate::ai_service::tools::atomic_replace(&letter_path(id), content.as_bytes())
        .map_err(|e| anyhow!(e))
}

/// 把信纸复制成一组新 id（另存到别的存档时用，保证一张信纸只属于一个存档）。
pub fn copy_letters(ids: &[String]) -> Result<Vec<String>> {
    let mut copied = Vec::with_capacity(ids.len());
    for id in ids {
        let new_id = new_letter_id();
        std::fs::copy(letter_path(id), letter_path(&new_id))
            .map_err(|e| anyhow!("复制信纸 {id} 失败: {e}"))?;
        copied.push(new_id);
    }
    Ok(copied)
}

/// 启动时删除 data/letters 下未被任何存档引用的文件（含未存档会话留下的信纸）。
pub async fn cleanup_orphan_letter_files(db: &DatabaseConnection) -> Result<u64> {
    let dir = crate::api::letters_dir();
    if !dir.exists() {
        return Ok(0);
    }
    let referenced = SaveLetterRepo::all_ids(db).await?;
    let mut deleted = 0u64;
    for entry in std::fs::read_dir(&dir).map_err(|e| anyhow!("读取信纸目录失败: {e}"))? {
        let entry = entry.map_err(|e| anyhow!("遍历信纸目录失败: {e}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let keep = path
            .extension()
            .is_some_and(|ext| ext == "md")
            .then(|| path.file_stem())
            .flatten()
            .is_some_and(|stem| referenced.contains(&*stem.to_string_lossy()));
        if !keep {
            std::fs::remove_file(&path)
                .map_err(|e| anyhow!("删除孤立信纸 {} 失败: {e}", path.display()))?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::Migrator;
    use sea_orm_migration::MigratorTrait;

    #[tokio::test]
    async fn replace_all_keeps_page_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save_letter_test.db");
        let db = Database::connect(format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .unwrap();
        Migrator::up(&db, None).await.unwrap();

        let ids = vec!["b".to_string(), "a".to_string(), "c".to_string()];
        SaveLetterRepo::replace_all(&db, 1, &ids).await.unwrap();
        SaveLetterRepo::replace_all(&db, 2, &["d".to_string()])
            .await
            .unwrap();
        assert_eq!(SaveLetterRepo::list_ids(&db, 1).await.unwrap(), ids);

        SaveLetterRepo::replace_all(&db, 1, &ids[..1])
            .await
            .unwrap();
        assert_eq!(SaveLetterRepo::list_ids(&db, 1).await.unwrap(), vec!["b"]);
        assert_eq!(SaveLetterRepo::all_ids(&db).await.unwrap().len(), 2);
    }
}
