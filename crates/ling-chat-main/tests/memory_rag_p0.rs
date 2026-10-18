use std::collections::{HashMap, HashSet};

use ling_chat_main::ai_service::game_system::memory_builder::MemoryBuilder;
use ling_chat_main::db::{self, managers::save_repo::SaveRepo};
use ling_chat_notes::{Note, NotesStore};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Deserialize;

#[path = "memory_rag/p1.rs"]
mod p1;

#[path = "memory_rag/p2.rs"]
mod p2;

#[path = "memory_rag/p3.rs"]
mod p3;

#[path = "memory_rag/p4.rs"]
mod p4;

#[path = "memory_rag/p5.rs"]
mod p5;

#[derive(Deserialize)]
struct Fixture {
    schema_version: u32,
    roles: Vec<Role>,
    saves: Vec<Save>,
    lines: Vec<Line>,
    padding: Padding,
    notes: Vec<serde_json::Value>,
    documents: Vec<Document>,
    queries: Vec<Query>,
}

#[derive(Deserialize)]
struct Role {
    id: i32,
    name: String,
    role_type: String,
}

#[derive(Deserialize)]
struct Save {
    id: i32,
    title: String,
    last_message_id: i32,
}

#[derive(Deserialize)]
struct Line {
    id: i32,
    save_id: i32,
    parent_line_id: Option<i32>,
    sender_role_id: i32,
    perceived_role_ids: Vec<i32>,
    content: String,
}

#[derive(Deserialize)]
struct Padding {
    save_id: i32,
    first_id: i32,
    count: i32,
    parent_line_id: i32,
    role_id: i32,
}

#[derive(Deserialize)]
struct Document {
    id: String,
    kind: String,
    role_id: i32,
    save_id: Option<i32>,
    source_line_ids: Vec<i32>,
    content: String,
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct Query {
    id: String,
    split: String,
    category: String,
    save_id: i32,
    role_id: i32,
    query: String,
    recent_context: String,
    expected_any: Vec<String>,
    excluded: Vec<String>,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/memory_rag/acceptance.json")).unwrap()
}

async fn insert_line(db: &DatabaseConnection, line: &Line) {
    let attribute = if line.sender_role_id == 1 {
        "user"
    } else {
        "assistant"
    };
    db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO line (id,save_id,parent_line_id,sender_role_id,attribute,content) VALUES (?,?,?,?,?,?)",
        vec![line.id.into(), line.save_id.into(), line.parent_line_id.into(),
             line.sender_role_id.into(), attribute.into(), line.content.clone().into()]
    )).await.unwrap();
    for role_id in &line.perceived_role_ids {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO line_perception (line_id,role_id) VALUES (?,?)",
            vec![line.id.into(), (*role_id).into()],
        ))
        .await
        .unwrap();
    }
}

async fn seed() -> (tempfile::TempDir, DatabaseConnection, Fixture) {
    let fixture = fixture();
    let dir = tempfile::tempdir().unwrap();
    let db = db::init_db(dir.path()).await.unwrap();
    for role in &fixture.roles {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO role (id,name,role_type) VALUES (?,?,?)",
            vec![
                role.id.into(),
                role.name.clone().into(),
                role.role_type.clone().into(),
            ],
        ))
        .await
        .unwrap();
    }
    for save in &fixture.saves {
        db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO save (id,title,create_date,update_date,main_role_id) VALUES (?,?,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,2)",
            vec![save.id.into(), save.title.clone().into()]
        )).await.unwrap();
    }
    for line in &fixture.lines {
        insert_line(&db, line).await;
    }
    let padding = &fixture.padding;
    let mut parent = padding.parent_line_id;
    for offset in 0..padding.count {
        let id = padding.first_id + offset;
        let is_user = offset % 2 == 0;
        insert_line(
            &db,
            &Line {
                id,
                save_id: padding.save_id,
                parent_line_id: Some(parent),
                sender_role_id: if is_user { 1 } else { padding.role_id },
                perceived_role_ids: vec![padding.role_id],
                content: if is_user {
                    format!("第{offset}轮普通闲聊：今天整理了桌面。")
                } else {
                    "好，我们继续聊聊今天的日常。".into()
                },
            },
        )
        .await;
        parent = id;
    }
    for save in &fixture.saves {
        SaveRepo::update_save_last_message(&db, save.id, Some(save.last_message_id))
            .await
            .unwrap();
    }
    let notes_dir = dir.path().join("game_data/notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    for role in &fixture.roles {
        let notes: Vec<_> = fixture
            .notes
            .iter()
            .filter(|note| note["role_id"].as_i64() == Some(i64::from(role.id)))
            .collect();
        std::fs::write(
            notes_dir.join(format!("{}.json", role.name)),
            serde_json::to_vec_pretty(&notes).unwrap(),
        )
        .unwrap();
    }
    (dir, db, fixture)
}

#[tokio::test]
async fn isolated_database_preserves_history_and_perception_baseline() {
    let (dir, db, _) = seed().await;
    assert!(dir.path().join("game_database.db").exists());
    let check = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA quick_check",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(check.try_get_by_index::<String>(0).unwrap(), "ok");
    let lines = SaveRepo::get_gameline_list(&db, 101).await.unwrap();
    assert_eq!(lines.len(), 44);
    assert!(!lines.iter().any(|line| line.base.id == Some(900)));
    assert!(
        !lines[lines.len() - 30..]
            .iter()
            .any(|line| line.content().contains("银色书签"))
    );
    let role_a = MemoryBuilder::new(2)
        .build(&lines)
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(role_a.contains("银色书签"));
    assert!(role_a.contains("书房靠东"));
    assert!(!role_a.contains("蓝色纸鹤"));
    let role_b = MemoryBuilder::new(3)
        .build(&lines)
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(role_b.contains("蓝色纸鹤"));
    assert!(!role_b.contains("银色书签"));
    assert_eq!(SaveRepo::get_line_list(&db, 102).await.unwrap().len(), 1);
    assert_eq!(SaveRepo::get_line_list(&db, 103).await.unwrap().len(), 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn acceptance_labels_have_valid_sources_and_scope() {
    let (_dir, db, fixture) = seed().await;
    assert_eq!(fixture.schema_version, 1);
    let docs: HashMap<_, _> = fixture
        .documents
        .iter()
        .map(|doc| (doc.id.as_str(), doc))
        .collect();
    assert_eq!(docs.len(), fixture.documents.len());
    let mut query_ids = HashSet::new();
    let mut histories = HashMap::new();
    for save in &fixture.saves {
        histories.insert(
            save.id,
            SaveRepo::get_gameline_list(&db, save.id).await.unwrap(),
        );
    }
    for doc in &fixture.documents {
        assert!(!doc.content.is_empty());
        assert!(!doc.tags.is_empty() || doc.id == "note_legacy");
        if doc.kind == "note" {
            assert!(doc.save_id.is_none() && doc.source_line_ids.is_empty());
            let note = fixture
                .notes
                .iter()
                .find(|note| note["id"].as_str() == Some(doc.id.as_str()))
                .unwrap();
            assert_eq!(note["content"].as_str(), Some(doc.content.as_str()));
        } else {
            assert!(doc.save_id.is_some() && !doc.source_line_ids.is_empty());
            for id in &doc.source_line_ids {
                let source = fixture.lines.iter().find(|line| line.id == *id).unwrap();
                assert_eq!(Some(source.save_id), doc.save_id);
                assert!(
                    source.sender_role_id == doc.role_id
                        || source.perceived_role_ids.contains(&doc.role_id)
                );
                if doc.kind == "raw" {
                    assert_eq!(doc.content, source.content);
                }
            }
        }
    }
    for query in &fixture.queries {
        assert!(query_ids.insert(&query.id));
        assert!(!query.query.is_empty() && !query.category.is_empty());
        assert!(query.split == "calibration" || query.split == "holdout");
        assert!(fixture.roles.iter().any(|role| role.id == query.role_id));
        let lines = &histories[&query.save_id];
        if query.category == "follow_up" {
            assert!(!query.recent_context.is_empty());
        }
        for id in query.expected_any.iter().chain(&query.excluded) {
            assert!(docs.contains_key(id.as_str()));
        }
        for id in &query.expected_any {
            assert!(!query.excluded.contains(id));
            let doc = docs[id.as_str()];
            assert_eq!(
                doc.role_id, query.role_id,
                "{}: 预期来源越过角色范围",
                query.id
            );
            if let Some(save_id) = doc.save_id {
                assert_eq!(save_id, query.save_id);
                assert!(doc.source_line_ids.iter().all(|id| {
                    lines.iter().any(|line| {
                        line.base.id == Some(*id)
                            && (line.sender_role_id() == Some(query.role_id)
                                || line.perceived_role_ids.contains(&query.role_id))
                    })
                }));
            }
        }
    }
    assert_eq!(fixture.queries.len(), 18);
    db.close().await.unwrap();
}

#[tokio::test]
async fn rewind_baseline_removes_sources_but_preserves_role_notes() {
    let (dir, db, _) = seed().await;
    SaveRepo::update_save_last_message(&db, 101, Some(2))
        .await
        .unwrap();
    let lines = SaveRepo::get_line_list(&db, 101).await.unwrap();
    assert_eq!(lines.iter().map(|line| line.id).collect::<Vec<_>>(), [1, 2]);
    let store = NotesStore::new(dir.path());
    let notes = store.load("测试角色甲").unwrap();
    let legacy = notes.iter().find(|note| note.id == "note_legacy").unwrap();
    assert!(legacy.tags.is_empty());
    store.save("测试角色甲", &notes).unwrap();
    let reloaded: Vec<Note> = store.load("测试角色甲").unwrap();
    assert_eq!(reloaded.len(), 2);
    assert!(
        store
            .load("测试角色乙")
            .unwrap()
            .iter()
            .all(|note| note.id == "note_private")
    );
    db.close().await.unwrap();
}
