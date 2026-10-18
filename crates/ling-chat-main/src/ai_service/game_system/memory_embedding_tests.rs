use crate::ai_service::embedding::EmbeddingEngine;

#[tokio::test]
#[ignore = "需要已下载的嵌入模型与 ONNX Runtime"]
async fn memory_embedding_real_index_restart() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    #[cfg(target_os = "windows")]
    ort::init_from(root.join("src-tauri/binaries/onnxruntime.dll")).unwrap();
    let engine = EmbeddingEngine::load(
        &root.join("data/third_party/embedding_model_bge_small_zh"),
        true,
    );
    assert!(engine.is_enabled());
    const EXPECTED_DIMENSION: usize = 512;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/memory_rag/acceptance.json"
    )))
    .expect("P0 验收集无效");
    for pair in fixture["semantic_pairs"].as_array().unwrap() {
        let query = pair["query"].as_str().unwrap();
        let texts = [
            query,
            pair["relevant"].as_str().unwrap(),
            pair["irrelevant"].as_str().unwrap(),
        ];
        let vectors: Vec<_> = texts
            .iter()
            .map(|text| {
                let vector = engine.embed(text).expect("记忆语料嵌入失败");
                assert_eq!(vector.len(), EXPECTED_DIMENSION);
                assert!(vector.iter().all(|value| value.is_finite()));
                let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
                assert!((norm - 1.0).abs() < 1e-3, "记忆向量未归一化: {norm}");
                vector
            })
            .collect();
        let relevant = crate::ai_service::embedding::cosine_similarity(&vectors[0], &vectors[1]);
        let irrelevant = crate::ai_service::embedding::cosine_similarity(&vectors[0], &vectors[2]);
        println!("记忆语义检查 {query}: 相关={relevant:.4}, 无关={irrelevant:.4}");
        assert!(relevant > irrelevant, "记忆语义区分不足: {query}");
    }

    let long_text = "春天我们去公园散步，看银杏和花朵。".repeat(80);
    assert!(engine.token_count(&long_text).unwrap() > engine.max_sequence_length());
    let document = ling_chat_notes::memory::MemoryDocument::from_note(
        2,
        &ling_chat_notes::Note {
            id: "long-real-model".into(),
            content: long_text.clone(),
            tags: vec![],
            created_at: "2026-10-18".into(),
        },
    )
    .unwrap();
    let chunks = ling_chat_notes::chunking::chunk_document(&document, 480, false, |text| {
        engine.token_count(text).map_err(|error| error.to_string())
    })
    .unwrap();
    assert!(chunks.len() > 1);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<String>(),
        long_text
    );
    for chunk in &chunks {
        assert!(engine.token_count(&chunk.content).unwrap() <= 480);
        assert_eq!(
            engine.embed(&chunk.content).unwrap().len(),
            EXPECTED_DIMENSION
        );
    }
    use crate::ai_service::game_system::memory_index::{IndexMode, MemoryIndexer};
    use crate::db::managers::save_repo::SaveRepo;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let dir = tempfile::tempdir().unwrap();
    let db = crate::db::init_db(dir.path()).await.unwrap();
    db.execute_unprepared("INSERT INTO role(id,name,role_type) VALUES (1,'测试用户','user'),(2,'真实模型角色','main')").await.unwrap();
    let save = SaveRepo::create_save(&db, "P2 真实向量索引").await.unwrap();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO line(id,save_id,attribute,sender_role_id,content) VALUES (1,?,'user',1,?)",
        vec![save.id.into(), long_text.clone().into()],
    ))
    .await
    .unwrap();
    db.execute_unprepared("INSERT INTO line_perception(line_id,role_id) VALUES (1,2)")
        .await
        .unwrap();
    SaveRepo::update_save_last_message(&db, save.id, Some(1))
        .await
        .unwrap();
    let store = ling_chat_notes::NotesStore::new(dir.path());
    store
        .add(
            "真实模型角色",
            "我晚上喝咖啡会睡不着。".into(),
            vec!["偏好".into()],
        )
        .unwrap();
    let indexer = MemoryIndexer::new(engine.clone());
    let first = indexer
        .sync(
            &db,
            &store,
            save.id,
            2,
            "真实模型角色",
            IndexMode::Incremental,
        )
        .await
        .unwrap();
    assert!(first.embedded > 1 && first.lexical_only == 0);
    db.close().await.unwrap();
    let reopened = crate::db::init_db(dir.path()).await.unwrap();
    let second = indexer
        .sync(
            &reopened,
            &store,
            save.id,
            2,
            "真实模型角色",
            IndexMode::Incremental,
        )
        .await
        .unwrap();
    assert_eq!(second.embedded, 0);
    assert_eq!(second.reused, first.embedded);
    assert_eq!(second.index_version, first.index_version);
    println!(
        "P2 真实模型索引: {} 个文档，{} 个分块；重开数据库后复用 {} 个向量",
        first.documents, first.chunks, second.reused
    );
    reopened.close().await.unwrap();
}
