#[cfg(test)]
mod tests {
    use crate::ai_service::game_system::game_status::GameStatus;
    use crate::ai_service::game_system::role_manager::GameRoleManager;
    use crate::ai_service::llm::LlmSlot;
    use crate::ai_service::memory::{MemoryConfig, MemorySectionLimits};
    use crate::ai_service::service::AIService;
    use crate::ai_service::types::{
        GameLine, GameMemoryBank, GameRole, LineAttributeExt, LineBase,
    };
    use crate::api::save::{create_save_for_session, update_save_for_session};
    use crate::config::tts::TtsConfig;
    use crate::db::entities::line::LineAttribute;
    use crate::db::entities::memory_bank;
    use crate::db::managers::memory_repo::MemoryRepo;
    use crate::db::managers::save_repo::SaveRepo;
    use crate::memory_test_api::temp_db::TemporaryDatabase;
    use std::fs;
    use std::sync::Arc;
    use tokio::sync::{Notify, RwLock};

    #[tokio::test]
    async fn memory_bank_round_trips_through_migrated_sqlite() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(701, "round-trip").await.unwrap();
        let mut bank = GameMemoryBank::default();
        bank.meta.last_processed_global_idx = 13;
        bank.data.short_term = "短期对话".into();
        bank.data.long_term = "Long term".into();
        bank.data.user_info = "用户偏好".into();
        bank.data.promises = "约定".into();
        let loaded = db.round_trip(save_id, role_id, &bank).await.unwrap();
        assert_eq!(loaded, bank);
    }

    #[test]
    fn multilingual_fixture_matches_game_line_serde_contract() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/memory/multilingual.json")).unwrap();
        let line = value["lines"][0].clone();
        let parsed: crate::ai_service::types::GameLine = serde_json::from_value(line).unwrap();
        assert_eq!(parsed.base.content, "风雪说：你好，世界 🌏");
        assert_eq!(parsed.base.attribute.as_str(), "user");
        assert_eq!(value["display_name"], "测试角色");
    }

    #[tokio::test]
    async fn duplicate_rows_select_latest_before_parsing() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(708, "duplicates").await.unwrap();
        let old = memory_bank::ActiveModel {
            info: sea_orm::Set("{not-json".into()),
            save_id: sea_orm::Set(save_id),
            role_id: sea_orm::Set(Some(role_id)),
            ..Default::default()
        };
        use sea_orm::ActiveModelTrait;
        old.insert(&db.connection).await.unwrap();
        let mut bank = GameMemoryBank::default();
        bank.data.long_term = "latest valid".into();
        let encoded = serde_json::to_string(&bank).unwrap();
        memory_bank::ActiveModel {
            info: sea_orm::Set(encoded),
            save_id: sea_orm::Set(save_id),
            role_id: sea_orm::Set(Some(role_id)),
            ..Default::default()
        }
        .insert(&db.connection)
        .await
        .unwrap();
        let row = MemoryRepo::get_latest_memory(&db.connection, save_id, role_id)
            .await
            .unwrap()
            .unwrap();
        let loaded: GameMemoryBank = serde_json::from_str(&row.info).unwrap();
        assert_eq!(loaded.data.long_term, "latest valid");
    }

    #[tokio::test]
    async fn role_manager_ignores_old_bad_duplicate_but_rejects_new_bad_duplicate() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(709, "duplicate-load").await.unwrap();
        use sea_orm::ActiveModelTrait;
        memory_bank::ActiveModel {
            info: sea_orm::Set("{old-bad".into()),
            save_id: sea_orm::Set(save_id),
            role_id: sea_orm::Set(Some(role_id)),
            ..Default::default()
        }
        .insert(&db.connection)
        .await
        .unwrap();
        let valid = serde_json::to_string(&GameMemoryBank::default()).unwrap();
        memory_bank::ActiveModel {
            info: sea_orm::Set(valid),
            save_id: sea_orm::Set(save_id),
            role_id: sea_orm::Set(Some(role_id)),
            ..Default::default()
        }
        .insert(&db.connection)
        .await
        .unwrap();
        let llm: LlmSlot = Arc::new(RwLock::new(None));
        let mut manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            llm,
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
                display_name: Some("duplicate-load".into()),
                ..Default::default()
            },
        );
        manager
            .load_memory_banks_from_db(&db.connection, save_id, Some(&[role_id]))
            .await
            .unwrap();

        memory_bank::ActiveModel {
            info: sea_orm::Set("{new-bad".into()),
            save_id: sea_orm::Set(save_id),
            role_id: sea_orm::Set(Some(role_id)),
            ..Default::default()
        }
        .insert(&db.connection)
        .await
        .unwrap();
        let error = manager
            .load_memory_banks_from_db(&db.connection, save_id, Some(&[role_id]))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("memory_bank.id"));
    }

    #[tokio::test]
    async fn typed_repository_loads_latest_rows_and_reports_newest_bad_json() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(710, "typed-load").await.unwrap();
        use sea_orm::ActiveModelTrait;
        memory_bank::ActiveModel {
            info: sea_orm::Set("{old-bad".into()),
            save_id: sea_orm::Set(save_id),
            role_id: sea_orm::Set(Some(role_id)),
            ..Default::default()
        }
        .insert(&db.connection)
        .await
        .unwrap();
        let mut bank = GameMemoryBank::default();
        bank.data.promises = "newest valid".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_id, role_id, &bank)
            .await
            .unwrap();
        let loaded = MemoryRepo::load_for_save(&db.connection, save_id)
            .await
            .unwrap();
        assert_eq!(loaded[&role_id], bank);

        memory_bank::ActiveModel {
            info: sea_orm::Set("{new-bad".into()),
            save_id: sea_orm::Set(save_id),
            role_id: sea_orm::Set(Some(role_id)),
            ..Default::default()
        }
        .insert(&db.connection)
        .await
        .unwrap();
        let error = MemoryRepo::load_for_save(&db.connection, save_id)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("memory_bank.id"));
    }

    #[tokio::test]
    async fn delete_for_save_removes_only_target_memory_rows() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_a, role_a) = db.seed_save_role(711, "delete-a").await.unwrap();
        let (save_b, role_b) = db.seed_save_role(712, "delete-b").await.unwrap();
        let bank = GameMemoryBank::default();
        MemoryRepo::upsert_for_save_role(&db.connection, save_a, role_a, &bank)
            .await
            .unwrap();
        MemoryRepo::upsert_for_save_role(&db.connection, save_b, role_b, &bank)
            .await
            .unwrap();

        assert_eq!(
            MemoryRepo::delete_for_save(&db.connection, save_a)
                .await
                .unwrap(),
            1
        );
        assert!(
            MemoryRepo::get_latest_memory(&db.connection, save_a, role_a)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            MemoryRepo::get_latest_memory(&db.connection, save_b, role_b)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn malformed_memory_json_is_reported() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(702, "malformed").await.unwrap();
        let error = db.malformed_json_error(save_id, role_id).await.unwrap_err();
        assert!(error.to_string().contains("malformed memory JSON"));
    }

    #[tokio::test]
    async fn rows_are_isolated_by_save_and_role() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_a, role_a) = db.seed_save_role(703, "a").await.unwrap();
        let (save_b, role_b) = db.seed_save_role(704, "b").await.unwrap();
        let mut bank = GameMemoryBank::default();
        bank.data.long_term = "A".into();
        let row_a = db.round_trip(save_a, role_a, &bank).await.unwrap();
        bank.data.long_term = "B".into();
        let row_b = db.round_trip(save_b, role_b, &bank).await.unwrap();
        assert_eq!(row_a.data.long_term, "A");
        assert_eq!(row_b.data.long_term, "B");
    }

    #[tokio::test]
    async fn ai_service_load_restore_replaces_old_bank_and_clears_missing_bank() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_a, role_id) = db.seed_save_role(716, "load-restore").await.unwrap();
        let save_b = SaveRepo::create_save(&db.connection, "load-restore-empty")
            .await
            .unwrap()
            .id;
        let llm: LlmSlot = Arc::new(RwLock::new(None));
        let mut manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            llm.clone(),
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
                display_name: Some("load-restore".into()),
                ..Default::default()
            },
        );
        let mut bank_a = GameMemoryBank::default();
        bank_a.meta.last_processed_global_idx = 3;
        bank_a.data.long_term = "save A".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_a, role_id, &bank_a)
            .await
            .unwrap();
        let mut bank_b = GameMemoryBank::default();
        bank_b.meta.last_processed_global_idx = 1;
        bank_b.data.long_term = "save B".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_b, role_id, &bank_b)
            .await
            .unwrap();

        let mut service = AIService::new(
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
        service.game_status = Arc::new(tokio::sync::Mutex::new(GameStatus::new(manager)));
        let lines = vec![GameLine::from_base(
            LineBase {
                content: "loaded line".into(),
                attribute: LineAttributeExt(LineAttribute::Assistant),
                sender_role_id: Some(role_id),
                ..Default::default()
            },
            vec![role_id],
        )];

        service.restore_memory_banks(save_b).await.unwrap();
        service
            .load_lines(lines.clone(), role_id, Some(save_b))
            .await
            .unwrap();
        assert_eq!(
            service
                .game_status
                .lock()
                .await
                .role_manager
                .memory_snapshot(role_id)
                .unwrap()
                .bank,
            bank_b
        );

        MemoryRepo::delete_for_save(&db.connection, save_b)
            .await
            .unwrap();
        service.restore_memory_banks(save_b).await.unwrap();
        service
            .load_lines(lines, role_id, Some(save_b))
            .await
            .unwrap();
        assert_eq!(
            service
                .game_status
                .lock()
                .await
                .role_manager
                .memory_snapshot(role_id)
                .unwrap()
                .bank,
            GameMemoryBank::default(),
            "a missing row must never retain the previously restored save bank"
        );
    }

    #[tokio::test]
    async fn preview_gate_rejects_load_before_runtime_bank_or_history_can_change() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (formal_save_id, role_id) =
            db.seed_save_role(721, "preview-load-formal").await.unwrap();
        let preview_target_save_id = SaveRepo::create_save(&db.connection, "preview-load-target")
            .await
            .unwrap()
            .id;
        let llm: LlmSlot = Arc::new(RwLock::new(None));
        let mut manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            llm.clone(),
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
                display_name: Some("preview-load".into()),
                ..Default::default()
            },
        );
        let mut formal_bank = GameMemoryBank::default();
        formal_bank.data.long_term = "formal bank must survive preview load".into();
        MemoryRepo::upsert_for_save_role(&db.connection, formal_save_id, role_id, &formal_bank)
            .await
            .unwrap();
        let mut target_bank = GameMemoryBank::default();
        target_bank.data.long_term = "target bank must not be imported".into();
        MemoryRepo::upsert_for_save_role(
            &db.connection,
            preview_target_save_id,
            role_id,
            &target_bank,
        )
        .await
        .unwrap();
        manager
            .load_memory_banks_from_db(&db.connection, formal_save_id, Some(&[role_id]))
            .await
            .unwrap();

        let mut service = AIService::new(
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
        let formal_lines = vec![GameLine::from_base(
            LineBase {
                content: "formal history must survive preview load".into(),
                attribute: LineAttributeExt(LineAttribute::Assistant),
                sender_role_id: Some(role_id),
                ..Default::default()
            },
            vec![role_id],
        )];
        let mut status = GameStatus::new(manager);
        status.line_list = formal_lines.clone();
        status.role_manager.set_memory_preview(true);
        service.game_status = Arc::new(tokio::sync::Mutex::new(status));

        // `load_save` takes this exact gate before its first DB/settings/runtime
        // action. Its rejection therefore leaves the preview owner's formal
        // runtime state untouched rather than importing target_save's bank.
        assert!(
            service
                .acquire_formal_session_gate()
                .await
                .unwrap_err()
                .to_string()
                .contains("试玩期间不能保存正式会话")
        );
        let status = service.game_status.lock().await;
        assert_eq!(status.line_list, formal_lines);
        assert_eq!(
            status.role_manager.memory_snapshot(role_id).unwrap().bank,
            formal_bank
        );
        assert_ne!(
            status.role_manager.memory_snapshot(role_id).unwrap().bank,
            target_bank
        );
    }

    #[tokio::test]
    async fn rollback_facade_persists_reset_memory_with_truncated_history() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(713, "rollback").await.unwrap();
        let llm: LlmSlot = Arc::new(RwLock::new(None));
        let mut service = AIService::new(
            db.connection.clone(),
            db.directory.path().to_path_buf(),
            llm.clone(),
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
            llm,
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
                display_name: Some("rollback".into()),
                ..Default::default()
            },
        );
        let mut old_bank = GameMemoryBank::default();
        old_bank.meta.last_processed_global_idx = 2;
        old_bank.data.long_term = "must not resurrect".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_id, role_id, &old_bank)
            .await
            .unwrap();
        manager
            .load_memory_banks_from_db(&db.connection, save_id, Some(&[role_id]))
            .await
            .unwrap();
        let mut status = GameStatus::new(manager);
        status.active_save_id = Some(save_id);
        status.main_role_id = Some(role_id);
        status.present_role_ids.insert(role_id);
        status.line_list = vec![
            GameLine::from_base(
                LineBase {
                    content: "kept".into(),
                    attribute: LineAttributeExt(LineAttribute::Assistant),
                    sender_role_id: Some(role_id),
                    ..Default::default()
                },
                vec![role_id],
            ),
            GameLine::from_base(
                LineBase {
                    content: "rollback from here".into(),
                    attribute: LineAttributeExt(LineAttribute::User),
                    sender_role_id: Some(0),
                    ..Default::default()
                },
                vec![role_id],
            ),
        ];
        service.game_status = Arc::new(tokio::sync::Mutex::new(status));

        let remaining = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            service.rollback_conversation(1),
        )
        .await
        .expect("rollback must not reacquire its own session gate")
        .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(
            SaveRepo::get_gameline_list(&db.connection, save_id)
                .await
                .unwrap()
                .len(),
            1
        );
        let persisted = MemoryRepo::load_for_save(&db.connection, save_id)
            .await
            .unwrap();
        assert_eq!(persisted[&role_id], GameMemoryBank::default());
    }

    #[tokio::test]
    async fn rollback_waits_for_admission_and_preview_rejection_has_no_effects() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(720, "rollback-gate").await.unwrap();
        // Canonical player lines use role 0; preserve real foreign-key checks.
        db.seed_save_role(0, "player").await.unwrap();
        let mut service = AIService::new(
            db.connection.clone(),
            db.directory.path().to_path_buf(),
            Arc::new(RwLock::new(None)),
            TtsConfig::default(),
            None,
            false,
            1,
            0,
            MemorySectionLimits::default(),
        )
        .await;
        let game_status = service.game_status.clone();
        let lines = vec![GameLine::from_base(
            LineBase {
                content: "must survive rejected rollback".into(),
                attribute: LineAttributeExt(LineAttribute::User),
                sender_role_id: Some(0),
                ..Default::default()
            },
            vec![role_id],
        )];
        {
            let mut status = game_status.lock().await;
            status.active_save_id = Some(save_id);
            status.line_list = lines.clone();
        }
        SaveRepo::sync_lines(&db.connection, save_id, &lines)
            .await
            .unwrap();
        let persisted = SaveRepo::get_gameline_list(&db.connection, save_id)
            .await
            .unwrap();
        let mut bank = GameMemoryBank::default();
        bank.data.long_term = "unchanged".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_id, role_id, &bank)
            .await
            .unwrap();

        let gate = game_status.lock().await.preview_session_gate();
        let permit = gate.lock_owned().await;
        let rollback = service.rollback_conversation(1);
        tokio::pin!(rollback);
        // Poll the real facade to its first pending await, without sleeps or
        // scheduler timing assumptions. The old implementation truncated here
        // and only blocked when trying to save; admission must block it first.
        tokio::select! {
            biased;
            result = &mut rollback => panic!("rollback bypassed held gate: {result:?}"),
            _ = std::future::ready(()) => {}
        }
        assert_eq!(game_status.lock().await.line_list, lines);
        {
            let mut status = game_status.lock().await;
            status.role_manager.set_memory_preview(true);
            status.preview_generation = status.preview_generation.wrapping_add(1);
        }
        drop(permit);
        assert!(rollback.await.is_err());
        assert_eq!(game_status.lock().await.line_list, lines);
        assert_eq!(
            SaveRepo::get_gameline_list(&db.connection, save_id)
                .await
                .unwrap(),
            persisted
        );
        assert_eq!(
            MemoryRepo::load_for_save(&db.connection, save_id)
                .await
                .unwrap()[&role_id],
            bank
        );
    }

    #[tokio::test]
    async fn snapshot_with_no_running_script_clears_existing_row_and_link() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(714, "script-clear").await.unwrap();
        let running_script_id =
            SaveRepo::upsert_running_script(&db.connection, save_id, "story", "{}", "start", 1)
                .await
                .unwrap();
        let mut service = AIService::new(
            db.connection.clone(),
            db.directory.path().to_path_buf(),
            Arc::new(RwLock::new(None)),
            TtsConfig::default(),
            None,
            false,
            1,
            0,
            MemorySectionLimits::default(),
        )
        .await;
        let mut manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            Arc::new(RwLock::new(None)),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: false,
                update_interval: 1,
                recent_window: 0,
                limits: MemorySectionLimits::default(),
            },
        );
        manager.loaded_roles.insert(
            role_id,
            GameRole {
                role_id: Some(role_id),
                ..Default::default()
            },
        );
        let mut status = GameStatus::new(manager);
        status.main_role_id = Some(role_id);
        status.script_status = None;
        service.game_status = Arc::new(tokio::sync::Mutex::new(status));
        service.save_current_session(save_id).await.unwrap();

        let save = SaveRepo::get_save_by_id(&db.connection, save_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(save.running_script_id, None);
        assert!(
            SaveRepo::get_running_script(&db.connection, running_script_id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn clearing_script_snapshot_removes_all_legacy_rows_for_the_same_save() {
        use crate::db::entities::running_script;
        use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, _) = db
            .seed_save_role(718, "duplicate-script-clear")
            .await
            .unwrap();
        for event_sequence in [1, 2] {
            running_script::ActiveModel {
                script_folder: sea_orm::Set("legacy-story".into()),
                variable_info: sea_orm::Set("{}".into()),
                current_chapter: sea_orm::Set("start".into()),
                event_sequence: sea_orm::Set(event_sequence),
                save_id: sea_orm::Set(save_id),
                ..Default::default()
            }
            .insert(&db.connection)
            .await
            .unwrap();
        }
        let linked = SaveRepo::upsert_running_script(
            &db.connection,
            save_id,
            "current-story",
            "{}",
            "start",
            3,
        )
        .await
        .unwrap();
        let (other_save_id, _) = db
            .seed_save_role(720, "script-cleanup-other")
            .await
            .unwrap();
        let other_script = SaveRepo::upsert_running_script(
            &db.connection,
            other_save_id,
            "other-story",
            "{}",
            "start",
            1,
        )
        .await
        .unwrap();
        assert!(
            SaveRepo::get_running_script(&db.connection, linked)
                .await
                .unwrap()
                .is_some()
        );

        SaveRepo::clear_running_script_for_save(&db.connection, save_id)
            .await
            .unwrap();
        assert_eq!(
            running_script::Entity::find()
                .filter(running_script::Column::SaveId.eq(save_id))
                .count(&db.connection)
                .await
                .unwrap(),
            0,
            "all legacy duplicates for this save must be removed"
        );
        assert_eq!(
            SaveRepo::get_save_by_id(&db.connection, save_id)
                .await
                .unwrap()
                .unwrap()
                .running_script_id,
            None
        );
        assert!(
            SaveRepo::get_running_script(&db.connection, other_script)
                .await
                .unwrap()
                .is_some(),
            "script cleanup must not cross the target save boundary"
        );
    }

    #[tokio::test]
    async fn preview_save_helpers_reject_before_creating_rows_copying_screenshots_or_mutating_existing_save()
     {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(717, "preview-save").await.unwrap();
        let original_lines = vec![GameLine::from_base(
            LineBase {
                content: "formal-history".into(),
                attribute: LineAttributeExt(LineAttribute::Assistant),
                sender_role_id: Some(role_id),
                ..Default::default()
            },
            vec![role_id],
        )];
        SaveRepo::sync_lines(&db.connection, save_id, &original_lines)
            .await
            .unwrap();
        let persisted_lines_before = SaveRepo::get_gameline_list(&db.connection, save_id)
            .await
            .unwrap();
        let mut original_bank = GameMemoryBank::default();
        original_bank.data.long_term = "formal-memory".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_id, role_id, &original_bank)
            .await
            .unwrap();

        let mut service = AIService::new(
            db.connection.clone(),
            db.directory.path().to_path_buf(),
            Arc::new(RwLock::new(None)),
            TtsConfig::default(),
            None,
            false,
            1,
            0,
            MemorySectionLimits::default(),
        )
        .await;
        service
            .game_status
            .lock()
            .await
            .role_manager
            .set_memory_preview(true);

        let screenshots_dir = db.directory.path().join("screenshots");
        let source = db.directory.path().join("source.png");
        fs::write(&source, b"preview screenshot").unwrap();
        let saves_before = SaveRepo::count_saves(&db.connection).await.unwrap();

        let create_error = create_save_for_session(
            &db.connection,
            &mut service,
            "must-not-create",
            Some(source.to_str().unwrap()),
            &screenshots_dir,
        )
        .await
        .unwrap_err();
        assert!(create_error.contains("试玩期间不能保存"));
        assert_eq!(
            SaveRepo::count_saves(&db.connection).await.unwrap(),
            saves_before
        );
        assert!(
            fs::read_dir(&screenshots_dir).is_err(),
            "no screenshot directory should be created"
        );

        let update_error = update_save_for_session(
            &db.connection,
            &mut service,
            save_id,
            Some(source.to_str().unwrap()),
            &screenshots_dir,
        )
        .await
        .unwrap_err();
        assert!(update_error.contains("试玩期间不能保存"));
        assert!(!screenshots_dir.join(format!("{save_id}.png")).exists());
        assert_eq!(
            SaveRepo::get_gameline_list(&db.connection, save_id)
                .await
                .unwrap(),
            persisted_lines_before,
            "preview update must not sync preview lines into the formal save"
        );
        assert_eq!(
            MemoryRepo::load_for_save(&db.connection, save_id)
                .await
                .unwrap()[&role_id],
            original_bank,
            "preview update must not overwrite the formal MemoryBank"
        );
    }

    #[tokio::test]
    async fn preview_transition_waits_for_admitted_formal_snapshot_write() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db
            .seed_save_role(719, "preview-linearization")
            .await
            .unwrap();
        let mut service = AIService::new(
            db.connection.clone(),
            db.directory.path().to_path_buf(),
            Arc::new(RwLock::new(None)),
            TtsConfig::default(),
            None,
            false,
            1,
            0,
            MemorySectionLimits::default(),
        )
        .await;
        {
            let mut status = service.game_status.lock().await;
            status.main_role_id = Some(role_id);
            status.line_list = vec![GameLine::from_base(
                LineBase {
                    content: "formal-before-preview".into(),
                    attribute: LineAttributeExt(LineAttribute::Assistant),
                    sender_role_id: Some(role_id),
                    ..Default::default()
                },
                vec![role_id],
            )];
        }

        // This is the exact admission point used by every formal save path.
        // Hold it after admission while the controlled preview transition tries
        // to start; the transition must remain blocked until this immutable
        // snapshot has completed its DB write.
        let formal_gate = service.acquire_formal_session_gate().await.unwrap();
        let started = Arc::new(Notify::new());
        let completed = Arc::new(Notify::new());
        let game_status = service.game_status.clone();
        let started_transition = started.clone();
        let completed_transition = completed.clone();
        let transition = tokio::spawn(async move {
            let gate = game_status.lock().await.preview_session_gate();
            started_transition.notify_one();
            let _preview_gate = gate.lock_owned().await;
            game_status
                .lock()
                .await
                .role_manager
                .set_memory_preview(true);
            completed_transition.notify_one();
        });
        started.notified().await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), completed.notified())
                .await
                .is_err()
        );

        let snapshot = service.capture_guarded_session_snapshot().await;
        service
            .persist_captured_formal_session(save_id, &snapshot)
            .await
            .unwrap();
        let persisted = SaveRepo::get_gameline_list(&db.connection, save_id)
            .await
            .unwrap();
        assert_eq!(persisted.len(), snapshot.lines.len());
        assert_eq!(persisted[0].base.content, snapshot.lines[0].base.content);
        assert_eq!(
            persisted[0].perceived_role_ids,
            snapshot.lines[0].perceived_role_ids
        );
        assert!(persisted[0].base.id.is_some());
        assert!(snapshot.lines[0].base.id.is_none());
        // DB assigns line ids, but all session content captured before preview
        // must be exactly the formal snapshot that was admitted.
        let persisted_before_preview = persisted;
        drop(formal_gate);
        completed.notified().await;
        transition.await.unwrap();
        assert!(service.is_preview_session().await);

        // Once preview has linearized first, the identical production facade
        // rejects before it can issue a second formal DB write.
        assert!(service.save_current_session(save_id).await.is_err());
        assert_eq!(
            SaveRepo::get_gameline_list(&db.connection, save_id)
                .await
                .unwrap(),
            persisted_before_preview
        );
    }

    #[tokio::test]
    async fn typed_upsert_is_scoped_to_its_save_and_role() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_a, role_a) = db.seed_save_role(705, "a").await.unwrap();
        let (save_b, role_b) = db.seed_save_role(706, "b").await.unwrap();
        let mut first = GameMemoryBank::default();
        first.data.long_term = "A".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_a, role_a, &first)
            .await
            .unwrap();

        let mut second = GameMemoryBank::default();
        second.data.long_term = "B".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_b, role_b, &second)
            .await
            .unwrap();

        let banks_a = MemoryRepo::load_for_save(&db.connection, save_a)
            .await
            .unwrap();
        let banks_b = MemoryRepo::load_for_save(&db.connection, save_b)
            .await
            .unwrap();
        assert_eq!(banks_a[&role_a], first);
        assert_eq!(banks_b[&role_b], second);
    }
}
