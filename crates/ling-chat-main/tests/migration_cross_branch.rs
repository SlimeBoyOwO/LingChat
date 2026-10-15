//! 临时验证（验证完即删）：跨分支迁移记录不再导致启动失败。
//!
//! 复现平板上的崩溃：库里存在「本构建没有对应迁移文件」的已应用记录时，
//! `Migrator::up` 会直接报错；`init_db` 应容忍并保留这些记录。

use std::path::{Path, PathBuf};

use ling_chat_main::db::init_db;
use ling_chat_main::migration::Migrator;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;

const FOREIGN: &str = "m20261101_000001_some_other_branch";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lingchat-migration-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn connect(dir: &Path) -> DatabaseConnection {
    let url = format!("sqlite:{}?mode=rwc", dir.join("game_database.db").display());
    Database::connect(&url).await.unwrap()
}

async fn versions(db: &DatabaseConnection) -> Vec<String> {
    db.query_all(Statement::from_string(
        DatabaseBackend::Sqlite,
        "SELECT version FROM seaql_migrations ORDER BY version",
    ))
    .await
    .unwrap()
    .into_iter()
    .map(|row| row.try_get::<String>("", "version").unwrap())
    .collect()
}

#[test]
fn foreign_records_are_tolerated_and_preserved() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let dir = temp_dir("foreign");
        let expected = Migrator::migrations().len();

        // 1) 全新库：正常初始化，账本里应有全部迁移。
        let db = init_db(&dir).await.expect("首次初始化失败");
        let applied = versions(&db).await;
        assert_eq!(applied.len(), expected, "首次初始化后账本不完整");
        drop(db);

        // 2) 模拟「其它分支建的库」：多一条本构建没有的记录。
        let db = connect(&dir).await;
        db.execute_unprepared(&format!(
            "INSERT INTO seaql_migrations (version, applied_at) VALUES ('{FOREIGN}', 1)"
        ))
        .await
        .unwrap();

        // 3) 旧路径（sea-orm 的 Migrator::up）应当因此报错 —— 这就是平板崩溃的原因。
        let err = Migrator::up(&db, None)
            .await
            .expect_err("Migrator::up 竟然没有报错");
        let text = err.to_string();
        assert!(text.contains("is missing"), "非预期错误: {text}");
        assert!(text.contains(FOREIGN), "错误里没提到外部记录: {text}");
        println!("旧路径报错: {text}");
        drop(db);

        // 4) 新路径：init_db 容忍未知记录，并且不删除它。
        let db = init_db(&dir)
            .await
            .expect("未知迁移记录不应再导致初始化失败");
        let applied = versions(&db).await;
        assert!(
            applied.iter().any(|v| v == FOREIGN),
            "外部记录被删掉了: {applied:?}"
        );
        assert_eq!(applied.len(), expected + 1, "账本被改动: {applied:?}");

        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn missing_pending_records_are_reapplied() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let dir = temp_dir("pending");
        let db = init_db(&dir).await.expect("首次初始化失败");
        let applied = versions(&db).await;
        drop(db);

        // 抹掉一条 memory 分支的记账，模拟「另一个分支缺少该迁移」的库。
        let victim = applied
            .iter()
            .find(|v| v.starts_with("m20261007"))
            .expect("缺少 memory 迁移")
            .clone();
        let db = connect(&dir).await;
        db.execute_unprepared(&format!(
            "DELETE FROM seaql_migrations WHERE version = '{victim}'"
        ))
        .await
        .unwrap();
        drop(db);

        // 待应用的迁移应被重新执行并记账（DDL 幂等，不应报错）。
        let db = init_db(&dir).await.expect("重跑待应用迁移失败");
        let applied = versions(&db).await;
        assert!(
            applied.iter().any(|v| v == &victim),
            "待应用迁移没有被记回账本: {applied:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    });
}
