//! 记忆存储回归测试：验证「长期记忆并入每「剧本×角色」共用归档表」的关键链路。
//!
//! 1. `unified_archive_smoke`：存量 `memory_embeddings` 迁移进归档表、长期记忆读写、
//!    旧版整合条目混读、计数/角色列表、分级删除——一个串行场景自洽；
//! 2. `fix_table_names_upgrade_path`：把历史建错的单数表名（`memory_embedding` /
//!    `memory_archive_profile`）修正为复数表名的升级路径。
//!
//! 用临时文件库（sqlx 连接池对 `:memory:` 每连接独立 schema，迁移与读写会落到不同连接）。

use anyhow::Result;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;

use ling_chat_main::ai_service::memory::store;

async fn migrate_and_browse_unified(db: &DatabaseConnection) -> Result<()> {
    // 1) 旧 memory_embeddings 存量数据（迁移前）
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO memory_embeddings \
         (role_id, memory_id, chunk_index, content, tags, embedding, dim, model, created_at) \
         VALUES (1, 'mem-1', 0, '旧版长期记忆内容', NULL, x'0000803f', 1, 'test-model', 1000)",
    ))
    .await?;

    // 2) 启动迁移
    let migrated = store::migrate_memory_embeddings_to_archives(db)
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(migrated, 1, "应迁移 1 条旧向量记忆");

    // 3) 长期记忆读取
    let rows = store::load_role_rows(db, 1).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].content, "旧版长期记忆内容");
    assert_eq!(rows[0].embedding.len(), 1, "embedding 应还原为 1 维 f32");

    // 4) 幂等：再跑迁移应为 0（旧表已清空）
    let again = store::migrate_memory_embeddings_to_archives(db)
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(again, 0, "重复迁移应为 0");

    // 5) 往同一张共用表写入 legacy 整合条目
    let table = store::ensure_role_archive(db, 1)
        .await
        .map_err(anyhow::Error::msg)?;
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        format!(
            "INSERT INTO \"{table}\" (kind, section, content, created_at) \
             VALUES ('legacy', 'short_term', '整合的旧版记忆', 2000)"
        ),
    ))
    .await?;

    // 6) 浏览统一视图读取
    let legacy = store::load_role_legacy_rows(db, 1).await?;
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy[0].content, "整合的旧版记忆");

    // 6.5) 检索候选：未带向量的整合条目不参与；补向量后即可被 memory_query 命中。
    let before = store::load_role_search_rows(db, 1).await?;
    assert_eq!(before.len(), 1, "未带向量的整合条目不应进入检索候选");
    store::save_legacy_embedding(db, &table, legacy[0].id, &[1.0f32], "test-model").await?;
    let after = store::load_role_search_rows(db, 1).await?;
    assert_eq!(after.len(), 2, "补向量后整合条目应进入检索候选");
    assert!(
        after
            .iter()
            .any(|r| r.memory_id == format!("legacy_{}", legacy[0].id)),
        "整合条目应以 legacy_{{id}} 作为检索 id"
    );

    // 7) 计数与角色列表
    assert_eq!(
        store::count_role_entries(db, 1).await?,
        2,
        "应为 1 条长期记忆 + 1 条整合"
    );
    assert_eq!(
        store::count_role_memories(db, 1).await?,
        1,
        "长期记忆计数不受 legacy 影响"
    );
    let rids = store::role_ids_with_memories(db).await?;
    assert!(rids.contains(&1), "角色 1 应出现在有记忆角色列表");

    // 8) 删除 legacy 单条，不误伤长期记忆
    let deleted = store::delete_legacy_row(db, 1, legacy[0].id).await?;
    assert_eq!(deleted, 1);
    assert_eq!(store::count_role_entries(db, 1).await?, 1);
    assert_eq!(
        store::load_role_rows(db, 1).await?.len(),
        1,
        "长期记忆应仍在"
    );

    // 9) 删除整张长期记忆（memory_id 级）
    let removed = store::delete_memory_scoped(db, 1, "mem-1").await?;
    assert_eq!(removed, 1);
    assert!(store::role_ids_with_memories(db).await?.is_empty());

    Ok(())
}

#[tokio::test]
async fn unified_archive_smoke() -> Result<()> {
    // 用临时文件库（sqlx 连接池对 :memory: 每连接独立 schema，迁移与读写会落到不同连接）。
    std::fs::create_dir_all("target")?;
    let path = std::path::Path::new("target")
        .join(format!("memory_archive_smoke_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let db = Database::connect(format!("sqlite:{}?mode=rwc", path.display())).await?;
    ling_chat_main::migration::Migrator::up(&db, None).await?;
    let result = migrate_and_browse_unified(&db).await;
    drop(db);
    let _ = std::fs::remove_file(&path);
    result
}

/// 升级路径：旧库的 seaql_migrations 记录 m20261007/09 已应用，但表被建成了单数名。
/// 升级后跑 Migrator 应应用修正迁移 m20261010：删单数错误表、按复数名补建。
#[tokio::test]
async fn fix_table_names_upgrade_path() -> Result<()> {
    std::fs::create_dir_all("target")?;
    let path = std::path::Path::new("target")
        .join(format!("memory_archive_fix_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let db = Database::connect(format!("sqlite:{}?mode=rwc", path.display())).await?;

    // 1) 全新库跑全部迁移（含修正），拿到 schema + seaql_migrations 记录。
    ling_chat_main::migration::Migrator::up(&db, None).await?;

    // 2) 模拟旧库：把修正迁移从已应用记录里抹掉，复数表删掉，换成错误名单数表。
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "DELETE FROM seaql_migrations WHERE version LIKE '%fix_memory_table_names%'",
    ))
    .await?;
    for sql in [
        "DROP TABLE memory_embeddings",
        "DROP TABLE memory_archive_profiles",
        "CREATE TABLE memory_embedding (id INTEGER PRIMARY KEY AUTOINCREMENT)",
        "CREATE TABLE memory_archive_profile (id INTEGER PRIMARY KEY AUTOINCREMENT)",
    ] {
        db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await?;
    }

    // 3) 升级：再跑 Migrator::up → 只应用未被记录的 m20261010。
    ling_chat_main::migration::Migrator::up(&db, None).await?;

    // 4) 断言：单数错误表已被删除、复数表已按正确结构补建（可用 INSERT 验证）。
    for sql in [
        "SELECT COUNT(*) + 0 FROM memory_embedding",
        "SELECT COUNT(*) + 0 FROM memory_archive_profile",
    ] {
        let err = db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .err();
        assert!(err.is_some(), "单数错误表应该已被修正迁移删除");
    }
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO memory_embeddings \
         (role_id, memory_id, chunk_index, content, embedding, dim, model, created_at) \
         VALUES (1, 'm', 0, 'x', x'0000803f', 1, 'm', 0)",
    ))
    .await?;
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "INSERT INTO memory_archive_profiles (role_id, role_name, table_name, created_at, updated_at) \
         VALUES (1, 'r', 'memory_archive_1', 0, 0)",
    ))
    .await?;

    drop(db);
    let _ = std::fs::remove_file(&path);
    Ok(())
}
