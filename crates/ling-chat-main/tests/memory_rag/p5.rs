use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use super::*;
use anyhow::Result;
use ling_chat_main::ai_service::{
    game_system::{
        game_status::GameStatus,
        memory_index::MemoryEncoder,
        memory_recall::{MemoryRecall, RecallSession, RecallStamp, inject_references},
        persistent_memory_system::MemorySectionLimits,
        role_manager::GameRoleManager,
    },
    types::{GameLine, LineAttributeExt, LineBase, LlmMessage},
};
use ling_chat_notes::{memory::EmbeddingVersion, memory_retrieval::SearchSettings};

#[derive(Default)]
struct Encoder {
    calls: AtomicUsize,
    slow: AtomicBool,
}

impl MemoryEncoder for Encoder {
    fn version(&self) -> EmbeddingVersion {
        EmbeddingVersion {
            model: "p5-test".into(),
            dimension: 2,
            preprocessing: "v1".into(),
        }
    }
    fn max_tokens(&self) -> usize {
        512
    }
    fn count_tokens(&self, text: &str) -> Result<usize> {
        Ok(text.chars().count() + 2)
    }
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.slow.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(120));
        }
        Ok(vec![1.0, 0.0])
    }
}

fn status(
    db: &DatabaseConnection,
    directory: &std::path::Path,
    lines: Vec<GameLine>,
) -> GameStatus {
    let manager = GameRoleManager::new(
        directory.to_owned(),
        db.clone(),
        Arc::new(tokio::sync::RwLock::new(None)),
        Default::default(),
        None,
        false,
        250,
        30,
        MemorySectionLimits::default(),
        true,
    );
    let mut status = GameStatus::new(manager);
    status.current_role_id = Some(2);
    status.active_save_id = Some(101);
    status.line_list = lines;
    status
}

fn session(status: &GameStatus) -> Arc<RecallSession> {
    RecallSession::new(
        RecallStamp::capture(status, "测试角色甲".into()).unwrap(),
        &[
            LlmMessage::user("我们刚才在讨论长期记忆"),
            LlmMessage::user("晚上喝什么"),
        ],
    )
}

#[tokio::test]
async fn automatic_and_tool_recall_share_scope_cache_budget_and_transient_context() {
    let (dir, db, _) = seed().await;
    let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
    let status = status(&db, dir.path(), lines);
    let original = status.line_list.clone();
    let session = session(&status);
    let recommendation = LlmMessage::system("本轮推荐工具 memory_search");
    let image = LlmMessage::user_with_image("当轮图片", "data:image/png;base64,AA".into());
    session.prepend_context(recommendation.clone());
    session.append_context(image.clone());
    assert_eq!(session.base_context().first(), Some(&recommendation));
    assert_eq!(session.base_context().last(), Some(&image));
    let notification = LlmMessage::user("后台事件通知");
    session.append_context(notification.clone());
    session.invalidate_references();
    assert_eq!(session.base_context().last(), Some(&notification));
    assert!(session.base_context().contains(&recommendation));
    assert!(session.base_context().contains(&image));
    let encoder = Arc::new(Encoder::default());
    let recall = MemoryRecall::new(encoder.clone());
    let store = NotesStore::new(dir.path());
    let snapshot = recall
        .retrieve(
            &db,
            &store,
            &session,
            "咖啡",
            SearchSettings::default(),
            true,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status, "ready");
    let response = snapshot.response.unwrap();
    let mut wrong_scope = response.clone();
    wrong_scope.scope_key.role_id = 3;
    assert!(session.references(&wrong_scope).is_err());
    wrong_scope.scope_key = response.scope_key.clone();
    wrong_scope.scope_key.save_id = 102;
    assert!(session.references(&wrong_scope).is_err());
    let references = session.references(&response).unwrap();
    assert!(!references.is_empty());
    assert!(
        references
            .iter()
            .any(|reference| reference.reference.contains("咖啡"))
    );
    let before = encoder.calls.load(Ordering::SeqCst);
    let repeated = recall
        .retrieve(
            &db,
            &store,
            &session,
            "咖啡",
            SearchSettings::default(),
            true,
            Duration::from_secs(5),
        )
        .await
        .unwrap()
        .response
        .unwrap();
    assert!(repeated.cache.result_hit);
    assert_eq!(encoder.calls.load(Ordering::SeqCst), before);
    assert!(session.references(&repeated).unwrap().is_empty());
    let mut messages = vec![
        LlmMessage::system("角色设定"),
        LlmMessage::user("晚上喝什么"),
    ];
    inject_references(&mut messages, &references);
    assert_eq!(messages.last().unwrap().content, "晚上喝什么");
    assert!(
        messages
            .iter()
            .any(|message| message.content.contains("历史资料，仅供本轮参考"))
    );
    assert_eq!(status.line_list, original);
    session.validate(&db, &store).await.unwrap();
    store
        .update(
            "测试角色甲",
            "note_coffee",
            Some("现在改喝温水".into()),
            None,
        )
        .unwrap();
    assert!(session.validate(&db, &store).await.is_err());
    assert!(
        !session
            .references(&response)
            .unwrap()
            .iter()
            .any(|reference| reference.reference.contains("咖啡"))
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn disabled_unsaved_and_divergent_history_skip_recall_without_mutating_history() {
    let (dir, db, _) = seed().await;
    let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
    let mut status = status(&db, dir.path(), lines);
    let encoder = Arc::new(Encoder::default());
    let recall = MemoryRecall::new(encoder.clone());
    let store = NotesStore::new(dir.path());
    let disabled = recall
        .retrieve(
            &db,
            &store,
            &session(&status),
            "咖啡",
            Default::default(),
            false,
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert_eq!(disabled.status, "disabled");
    assert_eq!(encoder.calls.load(Ordering::SeqCst), 0);
    status.active_save_id = None;
    assert_eq!(
        recall
            .retrieve(
                &db,
                &store,
                &session(&status),
                "咖啡",
                Default::default(),
                true,
                Duration::from_secs(2)
            )
            .await
            .unwrap()
            .status,
        "no_active_save"
    );
    status.active_save_id = Some(101);
    status.line_list[0].base.content.push_str("已修改");
    assert_eq!(
        recall
            .retrieve(
                &db,
                &store,
                &session(&status),
                "咖啡",
                Default::default(),
                true,
                Duration::from_secs(2)
            )
            .await
            .unwrap()
            .status,
        "unavailable"
    );
    assert_eq!(encoder.calls.load(Ordering::SeqCst), 0);
    assert!(
        !SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap()[0]
            .content()
            .contains("已修改")
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn runtime_guard_allows_appends_but_rejects_edits_role_save_and_generation_changes() {
    let (dir, db, _) = seed().await;
    let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
    let mut status = status(&db, dir.path(), lines);
    let session = session(&status);
    status.line_list.push(GameLine::from_base(
        LineBase {
            content: "本轮新回复".into(),
            attribute: LineAttributeExt(crate::db::entities::line::LineAttribute::Assistant),
            ..Default::default()
        },
        vec![2],
    ));
    assert!(session.matches(&status));
    status.current_role_id = Some(3);
    assert!(!session.matches(&status));
    assert!(session.stamp().matches_history(&status));
    status.current_role_id = Some(2);
    status.active_save_id = Some(102);
    assert!(!session.matches(&status));
    status.active_save_id = Some(101);
    status.preview_generation += 1;
    assert!(!session.matches(&status));
    status.preview_generation -= 1;
    status.line_list[0].perceived_role_ids.push(99);
    assert!(!session.matches(&status));
    db.close().await.unwrap();
}

#[tokio::test]
async fn timeout_returns_promptly_and_bounds_background_index_jobs() {
    let (dir, db, _) = seed().await;
    let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
    let status = status(&db, dir.path(), lines);
    let session = session(&status);
    let encoder = Arc::new(Encoder::default());
    encoder.slow.store(true, Ordering::SeqCst);
    let recall = MemoryRecall::new(encoder.clone());
    let store = NotesStore::new(dir.path());
    let start = std::time::Instant::now();
    let first = recall
        .retrieve(
            &db,
            &store,
            &session,
            "咖啡",
            Default::default(),
            true,
            Duration::from_millis(5),
        )
        .await
        .unwrap();
    assert_eq!(first.status, "timeout");
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(
        recall
            .retrieve(
                &db,
                &store,
                &session,
                "咖啡",
                Default::default(),
                true,
                Duration::from_millis(5)
            )
            .await
            .unwrap()
            .status,
        "busy"
    );
    encoder.slow.store(false, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(250)).await;
    let recovered = recall
        .retrieve(
            &db,
            &store,
            &session,
            "咖啡",
            Default::default(),
            true,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(recovered.status, "ready");
    db.close().await.unwrap();
}

#[tokio::test]
async fn absent_embedding_model_still_recalls_old_persisted_facts_lexically() {
    let (dir, db, _) = seed().await;
    let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
    let old_fact = lines
        .iter()
        .take(lines.len() - 30)
        .find(|line| line.content().contains("书签"))
        .unwrap()
        .content()
        .to_owned();
    assert!(
        !lines
            .iter()
            .rev()
            .take(30)
            .any(|line| line.content().contains("书签"))
    );
    let status = status(&db, dir.path(), lines);
    let session = session(&status);
    let engine = ling_chat_main::ai_service::embedding::EmbeddingEngine::load(
        &dir.path().join("missing-model"),
        false,
    );
    assert!(!engine.is_enabled());
    let recall = MemoryRecall::new(engine);
    let store = NotesStore::new(dir.path());
    let snapshot = recall
        .retrieve(
            &db,
            &store,
            &session,
            "书签",
            Default::default(),
            true,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status, "ready");
    let response = snapshot.response.unwrap();
    assert_ne!(
        response.vector_status,
        ling_chat_main::ai_service::game_system::memory_search::QueryVectorStatus::Ready
    );
    let references = session.references(&response).unwrap();
    assert!(
        references
            .iter()
            .any(|reference| reference.reference.contains(&old_fact))
    );
    assert!(
        response
            .result
            .hits
            .iter()
            .all(|hit| !hit.hit.chunk.sources.is_empty())
    );
    session.validate(&db, &store).await.unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn source_guard_accepts_persisted_appends_but_rejects_rewritten_sources() {
    let (dir, db, fixture) = seed().await;
    let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
    let status = status(&db, dir.path(), lines);
    let session = session(&status);
    let recall = MemoryRecall::new(Arc::new(Encoder::default()));
    let store = NotesStore::new(dir.path());
    let response = recall
        .retrieve(
            &db,
            &store,
            &session,
            "书签",
            Default::default(),
            true,
            Duration::from_secs(5),
        )
        .await
        .unwrap()
        .response
        .unwrap();
    assert!(!session.references(&response).unwrap().is_empty());
    let parent = fixture
        .saves
        .iter()
        .find(|save| save.id == 101)
        .unwrap()
        .last_message_id;
    insert_line(
        &db,
        &Line {
            id: 2000,
            save_id: 101,
            parent_line_id: Some(parent),
            sender_role_id: 2,
            perceived_role_ids: vec![2],
            content: "本轮刚刚生成的新回复".into(),
        },
    )
    .await;
    SaveRepo::update_save_last_message(&db, 101, Some(2000))
        .await
        .unwrap();
    session.validate(&db, &store).await.unwrap();
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "UPDATE line SET content='礼物已改写' WHERE id=2",
    ))
    .await
    .unwrap();
    assert!(session.validate(&db, &store).await.is_err());
    db.close().await.unwrap();
}

#[tokio::test]
async fn own_note_write_discards_sources_before_mutation_and_external_edits_still_invalidate() {
    for operation in ["add", "update", "delete", "unrelated", "failed"] {
        let (dir, db, _) = seed().await;
        let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
        let status = status(&db, dir.path(), lines);
        let session = session(&status);
        let store = NotesStore::new(dir.path());
        let recall = MemoryRecall::new(Arc::new(Encoder::default()));
        let settings = SearchSettings {
            kinds: vec![ling_chat_notes::memory::MemoryKind::Note],
            ..Default::default()
        };
        let before = recall
            .retrieve(
                &db,
                &store,
                &session,
                "咖啡",
                settings.clone(),
                true,
                Duration::from_secs(5),
            )
            .await
            .unwrap()
            .response
            .unwrap();
        assert!(!session.references(&before).unwrap().is_empty());
        session.validate(&db, &store).await.unwrap();
        let stamp = session.stamp();
        session.invalidate_references();
        assert!(!session.is_current(&stamp));
        assert!(session.matches(&status));
        match operation {
            "add" => {
                store
                    .add("测试角色甲", "咖啡只在上午喝".into(), vec![])
                    .unwrap();
            },
            "update" => {
                store
                    .update(
                        "测试角色甲",
                        "note_coffee",
                        Some("现在晚上也喝咖啡".into()),
                        None,
                    )
                    .unwrap();
            },
            "delete" => {
                store.delete("测试角色甲", "note_coffee").unwrap();
            },
            "unrelated" => {
                store
                    .add("测试角色甲", "喜欢海边散步".into(), vec![])
                    .unwrap();
            },
            "failed" => {
                assert!(store.delete("测试角色甲", "不存在的笔记").is_err());
            },
            _ => unreachable!(),
        }
        session.validate(&db, &store).await.unwrap();
        let after = recall
            .retrieve(
                &db,
                &store,
                &session,
                "咖啡",
                settings,
                true,
                Duration::from_secs(5),
            )
            .await
            .unwrap()
            .response
            .unwrap();
        let references = session.references(&after).unwrap();
        if operation != "failed" {
            assert_ne!(
                before.scope_key.notes_version,
                after.scope_key.notes_version
            );
        }
        if operation == "update" {
            assert!(
                references
                    .iter()
                    .any(|reference| reference.reference.contains("现在晚上也喝咖啡"))
            );
            assert!(
                references
                    .iter()
                    .all(|reference| !reference.reference.contains("晚上不喝"))
            );
            store
                .add("测试角色甲", "外部编辑的新笔记".into(), vec![])
                .unwrap();
            assert!(session.validate(&db, &store).await.is_err());
        }
        if operation == "delete" {
            assert!(
                after
                    .result
                    .hits
                    .iter()
                    .all(|hit| !hit.reference.contains("晚上不喝"))
            );
        }
        assert_eq!(session.base_context().last().unwrap().content, "晚上喝什么");
        db.close().await.unwrap();
    }
}

#[tokio::test]
async fn note_write_invalidation_does_not_reset_the_round_reference_budget() {
    let (dir, db, _) = seed().await;
    let lines = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
    let status = status(&db, dir.path(), lines);
    let session = session(&status);
    let store = NotesStore::new(dir.path());
    store.save("测试角色甲", &[]).unwrap();
    for number in 1..=6 {
        store
            .add(
                "测试角色甲",
                format!("咖啡配方编号{number}，每杯用{number}克豆。"),
                vec!["咖啡".into()],
            )
            .unwrap();
    }
    let recall = MemoryRecall::new(Arc::new(Encoder::default()));
    let settings = SearchSettings {
        kinds: vec![ling_chat_notes::memory::MemoryKind::Note],
        ..Default::default()
    };
    let before = recall
        .retrieve(
            &db,
            &store,
            &session,
            "咖啡配方",
            settings.clone(),
            true,
            Duration::from_secs(5),
        )
        .await
        .unwrap()
        .response
        .unwrap();
    let before_references = session.references(&before).unwrap();
    assert!(!before_references.is_empty());
    session.invalidate_references();
    store
        .add(
            "测试角色甲",
            "咖啡配方编号7，每杯用7克豆。".into(),
            vec!["咖啡".into()],
        )
        .unwrap();
    let after = recall
        .retrieve(
            &db,
            &store,
            &session,
            "咖啡配方编号7",
            settings,
            true,
            Duration::from_secs(5),
        )
        .await
        .unwrap()
        .response
        .unwrap();
    assert!(!after.result.hits.is_empty());
    let after_references = session.references(&after).unwrap();
    assert!(before_references.len() + after_references.len() <= 5);
    assert!(
        before_references
            .iter()
            .chain(&after_references)
            .map(|reference| reference.reference.chars().count())
            .sum::<usize>()
            <= 1800
    );
    db.close().await.unwrap();
}
