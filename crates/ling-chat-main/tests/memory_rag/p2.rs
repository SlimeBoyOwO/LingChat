use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::{Result, bail};
use ling_chat_main::ai_service::game_system::memory_index::{
    IndexMode, MemoryEncoder, MemoryIndexer,
};
use ling_chat_main::ai_service::game_system::memory_scope::MemoryScopeSnapshot;
use ling_chat_main::db::managers::memory_index_repo::MemoryIndexRepo;
use ling_chat_notes::memory::{EmbeddingVersion, MemoryKind, MemorySource};

use super::*;

struct Encoder {
    model: &'static str,
    calls: AtomicUsize,
    token_calls: AtomicUsize,
    max_tokens: AtomicUsize,
    fail: AtomicBool,
    no_tokenizer: AtomicBool,
    started: Option<Arc<tokio::sync::Notify>>,
    release: Option<Arc<tokio::sync::Notify>>,
}

impl Encoder {
    fn new(model: &'static str) -> Arc<Self> {
        Arc::new(Self {
            model,
            calls: AtomicUsize::new(0),
            token_calls: AtomicUsize::new(0),
            max_tokens: AtomicUsize::new(512),
            fail: AtomicBool::new(false),
            no_tokenizer: AtomicBool::new(false),
            started: None,
            release: None,
        })
    }
}

impl MemoryEncoder for Encoder {
    fn version(&self) -> EmbeddingVersion {
        EmbeddingVersion {
            model: self.model.into(),
            dimension: 3,
            preprocessing: "test-v1".into(),
        }
    }
    fn max_tokens(&self) -> usize {
        self.max_tokens.load(Ordering::SeqCst)
    }
    fn count_tokens(&self, text: &str) -> Result<usize> {
        self.token_calls.fetch_add(1, Ordering::SeqCst);
        if self.no_tokenizer.load(Ordering::SeqCst) {
            bail!("模拟缺失分词器");
        }
        Ok(text.chars().count() + 2)
    }
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            if let (Some(started), Some(release)) = (&self.started, &self.release) {
                started.notify_one();
                tokio::runtime::Handle::current().block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(30), release.notified())
                        .await
                })?;
            }
        }
        if self.fail.load(Ordering::SeqCst) {
            bail!("模拟推理失败");
        }
        Ok(vec![1.0, 0.0, 0.0])
    }
}

async fn sync(
    indexer: &MemoryIndexer,
    db: &DatabaseConnection,
    dir: &tempfile::TempDir,
) -> ling_chat_main::ai_service::game_system::memory_index::MemoryIndexReport {
    indexer
        .sync(
            db,
            &NotesStore::new(dir.path()),
            101,
            2,
            "测试角色甲",
            IndexMode::Incremental,
        )
        .await
        .unwrap()
}

async fn scope(db: &DatabaseConnection, dir: &tempfile::TempDir) -> MemoryScopeSnapshot {
    MemoryScopeSnapshot::load(db, &NotesStore::new(dir.path()), 101, 2, "测试角色甲")
        .await
        .unwrap()
}

async fn execute(db: &DatabaseConnection, sql: &str) {
    db.execute(Statement::from_string(DbBackend::Sqlite, sql))
        .await
        .unwrap();
}

#[tokio::test]
async fn restart_reuses_vectors_and_repeated_sync_does_not_rewrite_rows() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    let first = sync(&indexer, &db, &dir).await;
    assert!(first.embedded > 0 && first.lexical_only == 0);
    let before = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &encoder.version())
        .await
        .unwrap();
    encoder.token_calls.store(0, Ordering::SeqCst);
    let second = sync(&indexer, &db, &dir).await;
    assert_eq!(encoder.token_calls.load(Ordering::SeqCst), 0);
    assert_eq!(second.embedded, 0);
    assert_eq!(second.changed_documents, 0);
    assert_eq!(second.reused, first.embedded);
    assert_eq!(first.index_version, second.index_version);
    let after = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &encoder.version())
        .await
        .unwrap();
    assert_eq!(before.generation, after.generation);
    assert_eq!(before.documents, after.documents);
    db.close().await.unwrap();
    let reopened = db::init_db(dir.path()).await.unwrap();
    let fresh_encoder = Encoder::new("model-a");
    let third = sync(&MemoryIndexer::new(fresh_encoder.clone()), &reopened, &dir).await;
    assert_eq!(fresh_encoder.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fresh_encoder.token_calls.load(Ordering::SeqCst), 0);
    assert_eq!(third.reused, first.embedded);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn editing_one_source_changes_only_its_document_and_keeps_original_data() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    sync(&indexer, &db, &dir).await;
    let old_scope = scope(&db, &dir).await;
    let before = MemoryIndexRepo::load(&db, &old_scope, &encoder.version())
        .await
        .unwrap();
    execute(
        &db,
        "UPDATE line SET content='我买奶茶要五分糖。' WHERE id=1",
    )
    .await;
    let current = scope(&db, &dir).await;
    let filtered = MemoryIndexRepo::load(&db, &current, &encoder.version())
        .await
        .unwrap();
    assert!(!filtered.documents.iter().any(|item| {
        item.document
            .sources
            .iter()
            .any(|source| matches!(source, MemorySource::Line { line_id: 1, .. }))
    }));
    let report = sync(&indexer, &db, &dir).await;
    assert_eq!(report.changed_documents, 1);
    assert!(report.embedded > 0 && report.embedded < report.chunks);
    let after = MemoryIndexRepo::load(&db, &current, &encoder.version())
        .await
        .unwrap();
    for unchanged in before.documents.iter().filter(|item| {
        !item
            .document
            .sources
            .iter()
            .any(|source| matches!(source, MemorySource::Line { line_id: 1, .. }))
    }) {
        assert!(after.documents.contains(unchanged));
    }
    assert_eq!(SaveRepo::get_line_list(&db, 101).await.unwrap().len(), 44);
    db.close().await.unwrap();
}

#[tokio::test]
async fn external_note_changes_sync_and_save_removal_preserves_role_notes() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    sync(&indexer, &db, &dir).await;
    let store = NotesStore::new(dir.path());
    let mut notes = store.load("测试角色甲").unwrap();
    notes
        .iter_mut()
        .find(|note| note.id == "note_legacy")
        .unwrap()
        .content = "猫叫圆圆".into();
    std::fs::write(
        dir.path().join("game_data/notes/测试角色甲.json"),
        serde_json::to_vec(&notes).unwrap(),
    )
    .unwrap();
    let report = sync(&indexer, &db, &dir).await;
    assert_eq!(report.changed_documents, 1);
    store.delete("测试角色甲", "note_coffee").unwrap();
    sync(&indexer, &db, &dir).await;
    let loaded = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &encoder.version())
        .await
        .unwrap();
    assert_eq!(
        loaded
            .documents
            .iter()
            .filter(|item| item.document.kind == MemoryKind::Note)
            .count(),
        1
    );
    SaveRepo::delete_save(&db, 101).await.unwrap();
    let notes_left = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) FROM memory_document WHERE kind='note'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap();
    assert_eq!(notes_left, 1);
    let raw_left = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) FROM memory_document WHERE save_id=101",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap();
    assert_eq!(raw_left, 0);
    assert_eq!(store.load("测试角色甲").unwrap()[0].content, "猫叫圆圆");
    db.close().await.unwrap();
}

#[tokio::test]
async fn failure_preserves_lexical_index_and_retry_or_model_change_rebuilds_vectors() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    encoder.fail.store(true, Ordering::SeqCst);
    let indexer = MemoryIndexer::new(encoder.clone());
    let failed = sync(&indexer, &db, &dir).await;
    assert_eq!(failed.lexical_only, failed.chunks);
    assert!(failed.embedding_failures > 0);
    encoder.fail.store(false, Ordering::SeqCst);
    let retry = sync(&indexer, &db, &dir).await;
    assert_eq!(retry.embedded, failed.chunks);
    let replacement = Encoder::new("model-b");
    let before = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &replacement.version())
        .await
        .unwrap();
    assert!(
        before
            .documents
            .iter()
            .flat_map(|item| &item.chunks)
            .all(|item| item.embedding.is_none())
    );
    let changed = sync(&MemoryIndexer::new(replacement.clone()), &db, &dir).await;
    assert_eq!(changed.embedded, retry.chunks);
    assert_ne!(changed.index_version, retry.index_version);
    let rebuilt = MemoryIndexer::new(replacement.clone())
        .sync(
            &db,
            &NotesStore::new(dir.path()),
            101,
            2,
            "测试角色甲",
            IndexMode::Rebuild,
        )
        .await
        .unwrap();
    assert_eq!(rebuilt.embedded, changed.chunks);
    assert_eq!(SaveRepo::get_line_list(&db, 101).await.unwrap().len(), 44);
    db.close().await.unwrap();
}

#[tokio::test]
async fn missing_tokenizer_and_corrupt_derived_rows_can_recover() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    encoder.no_tokenizer.store(true, Ordering::SeqCst);
    let indexer = MemoryIndexer::new(encoder.clone());
    let fallback = sync(&indexer, &db, &dir).await;
    assert_eq!(fallback.lexical_only, fallback.chunks);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), 0);
    encoder.no_tokenizer.store(false, Ordering::SeqCst);
    let recovered = sync(&indexer, &db, &dir).await;
    assert!(recovered.embedded > 0);
    execute(
        &db,
        "UPDATE memory_chunk SET payload='corrupt' WHERE id=(SELECT id FROM memory_chunk LIMIT 1)",
    )
    .await;
    execute(&db, "UPDATE memory_embedding SET payload='corrupt' WHERE chunk_id=(SELECT chunk_id FROM memory_embedding LIMIT 1)").await;
    let repair = sync(&indexer, &db, &dir).await;
    assert!(repair.changed_documents > 0);
    let loaded = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &encoder.version())
        .await
        .unwrap();
    assert_eq!(
        loaded
            .documents
            .iter()
            .map(|item| item.chunks.len())
            .sum::<usize>(),
        recovered.chunks
    );
    assert!(
        loaded
            .documents
            .iter()
            .flat_map(|item| &item.chunks)
            .all(|item| item.embedding.is_some())
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn history_changes_during_inference_reject_the_entire_old_commit() {
    let (dir, db, _) = seed().await;
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let encoder = Arc::new(Encoder {
        model: "slow",
        calls: AtomicUsize::new(0),
        token_calls: AtomicUsize::new(0),
        max_tokens: AtomicUsize::new(512),
        fail: AtomicBool::new(false),
        no_tokenizer: AtomicBool::new(false),
        started: Some(started.clone()),
        release: Some(release.clone()),
    });
    let indexer = MemoryIndexer::new(encoder);
    let worker_db = db.clone();
    let store = NotesStore::new(dir.path());
    let worker = tokio::spawn(async move {
        indexer
            .sync(
                &worker_db,
                &store,
                101,
                2,
                "测试角色甲",
                IndexMode::Incremental,
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(30), started.notified())
        .await
        .unwrap();
    let update = SaveRepo::update_save_last_message(&db, 101, Some(2)).await;
    release.notify_one();
    update.unwrap();
    assert!(worker.await.unwrap().is_err());
    let rows = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) FROM memory_document",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap();
    assert_eq!(rows, 0);
    assert_eq!(SaveRepo::get_line_list(&db, 101).await.unwrap().len(), 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn oversized_tool_transactions_keep_complete_lexical_chunks() {
    let (dir, db, _) = seed().await;
    let call = serde_json::json!([{"id":"big","type":"function","function":{"name":"lookup","arguments":"{}"}}]).to_string();
    let result =
        serde_json::json!({"tool_call_id":"big","result":"工具内容".repeat(200)}).to_string();
    db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO line(id,save_id,parent_line_id,attribute,content,tool_call) VALUES (4000,101,1036,'assistant','查询记录',?)",
        vec![call.into()])).await.unwrap();
    db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO line(id,save_id,parent_line_id,attribute,content) VALUES (4001,101,4000,'tool',?)",
        vec![result.into()])).await.unwrap();
    execute(
        &db,
        "INSERT INTO line_perception(line_id,role_id) VALUES (4000,2),(4001,2)",
    )
    .await;
    SaveRepo::update_save_last_message(&db, 101, Some(4001))
        .await
        .unwrap();
    let encoder = Encoder::new("model-a");
    let report = sync(&MemoryIndexer::new(encoder.clone()), &db, &dir).await;
    assert_eq!(report.oversized_chunks, 1);
    let loaded = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &encoder.version())
        .await
        .unwrap();
    let tool_document = loaded
        .documents
        .iter()
        .find(|item| {
            item.document
                .sources
                .iter()
                .any(|source| matches!(source, MemorySource::Line { line_id: 4000, .. }))
        })
        .unwrap();
    assert!(
        tool_document
            .document
            .sources
            .iter()
            .any(|source| matches!(source, MemorySource::Line { line_id: 4001, .. }))
    );
    assert_eq!(tool_document.chunks.len(), 1);
    assert_eq!(
        tool_document.chunks[0].chunk.content,
        tool_document.document.content
    );
    assert!(tool_document.chunks[0].embedding.is_none());
    assert!(
        tool_document
            .document
            .content
            .contains(&"工具内容".repeat(200))
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn database_write_failure_rolls_back_derived_data_and_checkpoint() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    sync(&indexer, &db, &dir).await;
    let old_scope = scope(&db, &dir).await;
    let before = MemoryIndexRepo::load(&db, &old_scope, &encoder.version())
        .await
        .unwrap();
    execute(&db, "UPDATE line SET content='修改后的奶茶偏好' WHERE id=1").await;
    execute(&db, "CREATE TRIGGER fail_index BEFORE INSERT ON memory_chunk BEGIN SELECT RAISE(ABORT,'test write failure'); END").await;
    assert!(
        indexer
            .sync(
                &db,
                &NotesStore::new(dir.path()),
                101,
                2,
                "测试角色甲",
                IndexMode::Incremental
            )
            .await
            .is_err()
    );
    let after = MemoryIndexRepo::load(&db, &old_scope, &encoder.version())
        .await
        .unwrap();
    assert_eq!(before.documents, after.documents);
    assert_eq!(before.generation, after.generation);
    assert_eq!(
        SaveRepo::get_line_list(&db, 101).await.unwrap()[0].content,
        "修改后的奶茶偏好"
    );
    execute(&db, "DROP TRIGGER fail_index").await;
    assert_eq!(sync(&indexer, &db, &dir).await.changed_documents, 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn persisted_candidates_stay_scoped_before_and_after_rewind() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    let store = NotesStore::new(dir.path());
    for (save_id, role_id, name) in [
        (101, 2, "测试角色甲"),
        (101, 3, "测试角色乙"),
        (102, 2, "测试角色甲"),
        (103, 2, "测试角色甲"),
    ] {
        indexer
            .sync(&db, &store, save_id, role_id, name, IndexMode::Incremental)
            .await
            .unwrap();
        let scoped = MemoryScopeSnapshot::load(&db, &store, save_id, role_id, name)
            .await
            .unwrap();
        let loaded = MemoryIndexRepo::load(&db, &scoped, &encoder.version())
            .await
            .unwrap();
        assert!(!loaded.documents.is_empty());
        assert!(loaded.documents.iter().all(|item| {
            scoped.allows_document(&item.document)
                && item
                    .chunks
                    .iter()
                    .all(|chunk| scoped.allows_chunk(&item.document, &chunk.chunk))
        }));
        if role_id == 2 {
            assert!(
                !loaded
                    .documents
                    .iter()
                    .any(|item| item.document.content.contains("蓝色纸鹤")
                        || item.document.content.contains("绿色花盆"))
            );
        }
    }
    SaveRepo::update_save_last_message(&db, 101, Some(2))
        .await
        .unwrap();
    let rewound = scope(&db, &dir).await;
    let loaded = MemoryIndexRepo::load(&db, &rewound, &encoder.version())
        .await
        .unwrap();
    assert!(
        loaded
            .documents
            .iter()
            .filter(|item| item.document.kind == MemoryKind::Raw)
            .all(|item| item
                .document
                .sources
                .iter()
                .all(|source| matches!(source, MemorySource::Line { line_id: 1 | 2, .. })))
    );
    sync(&indexer, &db, &dir).await;
    let loaded = MemoryIndexRepo::load(&db, &rewound, &encoder.version())
        .await
        .unwrap();
    assert!(
        loaded
            .documents
            .iter()
            .any(|item| item.document.kind == MemoryKind::Raw)
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn upgrading_an_existing_database_preserves_history_and_notes() {
    use ling_chat_main::migration::Migrator;
    use sea_orm_migration::MigratorTrait;

    let (dir, db, _) = seed().await;
    let before = SaveRepo::get_gameline_list(&db, 101).await.unwrap();
    let note_bytes = std::fs::read(dir.path().join("game_data/notes/测试角色甲.json")).unwrap();
    Migrator::down(&db, Some(2)).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(before, SaveRepo::get_gameline_list(&db, 101).await.unwrap());
    assert_eq!(
        note_bytes,
        std::fs::read(dir.path().join("game_data/notes/测试角色甲.json")).unwrap()
    );
    let encoder = Encoder::new("model-a");
    let report = sync(&MemoryIndexer::new(encoder), &db, &dir).await;
    assert!(report.embedded > 0);
    db.close().await.unwrap();
}

#[tokio::test]
async fn an_older_worker_cannot_replace_a_newer_committed_generation() {
    let (dir, db, _) = seed().await;
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let encoder = Arc::new(Encoder {
        model: "model-a",
        calls: AtomicUsize::new(0),
        token_calls: AtomicUsize::new(0),
        max_tokens: AtomicUsize::new(512),
        fail: AtomicBool::new(false),
        no_tokenizer: AtomicBool::new(false),
        started: Some(started.clone()),
        release: Some(release.clone()),
    });
    let worker_db = db.clone();
    let store = NotesStore::new(dir.path());
    let worker = tokio::spawn(async move {
        MemoryIndexer::new(encoder)
            .sync(
                &worker_db,
                &store,
                101,
                2,
                "测试角色甲",
                IndexMode::Incremental,
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(30), started.notified())
        .await
        .unwrap();
    let newer_encoder = Encoder::new("model-a");
    let newer = MemoryIndexer::new(newer_encoder.clone())
        .sync(
            &db,
            &NotesStore::new(dir.path()),
            101,
            2,
            "测试角色甲",
            IndexMode::Incremental,
        )
        .await;
    release.notify_one();
    let newer = newer.unwrap();
    assert!(worker.await.unwrap().is_err());
    let loaded = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &newer_encoder.version())
        .await
        .unwrap();
    assert_eq!(loaded.generation, 1);
    assert_eq!(loaded.index_version.as_ref(), Some(&newer.index_version));
    db.close().await.unwrap();
}

#[tokio::test]
async fn metadata_backfill_reuses_vectors_without_tokenization() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    let before = sync(&indexer, &db, &dir).await;
    execute(&db, "UPDATE line SET audio_file='backfill.wav',tts_content='语音文本',predicted_emotion='happy',thinking='隐藏思考' WHERE id=7").await;
    encoder.token_calls.store(0, Ordering::SeqCst);
    let after = sync(&indexer, &db, &dir).await;
    assert_eq!(after.scope_key, before.scope_key);
    assert_eq!(after.index_version, before.index_version);
    assert_eq!(after.changed_documents, 0);
    assert_eq!(after.embedded, 0);
    assert_eq!(after.reused, before.chunks);
    assert_eq!(encoder.token_calls.load(Ordering::SeqCst), 0);
    db.close().await.unwrap();
}

#[tokio::test]
async fn chunk_layout_budget_version_and_missing_rows_require_repair() {
    let (dir, db, _) = seed().await;
    NotesStore::new(dir.path())
        .add("测试角色甲", "猫".repeat(160), vec![])
        .unwrap();
    let encoder = Encoder::new("model-a");
    let first = sync(&MemoryIndexer::new(encoder.clone()), &db, &dir).await;
    encoder.max_tokens.store(64, Ordering::SeqCst);
    encoder.token_calls.store(0, Ordering::SeqCst);
    let smaller = MemoryIndexer::new(encoder.clone());
    let changed = sync(&smaller, &db, &dir).await;
    assert!(encoder.token_calls.load(Ordering::SeqCst) > 0);
    assert!(changed.chunks > first.chunks);
    encoder.token_calls.store(0, Ordering::SeqCst);
    sync(&smaller, &db, &dir).await;
    assert_eq!(encoder.token_calls.load(Ordering::SeqCst), 0);
    execute(
        &db,
        "UPDATE memory_document SET chunking_version='older-algorithm'",
    )
    .await;
    let updated = sync(&smaller, &db, &dir).await;
    assert!(updated.changed_documents > 0 && updated.embedded > 0);
    execute(
        &db,
        "DELETE FROM memory_chunk WHERE id=(SELECT id FROM memory_chunk LIMIT 1)",
    )
    .await;
    let repaired = sync(&smaller, &db, &dir).await;
    assert!(repaired.changed_documents > 0 && repaired.embedded > 0);
    assert_eq!(repaired.chunks, changed.chunks);
    db.close().await.unwrap();
}

#[tokio::test]
async fn upgrading_old_index_layout_metadata_preserves_sources_and_repairs_once() {
    use ling_chat_main::migration::Migrator;
    use sea_orm_migration::MigratorTrait;

    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    let first = sync(&indexer, &db, &dir).await;
    let original = SaveRepo::get_gameline_list(&db, 101).await.unwrap();
    Migrator::down(&db, Some(1)).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    let migrated = sync(&indexer, &db, &dir).await;
    assert_eq!(migrated.chunks, first.chunks);
    assert_eq!(migrated.changed_documents, first.documents);
    assert_eq!(
        original,
        SaveRepo::get_gameline_list(&db, 101).await.unwrap()
    );
    encoder.token_calls.store(0, Ordering::SeqCst);
    sync(&indexer, &db, &dir).await;
    assert_eq!(encoder.token_calls.load(Ordering::SeqCst), 0);
    db.close().await.unwrap();
}

#[tokio::test]
async fn role_deletion_cleans_derived_rows_and_its_saves_across_roles() {
    use ling_chat_main::db::managers::role_repo::RoleRepo;

    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    let indexer = MemoryIndexer::new(encoder.clone());
    let store = NotesStore::new(dir.path());
    sync(&indexer, &db, &dir).await;
    for (save_id, role_id, name) in [
        (101, 3, "测试角色乙"),
        (102, 2, "测试角色甲"),
        (102, 3, "测试角色乙"),
    ] {
        indexer
            .sync(&db, &store, save_id, role_id, name, IndexMode::Incremental)
            .await
            .unwrap();
    }
    SaveRepo::update_save_main_role(&db, 102, Some(3))
        .await
        .unwrap();
    assert!(RoleRepo::delete_main_role(&db, 3).await.unwrap());
    for sql in [
        "SELECT COUNT(*) FROM memory_document WHERE role_id=3 OR save_id=102",
        "SELECT COUNT(*) FROM memory_index_state WHERE role_id=3 OR save_id=102",
        "SELECT COUNT(*) FROM memory_chunk c LEFT JOIN memory_document d ON c.document_id=d.id WHERE d.id IS NULL",
        "SELECT COUNT(*) FROM memory_embedding e LEFT JOIN memory_chunk c ON e.chunk_id=c.id WHERE c.id IS NULL",
    ] {
        let count = db
            .query_one(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .unwrap()
            .unwrap()
            .try_get_by_index::<i64>(0)
            .unwrap();
        assert_eq!(count, 0, "{sql}");
    }
    assert!(
        db.query_all(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_key_check"
        ))
        .await
        .unwrap()
        .is_empty()
    );
    let remaining = MemoryIndexRepo::load(&db, &scope(&db, &dir).await, &encoder.version())
        .await
        .unwrap();
    assert!(
        remaining
            .documents
            .iter()
            .any(|item| item.document.kind == MemoryKind::Raw)
    );
    assert_eq!(
        remaining
            .documents
            .iter()
            .filter(|item| item.document.kind == MemoryKind::Note)
            .count(),
        2
    );
    assert_eq!(store.load("测试角色甲").unwrap().len(), 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn save_deletion_explicitly_cleans_index_with_foreign_keys_disabled() {
    let (dir, db, _) = seed().await;
    let encoder = Encoder::new("model-a");
    sync(&MemoryIndexer::new(encoder.clone()), &db, &dir).await;
    execute(&db, "PRAGMA foreign_keys=OFF").await;
    SaveRepo::delete_save(&db, 101).await.unwrap();
    execute(&db, "PRAGMA foreign_keys=ON").await;
    assert!(
        db.query_all(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_key_check"
        ))
        .await
        .unwrap()
        .is_empty()
    );
    let count = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) FROM memory_index_state WHERE save_id=101",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap();
    assert_eq!(count, 0);
    let current =
        MemoryScopeSnapshot::load(&db, &NotesStore::new(dir.path()), 102, 2, "测试角色甲")
            .await
            .unwrap();
    let remaining = MemoryIndexRepo::load(&db, &current, &encoder.version())
        .await
        .unwrap();
    assert_eq!(remaining.documents.len(), 2);
    assert!(
        remaining
            .documents
            .iter()
            .all(|item| item.document.kind == MemoryKind::Note)
    );
    db.close().await.unwrap();
}
