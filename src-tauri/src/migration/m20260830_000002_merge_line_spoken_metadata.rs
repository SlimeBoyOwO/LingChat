use std::collections::HashMap;

use sea_orm::{QueryResult, Statement};
use sea_orm_migration::prelude::*;

const CONTENT_KEY: &str = "content";
const LANGUAGE_KEY: &str = "language";

/// 将早期 PR 版本的 spoken_content / spoken_language 双列合并为一个可扩展 JSON 哈希表。
///
/// 保留上一条迁移不变，使已经运行过开发版的数据库也能通过本迁移无损升级；新数据库
/// 会先创建旧双列，再立即合并到 spoken 单列。
#[derive(DeriveMigrationName)]
pub struct Migration;

async fn has_column(manager: &SchemaManager<'_>, name: &str) -> Result<bool, DbErr> {
    let sql = format!(
        "SELECT name FROM pragma_table_info('line') WHERE name = '{}'",
        name.replace('\'', "''")
    );
    let rows: Vec<QueryResult> = manager
        .get_connection()
        .query_all(Statement::from_string(manager.get_database_backend(), sql))
        .await?;
    Ok(!rows.is_empty())
}

fn merge_metadata(
    existing: Option<&str>,
    content: Option<String>,
    language: Option<String>,
) -> HashMap<String, String> {
    let mut spoken: HashMap<String, String> = existing
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    if let Some(content) = content.filter(|value| !value.is_empty()) {
        spoken.entry(CONTENT_KEY.to_string()).or_insert(content);
    }
    if let Some(language) = language.filter(|value| !value.is_empty()) {
        spoken.entry(LANGUAGE_KEY.to_string()).or_insert(language);
    }
    spoken
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !has_column(manager, "spoken").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .add_column(ColumnDef::new(Line::Spoken).text().null())
                        .to_owned(),
                )
                .await?;
        }

        let has_content = has_column(manager, "spoken_content").await?;
        let has_language = has_column(manager, "spoken_language").await?;
        if has_content || has_language {
            let content_expr = if has_content {
                "spoken_content"
            } else {
                "NULL"
            };
            let language_expr = if has_language {
                "spoken_language"
            } else {
                "NULL"
            };
            let rows = manager
                .get_connection()
                .query_all(Statement::from_string(
                    manager.get_database_backend(),
                    format!(
                        "SELECT id, spoken, {content_expr} AS legacy_content, \
                         {language_expr} AS legacy_language FROM line"
                    ),
                ))
                .await?;
            for row in rows {
                let id: i32 = row.try_get("", "id")?;
                let existing: Option<String> = row.try_get("", "spoken")?;
                let content: Option<String> = row.try_get("", "legacy_content")?;
                let language: Option<String> = row.try_get("", "legacy_language")?;
                let spoken = merge_metadata(existing.as_deref(), content, language);
                let encoded = if spoken.is_empty() {
                    None
                } else {
                    Some(
                        serde_json::to_string(&spoken)
                            .map_err(|error| DbErr::Custom(error.to_string()))?,
                    )
                };
                manager
                    .get_connection()
                    .execute(Statement::from_sql_and_values(
                        manager.get_database_backend(),
                        "UPDATE line SET spoken = ? WHERE id = ?",
                        [encoded.into(), id.into()],
                    ))
                    .await?;
            }
        }

        if has_content {
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .drop_column(Line::SpokenContent)
                        .to_owned(),
                )
                .await?;
        }
        if has_language {
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .drop_column(Line::SpokenLanguage)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !has_column(manager, "spoken_content").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .add_column(ColumnDef::new(Line::SpokenContent).text().null())
                        .to_owned(),
                )
                .await?;
        }
        if !has_column(manager, "spoken_language").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .add_column(ColumnDef::new(Line::SpokenLanguage).string_len(32).null())
                        .to_owned(),
                )
                .await?;
        }

        if has_column(manager, "spoken").await? {
            let rows = manager
                .get_connection()
                .query_all(Statement::from_string(
                    manager.get_database_backend(),
                    "SELECT id, spoken FROM line".to_string(),
                ))
                .await?;
            for row in rows {
                let id: i32 = row.try_get("", "id")?;
                let encoded: Option<String> = row.try_get("", "spoken")?;
                let spoken: HashMap<String, String> = encoded
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok())
                    .unwrap_or_default();
                manager
                    .get_connection()
                    .execute(Statement::from_sql_and_values(
                        manager.get_database_backend(),
                        "UPDATE line SET spoken_content = ?, spoken_language = ? WHERE id = ?",
                        [
                            spoken.get(CONTENT_KEY).cloned().into(),
                            spoken.get(LANGUAGE_KEY).cloned().into(),
                            id.into(),
                        ],
                    ))
                    .await?;
            }
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .drop_column(Line::Spoken)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Line {
    Table,
    Spoken,
    SpokenContent,
    SpokenLanguage,
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::{CONTENT_KEY, LANGUAGE_KEY, Migration, merge_metadata};

    #[test]
    fn legacy_columns_merge_without_overwriting_future_keys() {
        let existing = r#"{"provider":"indextts","content":"new"}"#;
        let merged = merge_metadata(
            Some(existing),
            Some("legacy".to_string()),
            Some("ja".to_string()),
        );
        assert_eq!(merged.get(CONTENT_KEY).map(String::as_str), Some("new"));
        assert_eq!(merged.get(LANGUAGE_KEY).map(String::as_str), Some("ja"));
        assert_eq!(merged.get("provider").map(String::as_str), Some("indextts"));
    }

    #[tokio::test]
    async fn migration_round_trips_legacy_columns_on_sqlite() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE line (id INTEGER PRIMARY KEY, spoken_content TEXT, spoken_language VARCHAR(32));\
             INSERT INTO line (id, spoken_content, spoken_language) VALUES (1, 'こんにちは', 'ja');",
        )
        .await
        .unwrap();
        let manager = SchemaManager::new(&db);
        let migration = Migration;

        migration.up(&manager).await.unwrap();
        let row = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                "SELECT spoken FROM line WHERE id = 1".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let encoded: Option<String> = row.try_get("", "spoken").unwrap();
        let spoken: std::collections::HashMap<String, String> =
            serde_json::from_str(encoded.as_deref().unwrap()).unwrap();
        assert_eq!(
            spoken.get(CONTENT_KEY).map(String::as_str),
            Some("こんにちは")
        );
        assert_eq!(spoken.get(LANGUAGE_KEY).map(String::as_str), Some("ja"));
        assert!(!super::has_column(&manager, "spoken_content").await.unwrap());
        assert!(
            !super::has_column(&manager, "spoken_language")
                .await
                .unwrap()
        );

        migration.down(&manager).await.unwrap();
        let row = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                "SELECT spoken_content, spoken_language FROM line WHERE id = 1".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<Option<String>>("", "spoken_content")
                .unwrap()
                .as_deref(),
            Some("こんにちは")
        );
        assert_eq!(
            row.try_get::<Option<String>>("", "spoken_language")
                .unwrap()
                .as_deref(),
            Some("ja")
        );
        assert!(!super::has_column(&manager, "spoken").await.unwrap());
    }
}
