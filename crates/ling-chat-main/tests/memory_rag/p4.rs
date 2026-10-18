use super::*;
use anyhow::{Result, bail};
use ling_chat_main::ai_service::game_system::memory_cache::{CacheLimits, MemoryCacheConfig};
use ling_chat_main::ai_service::game_system::memory_index::{
    IndexMode, MemoryEncoder, MemoryIndexer,
};
use ling_chat_main::ai_service::game_system::memory_search::{
    MemorySearchResponse, MemorySearcher, QueryVectorStatus,
};
use ling_chat_notes::memory::EmbeddingVersion;
use ling_chat_notes::memory_retrieval::SearchSettings;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Default)]
struct Encoder {
    calls: AtomicUsize,
    token_calls: AtomicUsize,
    model: AtomicUsize,
    preprocessing: AtomicUsize,
    fail: AtomicBool,
    slow_version: AtomicBool,
    slow_inference: AtomicBool,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl Encoder {
    fn wait(&self) {
        self.started.notify_one();
        tokio::runtime::Handle::current().block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(30), self.release.notified())
                .await
                .unwrap();
        });
    }
}

impl MemoryEncoder for Encoder {
    fn version(&self) -> EmbeddingVersion {
        if self.slow_version.load(Ordering::SeqCst) {
            self.wait();
        }
        EmbeddingVersion {
            model: format!("cache-test-{}", self.model.load(Ordering::SeqCst)),
            dimension: 2,
            preprocessing: format!("v{}", self.preprocessing.load(Ordering::SeqCst)),
        }
    }
    fn max_tokens(&self) -> usize {
        512
    }
    fn count_tokens(&self, text: &str) -> Result<usize> {
        self.token_calls.fetch_add(1, Ordering::SeqCst);
        Ok(text.chars().count() + 2)
    }
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.slow_inference.load(Ordering::SeqCst) {
            self.wait();
        }
        if self.fail.load(Ordering::SeqCst) {
            bail!("test inference failure");
        }
        Ok(vec![1.0, 0.0])
    }
}

async fn setup() -> (tempfile::TempDir, DatabaseConnection, Arc<Encoder>) {
    let (dir, db, _) = seed().await;
    let encoder = Arc::new(Encoder::default());
    synchronize(&db, &NotesStore::new(dir.path()), encoder.clone(), 101, 2).await;
    (dir, db, encoder)
}

async fn synchronize(
    db: &DatabaseConnection,
    store: &NotesStore,
    encoder: Arc<Encoder>,
    save: i32,
    role: i32,
) {
    MemoryIndexer::new(encoder)
        .sync(
            db,
            store,
            save,
            role,
            if role == 2 {
                "测试角色甲"
            } else {
                "测试角色乙"
            },
            IndexMode::Incremental,
        )
        .await
        .unwrap();
}

async fn search(
    searcher: &MemorySearcher,
    db: &DatabaseConnection,
    store: &NotesStore,
    query: &str,
) -> MemorySearchResponse {
    searcher
        .search(
            db,
            store,
            101,
            2,
            "测试角色甲",
            query,
            "",
            &SearchSettings::default(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn repeated_query_skips_inference_and_ranking_with_identical_references() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder.clone());
    let before = encoder.calls.load(Ordering::SeqCst);
    let tokens_before = encoder.token_calls.load(Ordering::SeqCst);
    let first = search(&searcher, &db, &store, "咖啡").await;
    let second = search(&searcher, &db, &store, "  咖啡  ").await;
    assert!(!first.cache.result_hit);
    assert!(second.cache.result_hit);
    assert_eq!(first.result, second.result);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 1);
    assert_eq!(
        encoder.token_calls.load(Ordering::SeqCst),
        tokens_before + 1
    );
    assert_eq!(searcher.cache_stats().hybrid_searches, 1);
    assert_eq!(searcher.cache_stats().results.hits, 1);
    searcher.clear_caches();
    assert_eq!(searcher.cache_stats().query_vectors.retained_bytes, 0);
    let third = search(&searcher, &db, &store, "咖啡").await;
    assert!(!third.cache.result_hit);
    assert_eq!(third.result, first.result);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn settings_and_context_invalidate_results_but_only_effective_queries_change_vectors() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder.clone());
    search(&searcher, &db, &store, "咖啡").await;
    let before = encoder.calls.load(Ordering::SeqCst);
    let settings = SearchSettings {
        max_hits: 1,
        ..Default::default()
    };
    let changed = searcher
        .search(&db, &store, 101, 2, "测试角色甲", "咖啡", "", &settings)
        .await
        .unwrap();
    assert!(!changed.cache.result_hit && changed.cache.query_vector_hit);
    let changed_context = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "咖啡",
            "最近聊过钢琴。",
            &SearchSettings::default(),
        )
        .await
        .unwrap();
    assert!(!changed_context.cache.result_hit && changed_context.cache.query_vector_hit);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before);
    let followup = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "那次呢？",
            "去公园看银杏。",
            &settings,
        )
        .await
        .unwrap();
    let another = searcher
        .search(
            &db,
            &store,
            101,
            2,
            "测试角色甲",
            "那次呢？",
            "计划去海边。",
            &settings,
        )
        .await
        .unwrap();
    assert!(
        !followup.cache.query_vector_hit
            && !another.cache.query_vector_hit
            && !another.cache.result_hit
    );
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn modified_notes_and_rewind_invalidate_warm_results_before_resync() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder.clone());
    let first = search(&searcher, &db, &store, "咖啡").await;
    assert!(
        search(&searcher, &db, &store, "咖啡")
            .await
            .cache
            .result_hit
    );
    store
        .update(
            "测试角色甲",
            "note_coffee",
            Some("用户晚上改喝温水。".into()),
            None,
        )
        .unwrap();
    let changed = search(&searcher, &db, &store, "咖啡").await;
    assert!(!changed.cache.result_hit && changed.cache.query_vector_hit);
    assert!(!changed.index_is_current);
    assert_ne!(changed.scope_key, first.scope_key);
    assert!(
        changed
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.content.contains("不喝咖啡"))
    );
    synchronize(&db, &store, encoder.clone(), 101, 2).await;
    let synced = search(&searcher, &db, &store, "咖啡").await;
    assert!(!synced.cache.result_hit && synced.cache.query_vector_hit);
    assert!(synced.index_is_current);
    assert!(
        search(&searcher, &db, &store, "咖啡")
            .await
            .cache
            .result_hit
    );
    store.delete("测试角色甲", "note_coffee").unwrap();
    let deleted = search(&searcher, &db, &store, "咖啡").await;
    assert!(!deleted.cache.result_hit);
    assert!(
        deleted
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.content.contains("温水"))
    );
    search(&searcher, &db, &store, "书签").await;
    SaveRepo::update_save_last_message(&db, 101, Some(1))
        .await
        .unwrap();
    let rewound = search(&searcher, &db, &store, "书签").await;
    assert!(!rewound.cache.result_hit && rewound.cache.query_vector_hit);
    assert!(
        rewound
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.content.contains("银色书签"))
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn role_and_save_changes_share_only_query_vectors() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    synchronize(&db, &store, encoder.clone(), 101, 3).await;
    synchronize(&db, &store, encoder.clone(), 102, 2).await;
    let searcher = MemorySearcher::new(encoder);
    let other_role = searcher
        .search(
            &db,
            &store,
            101,
            3,
            "测试角色乙",
            "保险箱",
            "",
            &SearchSettings::default(),
        )
        .await
        .unwrap();
    assert!(
        other_role
            .result
            .hits
            .iter()
            .any(|hit| hit.hit.chunk.content.contains("蓝色纸鹤"))
    );
    let current = search(&searcher, &db, &store, "保险箱").await;
    assert!(!current.cache.result_hit && current.cache.query_vector_hit);
    assert!(
        current
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.content.contains("蓝色纸鹤"))
    );
    search(&searcher, &db, &store, "晓禾").await;
    let other_save = searcher
        .search(
            &db,
            &store,
            102,
            2,
            "测试角色甲",
            "晓禾",
            "",
            &SearchSettings::default(),
        )
        .await
        .unwrap();
    assert!(!other_save.cache.result_hit && other_save.cache.query_vector_hit);
    assert!(
        other_save
            .result
            .hits
            .iter()
            .any(|hit| hit.hit.chunk.content.contains("红色风筝"))
    );
    assert!(
        other_save
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.content.contains("银色书签"))
    );
    db.execute_unprepared("DELETE FROM line_perception WHERE line_id=6 AND role_id=3")
        .await
        .unwrap();
    let revoked = searcher
        .search(
            &db,
            &store,
            101,
            3,
            "测试角色乙",
            "保险箱",
            "",
            &SearchSettings::default(),
        )
        .await
        .unwrap();
    assert!(!revoked.cache.result_hit);
    assert!(
        revoked
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.content.contains("蓝色纸鹤"))
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn model_and_preprocessing_changes_cannot_reuse_previous_vectors() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder.clone());
    search(&searcher, &db, &store, "咖啡").await;
    for preprocessing in [false, true] {
        if preprocessing {
            encoder.preprocessing.store(1, Ordering::SeqCst);
        } else {
            encoder.model.store(1, Ordering::SeqCst);
        }
        let missing = search(&searcher, &db, &store, "咖啡").await;
        assert!(!missing.cache.result_hit && !missing.cache.query_vector_hit);
        assert_eq!(missing.vector_status, QueryVectorStatus::NoIndexedVectors);
        synchronize(&db, &store, encoder.clone(), 101, 2).await;
        let before = encoder.calls.load(Ordering::SeqCst);
        let rebuilt = search(&searcher, &db, &store, "咖啡").await;
        assert!(!rebuilt.cache.result_hit && !rebuilt.cache.query_vector_hit);
        assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 1);
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn vector_replacement_and_missing_chunks_invalidate_results_without_scope_changes() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder);
    let first = search(&searcher, &db, &store, "咖啡").await;
    assert!(
        search(&searcher, &db, &store, "咖啡")
            .await
            .cache
            .result_hit
    );
    db.execute_unprepared(
        "UPDATE memory_embedding SET payload=json_set(payload,'$.values',json('[0.0,1.0]'))",
    )
    .await
    .unwrap();
    let changed = search(&searcher, &db, &store, "咖啡").await;
    assert_eq!(first.scope_key, changed.scope_key);
    assert_ne!(first.index_version, changed.index_version);
    assert!(!changed.cache.result_hit && changed.cache.query_vector_hit);
    assert!(
        changed
            .result
            .hits
            .iter()
            .all(|hit| hit.explanation.cosine == Some(0.0))
    );
    assert!(
        search(&searcher, &db, &store, "咖啡")
            .await
            .cache
            .result_hit
    );
    db.execute_unprepared("DELETE FROM memory_chunk WHERE document_id='note:2:note_coffee'")
        .await
        .unwrap();
    let deleted = search(&searcher, &db, &store, "咖啡").await;
    assert!(!deleted.cache.result_hit);
    assert!(
        deleted
            .result
            .hits
            .iter()
            .all(|hit| hit.hit.document_id != "note:2:note_coffee")
    );
    db.close().await.unwrap();
}

async fn cached_change_is_rejected(note: bool) {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = Arc::new(MemorySearcher::new(encoder.clone()));
    search(&searcher, &db, &store, "书签").await;
    encoder.slow_version.store(true, Ordering::SeqCst);
    let worker_searcher = searcher.clone();
    let worker_db = db.clone();
    let worker_store = store.clone();
    let worker = tokio::spawn(async move {
        worker_searcher
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
    if note {
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
    assert_eq!(searcher.cache_stats().results.hits, 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn cache_hit_still_rechecks_history_before_returning() {
    cached_change_is_rejected(false).await;
}

#[tokio::test]
async fn cache_hit_still_rechecks_external_notes_before_returning() {
    cached_change_is_rejected(true).await;
}

#[tokio::test]
async fn stale_search_does_not_populate_result_cache() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = Arc::new(MemorySearcher::new(encoder.clone()));
    encoder.slow_inference.store(true, Ordering::SeqCst);
    let worker_searcher = searcher.clone();
    let worker_db = db.clone();
    let worker_store = store.clone();
    let worker = tokio::spawn(async move {
        worker_searcher
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
    store.delete("测试角色甲", "note_legacy").unwrap();
    encoder.release.notify_one();
    assert!(worker.await.unwrap().is_err());
    assert_eq!(searcher.cache_stats().results.entries, 0);
    assert_eq!(searcher.cache_stats().query_vectors.entries, 1);
    let fresh = search(&searcher, &db, &store, "书签").await;
    assert!(fresh.cache.query_vector_hit && !fresh.cache.result_hit);
    db.close().await.unwrap();
}

#[tokio::test]
async fn lru_eviction_recomputes_without_deleting_permanent_memory() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let original_notes = store.load("测试角色甲").unwrap();
    let rows_before = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) FROM memory_embedding",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap();
    for vector_only in [false, true] {
        let mut config = MemoryCacheConfig::default();
        if vector_only {
            config.query_vectors.max_entries = 2;
            config.results.max_entries = 0;
        } else {
            config.results.max_entries = 2;
        }
        let searcher = MemorySearcher::with_cache_config(encoder.clone(), config);
        for query in ["奶茶", "咖啡", "奶茶", "猫"] {
            search(&searcher, &db, &store, query).await;
        }
        let before = encoder.calls.load(Ordering::SeqCst);
        let evicted = search(&searcher, &db, &store, "咖啡").await;
        assert!(!evicted.cache.result_hit);
        assert!(
            evicted
                .result
                .hits
                .iter()
                .any(|hit| hit.hit.chunk.content.contains("不喝咖啡"))
        );
        if vector_only {
            assert!(!evicted.cache.query_vector_hit);
            assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 1);
            assert!(searcher.cache_stats().query_vectors.evictions > 0);
        } else {
            assert!(evicted.cache.query_vector_hit);
            assert_eq!(encoder.calls.load(Ordering::SeqCst), before);
            assert!(searcher.cache_stats().results.evictions > 0);
        }
    }
    let rows_after = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) FROM memory_embedding",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap();
    assert_eq!(rows_before, rows_after);
    assert_eq!(
        serde_json::to_value(original_notes).unwrap(),
        serde_json::to_value(store.load("测试角色甲").unwrap()).unwrap()
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn tiny_byte_limits_and_disabled_caches_preserve_results() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let tiny = CacheLimits {
        max_entries: 100,
        max_bytes: 1,
        max_entry_bytes: 1,
    };
    for config in [
        MemoryCacheConfig::disabled(),
        MemoryCacheConfig {
            query_vectors: tiny,
            results: tiny,
        },
    ] {
        let searcher = MemorySearcher::with_cache_config(encoder.clone(), config);
        let before = encoder.calls.load(Ordering::SeqCst);
        let first = search(&searcher, &db, &store, "咖啡").await;
        let second = search(&searcher, &db, &store, "咖啡").await;
        assert_eq!(first.result, second.result);
        assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 2);
        assert_eq!(searcher.cache_stats().results.retained_bytes, 0);
        assert_eq!(searcher.cache_stats().query_vectors.retained_bytes, 0);
        assert_eq!(searcher.cache_stats().results.rejected, 2);
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn inference_failure_does_not_cache_fallback_or_prevent_recovery() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder.clone());
    encoder.fail.store(true, Ordering::SeqCst);
    let failed = search(&searcher, &db, &store, "咖啡").await;
    assert_eq!(failed.vector_status, QueryVectorStatus::InferenceFailed);
    assert!(!failed.result.hits.is_empty());
    assert_eq!(searcher.cache_stats().results.entries, 0);
    encoder.fail.store(false, Ordering::SeqCst);
    let before = encoder.calls.load(Ordering::SeqCst);
    let recovered = search(&searcher, &db, &store, "咖啡").await;
    assert_eq!(recovered.vector_status, QueryVectorStatus::Ready);
    assert!(!recovered.cache.result_hit && !recovered.cache.query_vector_hit);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 1);
    assert!(
        search(&searcher, &db, &store, "咖啡")
            .await
            .cache
            .result_hit
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn empty_cached_index_is_invalidated_when_vectors_arrive() {
    let (dir, db, _) = seed().await;
    let store = NotesStore::new(dir.path());
    let encoder = Arc::new(Encoder::default());
    let searcher = MemorySearcher::new(encoder.clone());
    assert!(
        search(&searcher, &db, &store, "奶茶")
            .await
            .result
            .hits
            .is_empty()
    );
    assert!(
        search(&searcher, &db, &store, "奶茶")
            .await
            .cache
            .result_hit
    );
    synchronize(&db, &store, encoder, 101, 2).await;
    let indexed = search(&searcher, &db, &store, "奶茶").await;
    assert!(!indexed.cache.result_hit);
    assert!(
        indexed
            .result
            .hits
            .iter()
            .any(|hit| hit.hit.chunk.content.contains("三分糖"))
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn source_edits_and_save_deletion_cannot_return_warm_results() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder);
    let old = search(&searcher, &db, &store, "书签").await;
    assert!(
        search(&searcher, &db, &store, "书签")
            .await
            .cache
            .result_hit
    );
    db.execute_unprepared("UPDATE line SET content='礼物已更正为围巾' WHERE id=2")
        .await
        .unwrap();
    let changed = search(&searcher, &db, &store, "书签").await;
    assert!(!changed.cache.result_hit);
    assert_ne!(old.scope_key, changed.scope_key);
    assert!(
        changed
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.content.contains("银色书签"))
    );
    SaveRepo::delete_save(&db, 101).await.unwrap();
    assert!(
        searcher
            .search(
                &db,
                &store,
                101,
                2,
                "测试角色甲",
                "书签",
                "",
                &SearchSettings::default()
            )
            .await
            .is_err()
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn oversized_and_empty_queries_do_not_fill_vector_cache() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder.clone());
    let before = encoder.calls.load(Ordering::SeqCst);
    for _ in 0..2 {
        let response = search(&searcher, &db, &store, &"咖啡".repeat(300)).await;
        assert_eq!(response.vector_status, QueryVectorStatus::TooManyTokens);
        assert!(!response.cache.result_hit && !response.cache.query_vector_hit);
    }
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
                &SearchSettings::default()
            )
            .await
            .is_err()
    );
    let empty = search(&searcher, &db, &store, "").await;
    assert_eq!(empty.vector_status, QueryVectorStatus::EmptyQuery);
    assert!(search(&searcher, &db, &store, "").await.cache.result_hit);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before);
    assert_eq!(searcher.cache_stats().query_vectors.entries, 0);
    assert_eq!(searcher.cache_stats().results.entries, 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn corrected_dates_and_distinct_facts_survive_persistent_index_and_result_cache() {
    let (dir, db, encoder) = setup().await;
    let store = NotesStore::new(dir.path());
    let searcher = MemorySearcher::new(encoder.clone());
    let make_note = |id: &str, content: &str| ling_chat_notes::Note {
        id: id.into(),
        content: content.into(),
        tags: vec![],
        created_at: "2026-10-18".into(),
    };
    let mut dated: Vec<_> = (0..25)
        .map(|index| make_note(&format!("a-wrong-{index}"), "2026年10月2日书签"))
        .collect();
    dated.push(make_note(
        "zz-target",
        "2026-10-01收到书签，2026年10月2日整理书架。",
    ));
    let silver = "那天下午我们沿着青榆公园的小路慢慢走到银杏树下躲雨，回家之前晓禾把一枚银色书签送给了我作为生日礼物。";
    let cases = [
        (dated, "2026年10月1日书签", 1),
        (
            vec![
                make_note("silver", silver),
                make_note("gold", &silver.replace("银色", "金色")),
            ],
            "书签",
            2,
        ),
        (
            vec![
                make_note("january", "我的纪念日是2026-1-01。"),
                make_note("october", "我的纪念日是2026-10-1。"),
            ],
            "纪念日",
            2,
        ),
    ];
    let settings = SearchSettings {
        kinds: vec![ling_chat_notes::memory::MemoryKind::Note],
        ..Default::default()
    };
    for (notes, query, expected_hits) in cases {
        store.save("测试角色甲", &notes).unwrap();
        synchronize(&db, &store, encoder.clone(), 101, 2).await;
        let before = encoder.calls.load(Ordering::SeqCst);
        let first = searcher
            .search(&db, &store, 101, 2, "测试角色甲", query, "", &settings)
            .await
            .unwrap();
        let second = searcher
            .search(&db, &store, 101, 2, "测试角色甲", query, "", &settings)
            .await
            .unwrap();
        assert_eq!(first.result.hits.len(), expected_hits, "{query}");
        assert!(!first.cache.result_hit && second.cache.result_hit);
        assert_eq!(first.result, second.result);
        assert_eq!(encoder.calls.load(Ordering::SeqCst), before + 1);
        if query == "2026年10月1日书签" {
            assert_eq!(first.result.hits[0].hit.document_id, "note:2:zz-target");
            assert_eq!(first.result.diagnostics.rejected_date, 25);
        } else {
            for note in &notes {
                assert!(
                    first
                        .result
                        .hits
                        .iter()
                        .any(|hit| hit.hit.chunk.content == note.content)
                );
            }
        }
    }
    db.close().await.unwrap();
}
