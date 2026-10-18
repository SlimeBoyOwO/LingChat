use std::collections::HashMap;

use ling_chat_main::ai_service::game_system::memory_scope::MemoryScopeSnapshot;
use ling_chat_notes::memory::{
    EventStatus, MemoryChunk, MemoryDocument, MemoryKind, MemoryScope, MemorySource,
};

use super::*;

async fn snapshot(
    db: &DatabaseConnection,
    dir: &tempfile::TempDir,
    save_id: i32,
    role_id: i32,
) -> MemoryScopeSnapshot {
    let name = if role_id == 2 {
        "测试角色甲"
    } else {
        "测试角色乙"
    };
    MemoryScopeSnapshot::load(db, &NotesStore::new(dir.path()), save_id, role_id, name)
        .await
        .unwrap()
}

fn document(
    fixture: &Fixture,
    source: &Document,
    snapshot: &MemoryScopeSnapshot,
) -> MemoryDocument {
    if source.kind == "note" {
        let note = fixture
            .notes
            .iter()
            .find(|note| note["id"] == source.id)
            .unwrap();
        return MemoryDocument::from_note(
            source.role_id,
            &serde_json::from_value(note.clone()).unwrap(),
        )
        .unwrap();
    }
    let mut document = MemoryDocument {
        id: source.id.clone(),
        kind: if source.kind == "raw" {
            MemoryKind::Raw
        } else {
            MemoryKind::Diary
        },
        role_id: source.role_id,
        scope: MemoryScope::Save {
            save_id: source.save_id.unwrap(),
        },
        content: source.content.clone(),
        tags: source.tags.clone(),
        recorded_at: None,
        event_time: None,
        event_status: EventStatus::Unknown,
        version: "fixture-v1".into(),
        sources: source
            .source_line_ids
            .iter()
            .map(|id| {
                snapshot
                    .lines()
                    .iter()
                    .find_map(|line| match &line.source {
                        MemorySource::Line { line_id, .. } if line_id == id => {
                            Some(line.source.clone())
                        },
                        _ => None,
                    })
                    .unwrap_or_else(|| MemorySource::Line {
                        line_id: *id,
                        version: "unavailable".into(),
                    })
            })
            .collect(),
    };
    document.version = document.content_version().unwrap();
    document
}

fn gift(fixture: &Fixture, snapshot: &MemoryScopeSnapshot) -> MemoryDocument {
    document(
        fixture,
        fixture
            .documents
            .iter()
            .find(|doc| doc.id == "diary_gift")
            .unwrap(),
        snapshot,
    )
}

fn chunk(document: &MemoryDocument) -> MemoryChunk {
    let mut chunk = MemoryChunk {
        id: format!("{}:0", document.id),
        document_id: document.id.clone(),
        ordinal: 0,
        content: document.content.clone(),
        version: "chunk-v1".into(),
        sources: document.sources.clone(),
    };
    chunk.version = chunk.content_version().unwrap();
    chunk
}

async fn execute(db: &DatabaseConnection, sql: &str) {
    db.execute(Statement::from_string(DbBackend::Sqlite, sql))
        .await
        .unwrap();
}

#[tokio::test]
async fn labeled_candidates_are_scoped_before_any_similarity_ranking() {
    let (dir, db, fixture) = seed().await;
    for query in &fixture.queries {
        let scope = snapshot(&db, &dir, query.save_id, query.role_id).await;
        let docs: HashMap<_, _> = fixture
            .documents
            .iter()
            .map(|source| (source.id.as_str(), document(&fixture, source, &scope)))
            .collect();
        let eligible = scope.eligible_documents(docs.values());
        for id in &query.expected_any {
            let doc = &docs[id.as_str()];
            assert!(eligible.contains(&doc), "{}: 正确来源被范围过滤", query.id);
            assert!(scope.allows_chunk(doc, &chunk(doc)));
        }
        for id in &query.excluded {
            let doc = &docs[id.as_str()];
            assert!(!eligible.contains(&doc), "{}: 禁止来源进入候选池", query.id);
            assert!(!scope.allows_chunk(doc, &chunk(doc)));
        }
        assert!(
            scope
                .lines()
                .iter()
                .any(|line| line.content.contains("书房靠东"))
                == (query.save_id == 101 && query.role_id == 2)
        );
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn rewinding_one_diary_source_rejects_the_whole_document_and_chunk() {
    let (dir, db, fixture) = seed().await;
    let before = snapshot(&db, &dir, 101, 2).await;
    let diary = document(
        &fixture,
        fixture
            .documents
            .iter()
            .find(|doc| doc.id == "diary_trip")
            .unwrap(),
        &before,
    );
    let note = before.notes()[0].clone();
    assert!(before.allows_document(&diary));
    SaveRepo::update_save_last_message(&db, 101, Some(3))
        .await
        .unwrap();
    let after = snapshot(&db, &dir, 101, 2).await;
    assert!(!after.allows_document(&diary));
    let mut partial = chunk(&diary);
    partial.sources.truncate(1);
    partial.version = partial.content_version().unwrap();
    assert!(!after.allows_chunk(&diary, &partial));
    assert_ne!(before.key().history_version, after.key().history_version);
    assert_eq!(before.key().notes_version, after.key().notes_version);
    assert!(after.allows_document(&note));
    assert!(snapshot(&db, &dir, 102, 2).await.allows_document(&note));
    assert!(!snapshot(&db, &dir, 101, 3).await.allows_document(&note));
    db.close().await.unwrap();
}

#[tokio::test]
async fn edits_and_perception_changes_invalidate_sources_with_the_same_tip() {
    let (dir, db, fixture) = seed().await;
    let before = snapshot(&db, &dir, 101, 2).await;
    let diary = gift(&fixture, &before);
    execute(
        &db,
        "UPDATE line SET content='晓禾送我一枚铜色书签。' WHERE id=2",
    )
    .await;
    let edited = snapshot(&db, &dir, 101, 2).await;
    assert_ne!(before.key().history_version, edited.key().history_version);
    assert!(!edited.allows_document(&diary));
    let updated_diary = gift(&fixture, &edited);
    assert!(edited.allows_document(&updated_diary));
    execute(
        &db,
        "DELETE FROM line_perception WHERE line_id=2 AND role_id=2",
    )
    .await;
    let hidden = snapshot(&db, &dir, 101, 2).await;
    assert_ne!(edited.key().history_version, hidden.key().history_version);
    assert!(!hidden.allows_document(&updated_diary));
    assert!(
        hidden
            .lines()
            .iter()
            .any(|line| matches!(line.source, MemorySource::Line { line_id: 7, .. }))
    );
    assert_eq!(hidden.key().notes_version, before.key().notes_version);
    db.close().await.unwrap();
}

#[tokio::test]
async fn media_and_private_metadata_backfill_preserve_fact_versions() {
    let (dir, db, fixture) = seed().await;
    let before = snapshot(&db, &dir, 101, 2).await;
    let diary = gift(&fixture, &before);
    for sql in [
        "UPDATE line SET audio_file='backfill.wav' WHERE id=2",
        "UPDATE line SET tts_content='书签的语音文本' WHERE id=2",
        "UPDATE line SET predicted_emotion='happy' WHERE id=2",
        "UPDATE line SET original_emotion='calm' WHERE id=2",
        "UPDATE line SET thinking='内部推理记录' WHERE id=2",
    ] {
        execute(&db, sql).await;
        let current = snapshot(&db, &dir, 101, 2).await;
        assert_eq!(before.key(), current.key());
        assert!(current.allows_document(&diary));
    }
    execute(&db, "UPDATE line SET action_content='收起书签' WHERE id=2").await;
    let changed = snapshot(&db, &dir, 101, 2).await;
    assert_ne!(before.key(), changed.key());
    assert!(!changed.allows_document(&diary));
    db.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_brackets_and_json_arrays_are_not_tool_protocol() {
    let (dir, db, _) = seed().await;
    for content in [
        "[微笑] 我把盆栽放在书房靠东的窗台上了。",
        "[1, 2, 3]",
        r#"[{"id":1,"name":"盆栽"}]"#,
        r#"[{"function":"sin"}]"#,
        r#"["苹果","橘子"]"#,
    ] {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE line SET content=? WHERE id=7",
            vec![content.into()],
        ))
        .await
        .unwrap();
        let current = snapshot(&db, &dir, 101, 2).await;
        assert!(current.lines().iter().any(|line| {
            matches!(line.source, MemorySource::Line { line_id: 7, .. }) && line.content == content
        }));
    }
    execute(&db, "UPDATE line SET tool_call='invalid-json' WHERE id=7").await;
    assert!(
        !snapshot(&db, &dir, 101, 2)
            .await
            .lines()
            .iter()
            .any(|line| { matches!(line.source, MemorySource::Line { line_id: 7, .. }) })
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn action_only_records_follow_the_same_role_visibility_rules() {
    let (dir, db, _) = seed().await;
    execute(
        &db,
        "UPDATE line SET content='',action_content='把钥匙放进蓝盒子' WHERE id IN (1,6,7)",
    )
    .await;
    execute(
        &db,
        "UPDATE line SET content=' ',thinking='只有隐藏思考' WHERE id=2",
    )
    .await;
    let current = snapshot(&db, &dir, 101, 2).await;
    let ids: Vec<_> = current
        .lines()
        .iter()
        .filter_map(|line| match line.source {
            MemorySource::Line { line_id, .. } => Some(line_id),
            _ => None,
        })
        .collect();
    assert!(ids.contains(&1) && ids.contains(&7));
    assert!(!ids.contains(&2) && !ids.contains(&6));
    assert!(
        snapshot(&db, &dir, 101, 3)
            .await
            .lines()
            .iter()
            .any(|line| { matches!(line.source, MemorySource::Line { line_id: 6, .. }) })
    );
    execute(&db, "UPDATE line SET attribute='system' WHERE id=1").await;
    assert!(
        !snapshot(&db, &dir, 101, 2)
            .await
            .lines()
            .iter()
            .any(|line| { matches!(line.source, MemorySource::Line { line_id: 1, .. }) })
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn external_note_edits_deletion_and_json_layout_are_detected() {
    let (dir, db, _) = seed().await;
    let before = snapshot(&db, &dir, 101, 2).await;
    let old = before
        .notes()
        .iter()
        .find(|note| note.id.ends_with("note_legacy"))
        .unwrap();
    let path = dir.path().join("game_data/notes/测试角色甲.json");
    let mut notes: Vec<Note> = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    notes.reverse();
    std::fs::write(&path, serde_json::to_vec(&notes).unwrap()).unwrap();
    let reordered = snapshot(&db, &dir, 101, 2).await;
    assert_eq!(before.key(), reordered.key());
    notes
        .iter_mut()
        .find(|note| note.id == "note_legacy")
        .unwrap()
        .content = "猫叫圆圆".into();
    std::fs::write(&path, serde_json::to_vec(&notes).unwrap()).unwrap();
    let edited = snapshot(&db, &dir, 101, 2).await;
    assert_ne!(before.key().notes_version, edited.key().notes_version);
    assert_eq!(before.key().history_version, edited.key().history_version);
    assert!(!edited.allows_document(old));
    std::fs::remove_file(&path).unwrap();
    let deleted = snapshot(&db, &dir, 101, 2).await;
    assert!(deleted.notes().is_empty());
    assert!(!deleted.allows_document(&edited.notes()[0]));
    std::fs::write(&path, b"invalid json").unwrap();
    assert!(
        MemoryScopeSnapshot::load(&db, &NotesStore::new(dir.path()), 101, 2, "测试角色甲")
            .await
            .is_err()
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn broken_cross_save_and_cyclic_chains_fail_closed() {
    let (dir, db, _) = seed().await;
    let store = NotesStore::new(dir.path());
    assert!(
        MemoryScopeSnapshot::load(&db, &store, 999, 2, "测试角色甲")
            .await
            .is_err()
    );
    assert!(
        MemoryScopeSnapshot::load(&db, &store, 101, 999, "测试角色甲")
            .await
            .is_err()
    );
    execute(&db, "UPDATE line SET parent_line_id=201 WHERE id=1").await;
    assert!(
        MemoryScopeSnapshot::load(&db, &store, 101, 2, "测试角色甲")
            .await
            .is_err()
    );
    execute(&db, "UPDATE line SET parent_line_id=2 WHERE id=1").await;
    assert!(
        MemoryScopeSnapshot::load(&db, &store, 101, 2, "测试角色甲")
            .await
            .is_err()
    );
    assert!(SaveRepo::get_line_list(&db, 101).await.is_err());
    execute(&db, "UPDATE line SET parent_line_id=NULL WHERE id=1").await;
    SaveRepo::update_save_last_message(&db, 101, None)
        .await
        .unwrap();
    let empty = snapshot(&db, &dir, 101, 2).await;
    assert!(empty.lines().is_empty());
    assert!(!empty.notes().is_empty());
    db.close().await.unwrap();
}

#[tokio::test]
async fn content_based_scope_keys_survive_restart_and_reject_duplicates() {
    let (dir, db, _) = seed().await;
    let before = snapshot(&db, &dir, 101, 2).await;
    db.close().await.unwrap();
    let reopened = db::init_db(dir.path()).await.unwrap();
    let after = snapshot(&reopened, &dir, 101, 2).await;
    assert_eq!(before.key(), after.key());
    let store = NotesStore::new(dir.path());
    let mut notes = store.load("测试角色甲").unwrap();
    notes.push(notes[0].clone());
    store.save("测试角色甲", &notes).unwrap();
    assert!(
        MemoryScopeSnapshot::load(&reopened, &store, 101, 2, "测试角色甲")
            .await
            .is_err()
    );
    reopened.close().await.unwrap();
}
