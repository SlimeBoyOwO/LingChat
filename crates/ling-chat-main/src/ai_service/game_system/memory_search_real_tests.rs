use std::path::PathBuf;

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::Value;

use super::*;
use crate::ai_service::embedding::EmbeddingEngine;
use crate::ai_service::game_system::memory_index::{IndexMode, MemoryIndexer};
use crate::db::managers::save_repo::SaveRepo;
use ling_chat_notes::memory::MemorySource;

async fn seed() -> (tempfile::TempDir, DatabaseConnection, Value) {
    let mut fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/memory_rag/acceptance.json"
    )))
    .unwrap();
    let additional: Vec<Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/memory_rag/p3_holdout_v2.json"
    )))
    .unwrap();
    fixture["queries"]
        .as_array_mut()
        .unwrap()
        .extend(additional);
    let dir = tempfile::tempdir().unwrap();
    let db = crate::db::init_db(dir.path()).await.unwrap();
    for role in fixture["roles"].as_array().unwrap() {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO role(id,name,role_type) VALUES (?,?,?)",
            vec![
                (role["id"].as_i64().unwrap() as i32).into(),
                role["name"].as_str().unwrap().into(),
                role["role_type"].as_str().unwrap().into(),
            ],
        ))
        .await
        .unwrap();
    }
    for save in fixture["saves"].as_array().unwrap() {
        db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO save(id,title,create_date,update_date,main_role_id) VALUES (?,?,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,2)",
            vec![(save["id"].as_i64().unwrap() as i32).into(), save["title"].as_str().unwrap().into()],
        )).await.unwrap();
    }
    let mut lines = fixture["lines"].as_array().unwrap().clone();
    let padding = &fixture["padding"];
    let mut parent = padding["parent_line_id"].as_i64().unwrap();
    for offset in 0..padding["count"].as_i64().unwrap() {
        let id = padding["first_id"].as_i64().unwrap() + offset;
        lines.push(serde_json::json!({
            "id": id, "save_id": 101, "parent_line_id": parent,
            "sender_role_id": if offset % 2 == 0 { 1 } else { 2 },
            "perceived_role_ids": [2],
            "content": if offset % 2 == 0 { format!("第{offset}轮普通闲聊：今天整理了桌面。") } else { "好，我们继续聊聊今天的日常。".into() },
        }));
        parent = id;
    }
    for line in lines {
        let id = line["id"].as_i64().unwrap() as i32;
        let sender = line["sender_role_id"].as_i64().unwrap() as i32;
        db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO line(id,save_id,parent_line_id,sender_role_id,attribute,content) VALUES (?,?,?,?,?,?)",
            vec![id.into(), (line["save_id"].as_i64().unwrap() as i32).into(),
                line["parent_line_id"].as_i64().map(|value| value as i32).into(), sender.into(),
                if sender == 1 { "user" } else { "assistant" }.into(), line["content"].as_str().unwrap().into()],
        )).await.unwrap();
        for role in line["perceived_role_ids"].as_array().unwrap() {
            db.execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO line_perception(line_id,role_id) VALUES (?,?)",
                vec![id.into(), (role.as_i64().unwrap() as i32).into()],
            ))
            .await
            .unwrap();
        }
    }
    for save in fixture["saves"].as_array().unwrap() {
        SaveRepo::update_save_last_message(
            &db,
            save["id"].as_i64().unwrap() as i32,
            Some(save["last_message_id"].as_i64().unwrap() as i32),
        )
        .await
        .unwrap();
    }
    let store = NotesStore::new(dir.path());
    for role in fixture["roles"].as_array().unwrap() {
        let notes = fixture["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|note| note["role_id"] == role["id"])
            .map(|note| serde_json::from_value(note.clone()).unwrap())
            .collect::<Vec<_>>();
        store.save(role["name"].as_str().unwrap(), &notes).unwrap();
    }
    (dir, db, fixture)
}

fn matches_label(
    hit: &ling_chat_notes::memory_retrieval::ExplainedMemoryHit,
    label: &Value,
    fixture: &Value,
) -> bool {
    if label["kind"] == "note" {
        return hit.hit.chunk.sources.iter().any(|source| matches!(source, MemorySource::Note { note_id, .. } if Some(note_id.as_str()) == label["id"].as_str()))
            && hit.hit.chunk.content.contains(label["content"].as_str().unwrap());
    }
    label["source_line_ids"].as_array().unwrap().iter().any(|id| {
        hit.hit.chunk.sources.iter().any(|source| matches!(source, MemorySource::Line { line_id, .. } if i64::from(*line_id) == id.as_i64().unwrap()))
            && fixture["lines"].as_array().unwrap().iter().any(|line| line["id"] == *id
                && hit.hit.chunk.content.contains(line["content"].as_str().unwrap()))
    })
}

async fn evaluate(split: &str, encoder: Arc<dyn MemoryEncoder>) {
    let (dir, db, fixture) = seed().await;
    let store = NotesStore::new(dir.path());
    let indexer = MemoryIndexer::new(encoder.clone());
    let searcher = MemorySearcher::new(encoder);
    let mut scopes = std::collections::HashSet::new();
    let mut answered = 0;
    let mut found = 0;
    let mut empty = 0;
    let mut false_retrievals = 0;
    let mut cold_duration = std::time::Duration::ZERO;
    let mut warm_duration = std::time::Duration::ZERO;
    let mut warm_hits = 0;
    for query in fixture["queries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|query| query["split"] == split)
    {
        let save_id = query["save_id"].as_i64().unwrap() as i32;
        let role_id = query["role_id"].as_i64().unwrap() as i32;
        let name = if role_id == 2 {
            "测试角色甲"
        } else {
            "测试角色乙"
        };
        if scopes.insert((save_id, role_id)) {
            let report = indexer
                .sync(&db, &store, save_id, role_id, name, IndexMode::Incremental)
                .await
                .unwrap();
            assert!(report.embedded > 0 && report.lexical_only == 0);
        }
        let cold_started = std::time::Instant::now();
        let response = searcher
            .search(
                &db,
                &store,
                save_id,
                role_id,
                name,
                query["query"].as_str().unwrap(),
                query["recent_context"].as_str().unwrap(),
                &SearchSettings::default(),
            )
            .await
            .unwrap();
        cold_duration += cold_started.elapsed();
        let stats = searcher.cache_stats();
        let warm_started = std::time::Instant::now();
        let cached = searcher
            .search(
                &db,
                &store,
                save_id,
                role_id,
                name,
                query["query"].as_str().unwrap(),
                query["recent_context"].as_str().unwrap(),
                &SearchSettings::default(),
            )
            .await
            .unwrap();
        warm_duration += warm_started.elapsed();
        warm_hits += usize::from(cached.cache.result_hit);
        assert!(cached.cache.result_hit);
        assert_eq!(cached.result, response.result);
        assert_eq!(
            stats.hybrid_searches,
            searcher.cache_stats().hybrid_searches
        );
        assert_eq!(
            stats.query_vectors.misses,
            searcher.cache_stats().query_vectors.misses
        );
        assert_eq!(response.vector_status, QueryVectorStatus::Ready);
        assert!(response.index_is_current);
        let expected = query["expected_any"].as_array().unwrap();
        let labels = fixture["documents"].as_array().unwrap();
        let hit = response.result.hits.iter().any(|hit| {
            expected.iter().any(|id| {
                matches_label(
                    hit,
                    labels.iter().find(|label| label["id"] == *id).unwrap(),
                    &fixture,
                )
            })
        });
        if expected.is_empty() {
            empty += 1;
            false_retrievals += usize::from(!response.result.hits.is_empty());
        } else {
            answered += 1;
            found += usize::from(hit);
        }
        let scope = MemoryScopeSnapshot::load(&db, &store, save_id, role_id, name)
            .await
            .unwrap();
        let model = &response.model;
        let snapshot = MemoryIndexRepo::load(&db, &scope, model).await.unwrap();
        for result in &response.result.hits {
            let indexed = snapshot
                .documents
                .iter()
                .find(|document| document.document.id == result.hit.document_id)
                .unwrap();
            assert!(scope.allows_chunk(&indexed.document, &result.hit.chunk));
            assert!(
                query["excluded"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|id| !matches_label(
                        result,
                        labels.iter().find(|label| label["id"] == *id).unwrap(),
                        &fixture
                    ))
            );
        }
        println!(
            "P3 {split} {} hit={hit} count={} chars={} matches={:?}",
            query["id"],
            response.result.hits.len(),
            response.result.diagnostics.used_characters,
            response
                .result
                .hits
                .iter()
                .map(|result| (
                    &result.hit.chunk.id,
                    &result.explanation.accepted_by,
                    result.explanation.cosine,
                    result.explanation.lexical_score,
                ))
                .collect::<Vec<_>>()
        );
    }
    println!(
        "P3 {split}: hit@5={found}/{answered}; no-answer false retrieval={false_retrievals}/{empty}"
    );
    println!(
        "P4 {split}: warm result hits={warm_hits}; cold total={cold_duration:?}; warm total={warm_duration:?}; stats={:?}",
        searcher.cache_stats()
    );
    assert!(found * 100 >= answered * 90);
    assert_eq!(false_retrievals, 0);
    if split == "calibration" {
        let mut false_positives = 0;
        for query in [
            "高铁退票手续费怎么算？",
            "咖啡豆的烘焙温度是多少？",
            "钢琴的发声原理是什么？",
            "海边城市的人口有多少？",
        ] {
            let response = searcher
                .search(
                    &db,
                    &store,
                    101,
                    2,
                    "测试角色甲",
                    query,
                    "",
                    &SearchSettings::default(),
                )
                .await
                .unwrap();
            false_positives += usize::from(!response.result.hits.is_empty());
            println!(
                "P3 calibration negative {query}: {:?}",
                response
                    .result
                    .hits
                    .iter()
                    .map(|hit| (
                        &hit.hit.chunk.id,
                        hit.explanation.cosine,
                        hit.explanation.lexical_score
                    ))
                    .collect::<Vec<_>>()
            );
        }
        println!("P3 supplemental negative false retrieval={false_positives}/4");
        assert_eq!(false_positives, 0);
    }
    db.close().await.unwrap();
}

fn run(split: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dll = root.join("src-tauri/binaries/onnxruntime.dll");
    assert!(dll.exists());
    ort::init_from(&dll).unwrap();
    let engine = EmbeddingEngine::load(
        &root.join("data/third_party/embedding_model_bge_small_zh"),
        true,
    );
    assert!(engine.is_enabled());
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(split, engine));
}

#[test]
#[ignore = "需要本地真实模型和 ONNX DLL，单独进程运行校准集"]
fn memory_search_real_calibration() {
    run("calibration");
}

#[test]
#[ignore = "参数冻结后以单独进程运行保留集"]
fn memory_search_real_holdout() {
    run("holdout");
}

#[test]
#[ignore = "初次保留集发现问题后新增独立样例，参数冻结后单独运行"]
fn memory_search_real_fresh_holdout() {
    run("holdout_v2");
}

#[test]
#[ignore = "P5 宿主检索链路，需要本地真实模型和 ONNX DLL，单独进程运行"]
fn memory_recall_real_host() {
    use crate::ai_service::game_system::memory_recall::{
        MemoryRecall, RecallSession, RecallStamp, inject_references,
    };
    use crate::ai_service::game_system::{
        game_status::GameStatus, persistent_memory_system::MemorySectionLimits,
        role_manager::GameRoleManager,
    };
    use crate::ai_service::types::LlmMessage;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    ort::init_from(root.join("src-tauri/binaries/onnxruntime.dll")).unwrap();
    let engine = EmbeddingEngine::load(
        &root.join("data/third_party/embedding_model_bge_small_zh"),
        true,
    );
    assert!(engine.is_enabled());
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let (dir, db, _) = seed().await;
        let manager = GameRoleManager::new(dir.path().to_owned(), db.clone(), Arc::new(tokio::sync::RwLock::new(None)),
            Default::default(), None, false, 250, 30, MemorySectionLimits::default(), true);
        let mut status = GameStatus::new(manager);
        status.current_role_id = Some(2);
        status.active_save_id = Some(101);
        status.line_list = SaveRepo::get_memory_gameline_list(&db, 101).await.unwrap();
        assert!(!status.line_list.iter().rev().take(30).any(|line| line.content().contains("书签")));
        let original = status.line_list.clone();
        let query = "上次收到的那份小礼物是什么？";
        let mut context = vec![LlmMessage::system("角色设定"), LlmMessage::user(query)];
        let session = RecallSession::new(RecallStamp::capture(&status, "测试角色甲".into()).unwrap(), &context);
        let recall = MemoryRecall::new(engine);
        let store = NotesStore::new(dir.path());
        let snapshot = recall.retrieve(&db, &store, &session, query, Default::default(), true, std::time::Duration::from_secs(30)).await.unwrap();
        assert_eq!(snapshot.status, "ready");
        let response = snapshot.response.unwrap();
        assert_eq!(response.vector_status, QueryVectorStatus::Ready);
        let references = session.references(&response).unwrap();
        assert!(references.iter().any(|reference| reference.reference.contains("银色书签")));
        assert!(references.iter().all(|reference| !reference.reference.contains("蓝色纸鹤") && !reference.reference.contains("金色戒指")));
        let warm = recall.retrieve(&db, &store, &session, query, Default::default(), true, std::time::Duration::from_secs(5)).await.unwrap().response.unwrap();
        assert!(warm.cache.result_hit);
        assert!(session.references(&warm).unwrap().is_empty());
        session.validate(&db, &store).await.unwrap();
        inject_references(&mut context, &references);
        assert_eq!(context.last().unwrap().content, query);
        assert_eq!(status.line_list, original);
        println!("P5 real host: old gift recalled; vector ready; warm cache hit; references={}; chars={}; history unchanged",
            references.len(), references.iter().map(|reference| reference.reference.chars().count()).sum::<usize>());
        db.close().await.unwrap();
    });
}
