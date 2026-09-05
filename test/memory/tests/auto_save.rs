use crate::ai_service::game_system::auto_save::{
    AutoSaveManager, fingerprint_requires_save, successful_fingerprint,
};
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::memory::{MemoryConfig, MemorySectionLimits};
use crate::ai_service::service::AIService;
use crate::ai_service::types::{GameLine, GameMemoryBank, GameRole, LineAttributeExt, LineBase};
use crate::config::tts::TtsConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::memory_repo::MemoryRepo;
use crate::memory_test_api::scripted_provider::ScriptedProvider;
use crate::memory_test_api::temp_db::TemporaryDatabase;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};
use std::sync::Arc;
use tokio::sync::Mutex;

#[test]
fn memory_revision_change_requires_a_subsequent_save() {
    let saved = successful_fingerprint(42, vec![(7, 1)]);
    let unchanged = successful_fingerprint(42, vec![(7, 1)]);
    let after_memory_commit = successful_fingerprint(42, vec![(7, 2)]);

    assert!(!fingerprint_requires_save(
        Some(11),
        Some(&saved),
        11,
        &unchanged,
    ));
    assert!(fingerprint_requires_save(
        Some(11),
        Some(&saved),
        11,
        &after_memory_commit,
    ));
}

#[test]
fn failed_persistence_does_not_advance_success_fingerprint_and_retries() {
    let saved = successful_fingerprint(42, vec![(7, 1)]);
    let after_memory_commit = successful_fingerprint(42, vec![(7, 2)]);
    let mut success_marker = Some(saved.clone());

    assert!(fingerprint_requires_save(
        Some(11),
        success_marker.as_ref(),
        11,
        &after_memory_commit,
    ));
    assert_eq!(success_marker.as_ref(), Some(&saved));

    success_marker = Some(after_memory_commit.clone());
    assert!(!fingerprint_requires_save(
        Some(11),
        success_marker.as_ref(),
        11,
        &after_memory_commit,
    ));
}

#[tokio::test]
async fn real_autosave_persists_memory_that_finishes_after_line_save() {
    let db = TemporaryDatabase::open().await.unwrap();
    let (_seed_save, role_id) = db.seed_save_role(707, "late-memory").await.unwrap();
    let provider = ScriptedProvider {
        delay_ms: 100,
        ..Default::default()
    };
    let llm: LlmSlot = provider.clone().slot();
    let service = AIService::new(
        db.connection.clone(),
        db.directory.path().to_path_buf(),
        llm,
        TtsConfig::default(),
        None,
        true,
        1,
        0,
        MemorySectionLimits::default(),
    )
    .await;
    let mut manager = GameRoleManager::new(
        db.directory.path().to_path_buf(),
        provider.clone().slot(),
        TtsConfig::default(),
        None,
        MemoryConfig {
            enabled: true,
            update_interval: 1,
            recent_window: 0,
            limits: MemorySectionLimits::default(),
        },
    );
    manager.loaded_roles.insert(
        role_id,
        GameRole {
            role_id: Some(role_id),
            display_name: Some("late-memory".into()),
            ..Default::default()
        },
    );
    let status = GameStatus::new(manager);
    let shared = Arc::new(Mutex::new(service));
    shared.lock().await.game_status = Arc::new(Mutex::new(status));
    {
        let service = shared.lock().await;
        let mut status = service.game_status.lock().await;
        status.main_role_id = Some(role_id);
        status.present_role_ids.insert(role_id);
        status.line_list = (0..2)
            .map(|idx| {
                GameLine::from_base(
                    LineBase {
                        content: format!("late save line {idx}"),
                        attribute: LineAttributeExt(LineAttribute::User),
                        sender_role_id: Some(role_id),
                        ..Default::default()
                    },
                    vec![role_id],
                )
            })
            .collect();
        status.refresh_memories(&db.connection).await.unwrap();
    }
    while provider.calls() < 4 {
        tokio::task::yield_now().await;
    }

    let mut autosave = AutoSaveManager::for_test(db.connection.clone(), shared.clone());
    // The line snapshot is saved first while memory compression is in flight.
    autosave.perform_test_save().await.unwrap();
    let first_revision = autosave.test_saved_revision().unwrap();
    provider.wait_idle().await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // Make the real memory repository fail after lines/status have been
    // written. The success marker must remain at the old revision, allowing a
    // subsequent production save to retry the same memory snapshot.
    db.connection
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "ALTER TABLE memory_bank RENAME TO memory_bank_save_failure",
        ))
        .await
        .unwrap();
    assert!(autosave.perform_test_save().await.is_err());
    assert_eq!(autosave.test_saved_revision().unwrap(), first_revision);
    db.connection
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "ALTER TABLE memory_bank_save_failure RENAME TO memory_bank",
        ))
        .await
        .unwrap();

    // Lines did not change, but the real runtime memory revision did; retrying
    // now must write it and advance the marker only after DB success.
    autosave.perform_test_save().await.unwrap();
    let second_revision = autosave.test_saved_revision().unwrap();
    assert_ne!(first_revision, second_revision);
    assert!(second_revision.iter().any(|(_, revision)| *revision > 0));

    let save_id = {
        let service = shared.lock().await;
        let status = service.game_status.lock().await;
        status.active_save_id.unwrap()
    };
    let row = MemoryRepo::get_latest_memory(&db.connection, save_id, role_id)
        .await
        .unwrap()
        .unwrap();
    let persisted: GameMemoryBank = serde_json::from_str(&row.info).unwrap();
    assert_eq!(persisted.meta.last_processed_global_idx, 2);
    assert_eq!(persisted.data.short_term, "[scripted:short_term]");
}

#[tokio::test]
async fn autosave_uses_one_snapshot_when_memory_commits_during_db_writes() {
    let db = TemporaryDatabase::open().await.unwrap();
    let (_seed_save, role_id) = db.seed_save_role(708, "snapshot-barrier").await.unwrap();
    let provider = ScriptedProvider {
        delay_ms: 100,
        ..Default::default()
    };
    let llm: LlmSlot = provider.clone().slot();
    let service = AIService::new(
        db.connection.clone(),
        db.directory.path().to_path_buf(),
        llm,
        TtsConfig::default(),
        None,
        true,
        1,
        0,
        MemorySectionLimits::default(),
    )
    .await;
    let mut manager = GameRoleManager::new(
        db.directory.path().to_path_buf(),
        provider.clone().slot(),
        TtsConfig::default(),
        None,
        MemoryConfig {
            enabled: true,
            update_interval: 1,
            recent_window: 0,
            limits: MemorySectionLimits::default(),
        },
    );
    manager.loaded_roles.insert(
        role_id,
        GameRole {
            role_id: Some(role_id),
            display_name: Some("snapshot-barrier".into()),
            ..Default::default()
        },
    );
    let status = GameStatus::new(manager);
    let shared = Arc::new(Mutex::new(service));
    shared.lock().await.game_status = Arc::new(Mutex::new(status));
    {
        let service = shared.lock().await;
        let mut status = service.game_status.lock().await;
        status.main_role_id = Some(role_id);
        status.present_role_ids.insert(role_id);
        status.line_list = (0..2)
            .map(|idx| {
                GameLine::from_base(
                    LineBase {
                        content: format!("barrier line {idx}"),
                        attribute: LineAttributeExt(LineAttribute::User),
                        sender_role_id: Some(role_id),
                        ..Default::default()
                    },
                    vec![role_id],
                )
            })
            .collect();
        status.refresh_memories(&db.connection).await.unwrap();
    }
    while provider.calls() < 4 {
        tokio::task::yield_now().await;
    }
    {
        let service = shared.lock().await;
        let status = service.game_status.lock().await;
        assert!(
            status
                .role_manager
                .wait_memory_updates(std::time::Duration::from_secs(5))
                .await
        );
    }
    let mut autosave = AutoSaveManager::for_test(db.connection.clone(), shared.clone());
    autosave.perform_test_save().await.unwrap();
    let first = autosave.test_saved_revision().unwrap();

    // Trigger a real N+1 compression, then hold a SQLite writer lock while
    // the production save is in its repository phase. This forces the memory
    // commit to happen during the save rather than merely before/after it.
    {
        let service = shared.lock().await;
        let mut status = service.game_status.lock().await;
        status.line_list.push(GameLine::from_base(
            LineBase {
                content: "barrier N+1".into(),
                attribute: LineAttributeExt(LineAttribute::User),
                sender_role_id: Some(role_id),
                ..Default::default()
            },
            vec![role_id],
        ));
        status.refresh_memories(&db.connection).await.unwrap();
    }
    while provider.calls() < 8 {
        tokio::task::yield_now().await;
    }
    let secondary = db.secondary_connection().await.unwrap();
    let txn = secondary.begin().await.unwrap();
    txn.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "UPDATE save SET update_date = update_date",
    ))
    .await
    .unwrap();

    let save_task = tokio::spawn(async move {
        let result = autosave.perform_test_save().await;
        (autosave, result)
    });
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    provider.wait_idle().await;
    txn.commit().await.unwrap();
    let (mut autosave, result) = save_task.await.unwrap();
    result.unwrap();
    let second = autosave.test_saved_revision().unwrap();
    assert_eq!(second, first);
    let save_id = {
        let service = shared.lock().await;
        let status = service.game_status.lock().await;
        status.active_save_id.unwrap()
    };
    let persisted_during_n_plus_one =
        MemoryRepo::get_latest_memory(&db.connection, save_id, role_id)
            .await
            .unwrap()
            .unwrap();
    let persisted_during_n_plus_one: GameMemoryBank =
        serde_json::from_str(&persisted_during_n_plus_one.info).unwrap();
    assert_eq!(
        persisted_during_n_plus_one.meta.last_processed_global_idx,
        2
    );

    // A following production save observes the newer runtime revision and
    // persists it, proving the previous marker did not overclaim.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    autosave.perform_test_save().await.unwrap();
    let third = autosave.test_saved_revision().unwrap();
    assert!(third.iter().any(|(_, revision)| *revision > first[0].1));
    let persisted_after_n_plus_one =
        MemoryRepo::get_latest_memory(&db.connection, save_id, role_id)
            .await
            .unwrap()
            .unwrap();
    let persisted_after_n_plus_one: GameMemoryBank =
        serde_json::from_str(&persisted_after_n_plus_one.info).unwrap();
    assert_eq!(persisted_after_n_plus_one.meta.last_processed_global_idx, 3);
}
