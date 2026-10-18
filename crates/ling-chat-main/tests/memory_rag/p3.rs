use super::*;
use anyhow::{Result, bail};
use ling_chat_main::ai_service::game_system::memory_index::{
    IndexMode, MemoryEncoder, MemoryIndexer,
};
use ling_chat_main::ai_service::game_system::memory_scope::MemoryScopeSnapshot;
use ling_chat_main::ai_service::game_system::memory_search::{MemorySearcher, QueryVectorStatus};
use ling_chat_main::db::managers::memory_index_repo::MemoryIndexRepo;
use ling_chat_notes::memory::{EmbeddingVersion, MemoryKind};
use ling_chat_notes::memory_retrieval::SearchSettings;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Default)]
struct Encoder {
    calls: AtomicUsize,
    fail: AtomicBool,
    invalid: AtomicBool,
    no_tokenizer: AtomicBool,
    slow: AtomicBool,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl MemoryEncoder for Encoder {
    fn version(&self) -> EmbeddingVersion {
        EmbeddingVersion {
            model: "p3-test".into(),
            dimension: 2,
            preprocessing: "v1".into(),
        }
    }
    fn max_tokens(&self) -> usize {
        512
    }
    fn count_tokens(&self, text: &str) -> Result<usize> {
        if self.no_tokenizer.load(Ordering::SeqCst) {
            bail!("tokenizer unavailable");
        }
        Ok(text.chars().count() + 2)
    }
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.slow.load(Ordering::SeqCst) {
            self.started.notify_one();
            tokio::runtime::Handle::current().block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(30), self.release.notified())
                    .await
            })?;
        }
        if self.fail.load(Ordering::SeqCst) {
            bail!("inference failed");
        }
        if self.invalid.load(Ordering::SeqCst) {
            return Ok(vec![f32::NAN, 0.0]);
        }
        Ok(vec![1.0, 0.0])
    }
}

fn lexical_settings() -> SearchSettings {
    SearchSettings {
        vector_threshold: 1.0,
        ..Default::default()
    }
}

async fn indexed() -> (tempfile::TempDir, DatabaseConnection, Arc<Encoder>) {
    let (dir, db, _) = seed().await;
    let encoder = Arc::new(Encoder::default());
    let indexer = MemoryIndexer::new(encoder.clone());
    for (save_id, role_id, name) in [
        (101, 2, "测试角色甲"),
        (101, 3, "测试角色乙"),
        (102, 2, "测试角色甲"),
        (103, 2, "测试角色甲"),
    ] {
        indexer
            .sync(
                &db,
                &NotesStore::new(dir.path()),
                save_id,
                role_id,
                name,
                IndexMode::Incremental,
            )
            .await
            .unwrap();
    }
    encoder.fail.store(true, Ordering::SeqCst);
    (dir, db, encoder)
}

#[tokio::test]
async fn lexical_fallback_preserves_scope_notes_and_plans() {
    let (dir, db, encoder) = indexed().await;
    let searcher = MemorySearcher::new(encoder);
    let store = NotesStore::new(dir.path());
    let fixture = fixture();
    for query in &fixture.queries {
        let name = if query.role_id == 2 {
            "测试角色甲"
        } else {
            "测试角色乙"
        };
        let response = searcher
            .search(
                &db,
                &store,
                query.save_id,
                query.role_id,
                name,
                &query.query,
                &query.recent_context,
                &lexical_settings(),
            )
            .await
            .unwrap();
        assert_eq!(response.vector_status, QueryVectorStatus::InferenceFailed);
        let scope = MemoryScopeSnapshot::load(&db, &store, query.save_id, query.role_id, name)
            .await
            .unwrap();
        let snapshot = MemoryIndexRepo::load(&db, &scope, &response.model)
            .await
            .unwrap();
        for result in &response.result.hits {
            let document = &snapshot
                .documents
                .iter()
                .find(|document| document.document.id == result.hit.document_id)
                .unwrap()
                .document;
            assert!(scope.allows_chunk(document, &result.hit.chunk));
            assert!(!result.hit.chunk.content.contains("金色戒指"));
            if query.role_id == 2 {
                assert!(!result.hit.chunk.content.contains("蓝色纸鹤"));
            }
        }
        if query.expected_any.is_empty() {
            assert!(response.result.hits.is_empty(), "{}", query.id);
        }
    }
    let response = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "海边",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert!(
        response
            .result
            .hits
            .iter()
            .any(|hit| hit.hit.chunk.content.contains("还没有出发"))
    );
    let response = searcher
        .search(
            &db,
            &store,
            102,
            2,
            "测试角色甲",
            "睡眠",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert_eq!(response.result.hits[0].hit.kind, MemoryKind::Note);
    assert_eq!(response.result.hits[0].explanation.accepted_by, ["tag"]);
    db.close().await.unwrap();
}

#[tokio::test]
async fn query_errors_and_missing_model_fall_back_without_truncation() {
    let (dir, db, encoder) = indexed().await;
    let searcher = MemorySearcher::new(encoder.clone());
    let store = NotesStore::new(dir.path());
    let before = encoder.calls.load(Ordering::SeqCst);
    let response = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            &"咖啡".repeat(300),
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert_eq!(response.vector_status, QueryVectorStatus::TooManyTokens);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before);
    assert_eq!(response.query.original.chars().count(), 600);
    let response = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert_eq!(response.vector_status, QueryVectorStatus::EmptyQuery);
    assert!(response.result.hits.is_empty());
    assert!(
        searcher
            .search(
                &db,
                &store,
                101,
                2,
                "测试角色甲",
                &"中".repeat(4097),
                "",
                &lexical_settings()
            )
            .await
            .is_err()
    );
    encoder.no_tokenizer.store(true, Ordering::SeqCst);
    let response = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "咖啡",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.vector_status,
        QueryVectorStatus::TokenizerUnavailable
    );
    assert!(!response.result.hits.is_empty());
    encoder.no_tokenizer.store(false, Ordering::SeqCst);
    encoder.fail.store(false, Ordering::SeqCst);
    encoder.invalid.store(true, Ordering::SeqCst);
    let response = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "咖啡",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert_eq!(response.vector_status, QueryVectorStatus::InvalidVector);
    assert!(!response.result.hits.is_empty());
    db.execute_unprepared("DELETE FROM memory_embedding")
        .await
        .unwrap();
    let response = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "咖啡",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert_eq!(response.vector_status, QueryVectorStatus::NoIndexedVectors);
    assert!(!response.result.hits.is_empty());
    let unavailable = ling_chat_main::ai_service::embedding::EmbeddingEngine::load(
        &dir.path().join("missing-model"),
        false,
    );
    let fallback = MemorySearcher::new(unavailable);
    let response = fallback
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "睡眠",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert_eq!(response.vector_status, QueryVectorStatus::NoIndexedVectors);
    assert_eq!(response.result.hits[0].hit.kind, MemoryKind::Note);
    db.close().await.unwrap();
}

#[tokio::test]
async fn changed_notes_rewind_and_deleted_sources_never_return_stale_text() {
    let (dir, db, encoder) = indexed().await;
    let searcher = MemorySearcher::new(encoder);
    let store = NotesStore::new(dir.path());
    let first = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "咖啡",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    store
        .update(
            "测试角色甲",
            "note_coffee",
            Some("晚上改喝温水。".into()),
            None,
        )
        .unwrap();
    let changed = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "咖啡",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert!(!changed.index_is_current);
    assert!(changed.result.hits.is_empty());
    assert_ne!(first.scope_key, changed.scope_key);
    assert_ne!(first.index_version, changed.index_version);
    SaveRepo::update_save_last_message(&db, 101, Some(1))
        .await
        .unwrap();
    let response = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "书签",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert!(response.result.hits.is_empty());
    let response = searcher
        .search(
            &db,
            &store,
            102,
            2,
            "测试角色甲",
            "红色风筝",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert!(!response.result.hits.is_empty());
    db.execute_unprepared("UPDATE line SET content='礼物已更正' WHERE id=201")
        .await
        .unwrap();
    let response = searcher
        .search(
            &db,
            &store,
            102,
            2,
            "测试角色甲",
            "红色风筝",
            "",
            &lexical_settings(),
        )
        .await
        .unwrap();
    assert!(response.result.hits.is_empty());
    assert!(response.scope_key.save_id == 102);
    db.close().await.unwrap();
}

async fn rejects_inflight_change(change_note: bool) {
    let (dir, db, encoder) = indexed().await;
    encoder.fail.store(false, Ordering::SeqCst);
    encoder.slow.store(true, Ordering::SeqCst);
    let searcher = MemorySearcher::new(encoder.clone());
    let store = NotesStore::new(dir.path());
    let worker_db = db.clone();
    let worker_store = store.clone();
    let worker = tokio::spawn(async move {
        searcher
            .search(
                &worker_db,
                &worker_store,
                101,
                2,
                "测试角色甲",
                "书签",
                "",
                &SearchSettings::default(),
            )
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        encoder.started.notified(),
    )
    .await
    .unwrap();
    if change_note {
        store.delete("测试角色甲", "note_legacy").unwrap();
    } else {
        SaveRepo::update_save_last_message(&db, 101, Some(1))
            .await
            .unwrap();
    }
    encoder.release.notify_one();
    assert!(
        worker
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("丢弃旧检索结果")
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn inflight_history_change_discards_results() {
    rejects_inflight_change(false).await;
}

#[tokio::test]
async fn inflight_external_note_change_discards_results() {
    rejects_inflight_change(true).await;
}

#[tokio::test]
async fn unbuilt_index_reports_empty_and_never_fabricates_memory() {
    let (dir, db, _) = seed().await;
    let encoder = Arc::new(Encoder::default());
    let searcher = MemorySearcher::new(encoder.clone());
    let response = searcher
        .search(
            &db,
            &NotesStore::new(dir.path()),
            101,
            2,
            "测试角色甲",
            "奶茶",
            "",
            &SearchSettings::default(),
        )
        .await
        .unwrap();
    assert!(response.result.hits.is_empty());
    assert!(!response.index_is_current);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), 0);
    assert_eq!(response.vector_status, QueryVectorStatus::NoIndexedVectors);
    assert_eq!(response.scope_key.role_id, 2);
    db.close().await.unwrap();
}
