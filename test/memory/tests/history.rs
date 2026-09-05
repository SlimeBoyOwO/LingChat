#[cfg(test)]
mod tests {
    use crate::ai_service::game_system::game_status::{GameStatus, HistoryChange};
    use crate::ai_service::game_system::role_manager::GameRoleManager;
    use crate::ai_service::llm::LlmSlot;
    use crate::ai_service::memory::{MemoryConfig, MemorySectionLimits};
    use crate::ai_service::service::AIService;
    use crate::ai_service::tools::executor::{
        Tool, ToolContext, ToolError, ToolExecutor, ToolResult,
    };
    use crate::ai_service::tools::registry::ToolRegistry;
    use crate::ai_service::types::{AdventureConfig, ScriptStatus};
    use crate::ai_service::types::{
        GameLine, GameMemoryBank, GameRole, LineAttributeExt, LineBase, ToolDefinition,
    };
    use crate::api::script_editor::commands::PreviewSession;
    use crate::config::tts::TtsConfig;
    use crate::db::entities::line::LineAttribute;
    use crate::db::managers::memory_repo::MemoryRepo;
    use crate::memory_test_api::scripted_provider::ScriptedProvider;
    use crate::memory_test_api::temp_db::TemporaryDatabase;
    use async_trait::async_trait;
    use serde_json::{Value, json};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::{Barrier, Mutex};

    #[test]
    fn append_preserves_runtime_epoch_and_scopes_to_its_suffix() {
        let change = HistoryChange::Append { from_idx: 12 };
        assert_eq!(change.append_from(), Some(12));
        assert_eq!(change.rewrite_from(), None);
    }

    #[test]
    fn rewrite_and_replace_all_invalidate_from_the_correct_boundary() {
        let rewrite = HistoryChange::Rewrite { from_idx: 7 };
        assert_eq!(rewrite.append_from(), None);
        assert_eq!(rewrite.rewrite_from(), Some(7));
        assert_eq!(HistoryChange::ReplaceAll.rewrite_from(), Some(0));
    }

    #[test]
    fn preview_and_restore_rebuild_context_without_formal_rewrite_semantics() {
        assert_eq!(HistoryChange::Preview.rewrite_from(), None);
        assert_eq!(HistoryChange::Preview.append_from(), None);
        assert_eq!(HistoryChange::Restore.rewrite_from(), None);
        assert_eq!(HistoryChange::Restore.append_from(), None);
    }

    struct CountedSideEffectTool(Arc<AtomicUsize>);

    #[async_trait]
    impl Tool for CountedSideEffectTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::new(
                "counted_side_effect",
                "controlled side-effect counter for session-gate regression",
                json!({
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": false
                }),
            )
        }

        async fn execute(
            &self,
            _: &ToolContext,
            arguments: Value,
        ) -> Result<ToolResult, ToolError> {
            if arguments != json!({}) {
                return Err(ToolError::InvalidArguments("expected empty object".into()));
            }
            self.0.fetch_add(1, Ordering::AcqRel);
            Ok(json!({"ok": true}))
        }
    }

    fn preview_script() -> ScriptStatus {
        ScriptStatus {
            folder_key: "preview-test".into(),
            name: "preview-test".into(),
            description: String::new(),
            intro_chapter: "start".into(),
            settings: Default::default(),
            script_path: std::path::PathBuf::new(),
            recommand_start: String::new(),
            adventure: AdventureConfig::default(),
            running_client_id: None,
            current_chapter_key: String::new(),
            current_event_process: 0,
            vars: Default::default(),
            plugin_id: None,
        }
    }

    #[tokio::test]
    async fn feed_identity_rejects_preview_enter_restore_and_keeps_current_formal_append() {
        let db = TemporaryDatabase::open().await.unwrap();
        let manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            Arc::new(tokio::sync::RwLock::new(None)),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: false,
                update_interval: 1,
                recent_window: 0,
                limits: MemorySectionLimits::default(),
            },
        );
        let mut status = GameStatus::new(manager);

        // This represents feed_image/feed_text's identity captured before an
        // analysis or lock await. Enter then restore leaves Normal mode, but
        // the generation change must still reject all formal writes and the
        // analysis-None downstream admission check.
        let stale_formal = status.history_session();
        assert!(!stale_formal.is_preview);
        status.role_manager.set_memory_preview(true);
        status.preview_generation = status.preview_generation.wrapping_add(1);
        status.role_manager.set_memory_preview(false);
        status.preview_generation = status.preview_generation.wrapping_add(1);
        assert!(
            !status.is_history_session_current(stale_formal),
            "analysis None must terminate before LLM/tool/memory/save admission"
        );
        assert!(
            !status
                .append_line_if_current(
                    &db.connection,
                    stale_formal,
                    LineBase {
                        content: "stale feed must not enter restored formal history".into(),
                        attribute: LineAttributeExt(LineAttribute::User),
                        display_name: Some("旁白".into()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap(),
            "feed_text must not write either preview or restored formal history"
        );
        assert!(status.line_list.is_empty());

        let current_formal = status.history_session();
        assert!(!current_formal.is_preview);
        assert!(status.is_history_session_current(current_formal));
        assert!(
            status
                .append_line_if_current(
                    &db.connection,
                    current_formal,
                    LineBase {
                        content: "current feed enters formal history".into(),
                        attribute: LineAttributeExt(LineAttribute::User),
                        display_name: Some("旁白".into()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap(),
            "without a session switch, feed requests retain normal behavior"
        );
        assert_eq!(status.line_list.len(), 1);
    }

    #[tokio::test]
    async fn entry_greeting_session_rejects_preview_round_trip_before_generator_admission() {
        let db = TemporaryDatabase::open().await.unwrap();
        let manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            Arc::new(tokio::sync::RwLock::new(None)),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: false,
                update_interval: 1,
                recent_window: 0,
                limits: MemorySectionLimits::default(),
            },
        );
        let mut status = GameStatus::new(manager);

        // This is the same one-shot identity captured by notify_player_entry
        // before its greeting write and rechecked after LLM/config setup.
        let greeting_session = status.history_session();
        assert!(!greeting_session.is_preview);
        assert!(status.is_history_session_current(greeting_session));

        status.role_manager.set_memory_preview(true);
        status.preview_generation = status.preview_generation.wrapping_add(1);
        status.role_manager.set_memory_preview(false);
        status.preview_generation = status.preview_generation.wrapping_add(1);

        assert!(
            !status.is_history_session_current(greeting_session),
            "a preview enter+restore must reject the already-written greeting's generator admission"
        );
        let current = status.history_session();
        assert!(!current.is_preview);
        assert_ne!(current.generation, greeting_session.generation);
        assert!(status.is_history_session_current(current));
    }

    #[tokio::test]
    async fn ai_dialogue_session_rejects_preview_round_trip_before_generator_or_tool_admission() {
        let db = TemporaryDatabase::open().await.unwrap();
        let manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            Arc::new(tokio::sync::RwLock::new(None)),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: false,
                update_interval: 1,
                recent_window: 0,
                limits: MemorySectionLimits::default(),
            },
        );
        let status = Arc::new(Mutex::new(GameStatus::new(manager)));
        let dialogue_session = status.lock().await.history_session();
        assert!(!dialogue_session.is_preview);

        {
            let mut gs = status.lock().await;
            gs.role_manager.set_memory_preview(true);
            gs.preview_generation = gs.preview_generation.wrapping_add(1);
            gs.role_manager.set_memory_preview(false);
            gs.preview_generation = gs.preview_generation.wrapping_add(1);
        }

        // ai_dialogue_event performs this exact recheck after slot_snapshot;
        // generator's tool loop uses the same production admission API. A
        // stale event therefore cannot start an LLM generator or a tool.
        assert!(
            !status
                .lock()
                .await
                .is_history_session_current(dialogue_session)
        );
        assert!(
            GameStatus::admit_tool_execution_if_current(&status, dialogue_session)
                .await
                .is_none(),
            "stale AI dialogue must not admit a tool after preview restoration"
        );

        let current = status.lock().await.history_session();
        assert!(!current.is_preview);
        assert_ne!(current.generation, dialogue_session.generation);
        assert!(
            GameStatus::admit_tool_execution_if_current(&status, current)
                .await
                .is_some(),
            "the current coherent session retains normal generator/tool admission"
        );
    }

    #[tokio::test]
    async fn stale_tool_admission_is_rejected_and_preview_cannot_cross_an_admitted_execution() {
        let db = TemporaryDatabase::open().await.unwrap();
        let manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            Arc::new(tokio::sync::RwLock::new(None)),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: false,
                update_interval: 1,
                recent_window: 0,
                limits: MemorySectionLimits::default(),
            },
        );
        let status = Arc::new(Mutex::new(GameStatus::new(manager)));
        let formal = status.lock().await.history_session();

        // The exact unified tool-admission API used by MessageGenerator's
        // tool_loop callback rejects a request after Preview changed identity:
        // neither the first tool nor a following LLM/tool round is admitted.
        {
            let mut gs = status.lock().await;
            gs.role_manager.set_memory_preview(true);
            gs.preview_generation = gs.preview_generation.wrapping_add(1);
            gs.role_manager.set_memory_preview(false);
            gs.preview_generation = gs.preview_generation.wrapping_add(1);
        }
        let first_request_llm_calls = AtomicUsize::new(1);
        let registry = ToolRegistry::new();
        let side_effects = Arc::new(AtomicUsize::new(0));
        registry
            .register(Arc::new(CountedSideEffectTool(side_effects.clone())))
            .unwrap();
        let executor = ToolExecutor::new(&registry);
        let context = ToolContext::new(HashSet::from(["counted_side_effect".to_string()]));
        if GameStatus::admit_tool_execution_if_current(&status, formal)
            .await
            .is_some()
        {
            let _ = executor
                .execute("counted_side_effect", "{}", &context)
                .await;
            first_request_llm_calls.fetch_add(1, Ordering::AcqRel);
        }
        assert_eq!(side_effects.load(Ordering::Acquire), 0);
        assert_eq!(
            first_request_llm_calls.load(Ordering::Acquire),
            1,
            "a stale response cannot begin a next LLM round"
        );
        assert!(status.lock().await.line_list.is_empty());

        // A multi-tool response may complete one admitted side effect before
        // Preview starts. Once it switches, the second execute and all later
        // LLM rounds are rejected; there is no canonical backfill, memory, or
        // save admission for this old request.
        let current = status.lock().await.history_session();
        let first_permit = GameStatus::admit_tool_execution_if_current(&status, current)
            .await
            .expect("current first tool must be admitted");
        let result = executor
            .execute("counted_side_effect", "{}", &context)
            .await;
        assert!(result.contains("\"ok\":true"));
        drop(first_permit);
        {
            let mut gs = status.lock().await;
            gs.role_manager.set_memory_preview(true);
            gs.preview_generation = gs.preview_generation.wrapping_add(1);
        }
        assert!(
            GameStatus::admit_tool_execution_if_current(&status, current)
                .await
                .is_none(),
            "the second executor.execute must not start after Preview"
        );
        assert_eq!(
            side_effects.load(Ordering::Acquire),
            1,
            "only the tool admitted before Preview may have its side effect"
        );
        assert!(status.lock().await.line_list.is_empty());

        // For a current request, admission retains the same preview gate used
        // by PreviewSession. A transition queued after admission cannot change
        // generation until that one executor call releases its permit, which
        // removes the check-then-await window for irreversible tool effects.
        {
            let mut gs = status.lock().await;
            gs.role_manager.set_memory_preview(false);
            gs.preview_generation = gs.preview_generation.wrapping_add(1);
        }
        let current = status.lock().await.history_session();
        let permit = GameStatus::admit_tool_execution_if_current(&status, current)
            .await
            .expect("current tool must be admitted");
        let transitioned = Arc::new(AtomicUsize::new(0));
        let transition_status = status.clone();
        let transition_count = transitioned.clone();
        let transition = tokio::spawn(async move {
            let gate = transition_status.lock().await.preview_session_gate();
            let _gate = gate.lock_owned().await;
            let mut gs = transition_status.lock().await;
            gs.role_manager.set_memory_preview(true);
            gs.preview_generation = gs.preview_generation.wrapping_add(1);
            transition_count.fetch_add(1, Ordering::AcqRel);
        });
        tokio::task::yield_now().await;
        assert_eq!(transitioned.load(Ordering::Acquire), 0);
        drop(permit);
        transition.await.unwrap();
        assert_eq!(transitioned.load(Ordering::Acquire), 1);
    }

    #[tokio::test]
    async fn preview_boundary_uses_empty_history_and_restores_canonical_history_and_bank() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, role_id) = db.seed_save_role(715, "preview").await.unwrap();
        let provider = ScriptedProvider {
            delay_ms: 30,
            ..Default::default()
        };
        let llm: LlmSlot = provider.clone().slot();
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
                display_name: Some("preview".into()),
                ..Default::default()
            },
        );
        let mut bank = GameMemoryBank::default();
        bank.meta.last_processed_global_idx = 1;
        bank.data.long_term = "canonical summary".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_id, role_id, &bank)
            .await
            .unwrap();
        manager
            .load_memory_banks_from_db(&db.connection, save_id, Some(&[role_id]))
            .await
            .unwrap();
        let mut status = GameStatus::new(manager);
        status.present_role_ids.insert(role_id);
        let canonical = vec![GameLine::from_base(
            LineBase {
                content: "real session".into(),
                attribute: LineAttributeExt(LineAttribute::User),
                sender_role_id: Some(0),
                ..Default::default()
            },
            vec![role_id],
        )];
        status.line_list = canonical.clone();

        // Start a normal-session compression, then enter preview while all four
        // section requests are still in flight. The preview boundary must make
        // that job stale rather than letting it commit to the formal bank.
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: "formal pending compression".into(),
                    attribute: LineAttributeExt(LineAttribute::Assistant),
                    sender_role_id: Some(role_id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        while provider.calls() == 0 {
            tokio::task::yield_now().await;
        }
        let canonical = status.line_list.clone();
        let formal_snapshot = status.role_manager.memory_snapshot(role_id).unwrap();
        assert!(formal_snapshot.updating);

        // This is the same production boundary used by PreviewSession::begin.
        status.role_manager.set_memory_preview(true);
        status
            .replace_preview_history(&db.connection, Vec::new())
            .await
            .unwrap();
        assert!(
            status
                .role_manager
                .wait_memory_updates(Duration::from_secs(2))
                .await,
            "the invalidated formal task must release its one-flight slot"
        );
        assert!(status.line_list.is_empty());
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: "preview-only".into(),
                    attribute: LineAttributeExt(LineAttribute::Assistant),
                    sender_role_id: Some(role_id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(status.line_list.len(), 1);
        let isolated_snapshot = status.role_manager.memory_snapshot(role_id).unwrap();
        assert_eq!(
            isolated_snapshot.bank, bank,
            "a formal task invalidated by preview must not advance or replace permanent memory"
        );
        assert_eq!(isolated_snapshot.revision, formal_snapshot.revision);
        assert_eq!(
            provider.calls(),
            4,
            "preview must not start a new compaction"
        );

        // A preview rollback touches the already processed formal prefix only
        // numerically. Its explicit Preview history change must not turn that
        // local index into a formal Rewrite and clear the formal bank.
        status.truncate_lines(&db.connection, 0).await.unwrap();
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: "preview replacement".into(),
                    attribute: LineAttributeExt(LineAttribute::Assistant),
                    sender_role_id: Some(role_id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        // Multiple tool-result backfills use the same mid-history insertion
        // production API. They must remain preview-local even though each is a
        // normal-session Rewrite when used outside preview.
        status
            .insert_lines(
                &db.connection,
                0,
                vec![
                    LineBase {
                        content: "preview tool result 1".into(),
                        attribute: LineAttributeExt(LineAttribute::Tool),
                        ..Default::default()
                    },
                    LineBase {
                        content: "preview tool result 2".into(),
                        attribute: LineAttributeExt(LineAttribute::Tool),
                        ..Default::default()
                    },
                ],
            )
            .await
            .unwrap();
        assert_eq!(status.line_list.len(), 3);
        let during_preview = status.role_manager.memory_snapshot(role_id).unwrap();
        assert_eq!(during_preview.bank, formal_snapshot.bank);
        assert_eq!(during_preview.revision, formal_snapshot.revision);
        assert!(!during_preview.updating);
        assert_eq!(
            during_preview.bank, formal_snapshot.bank,
            "preview rollback and tool inserts must not mutate the formal runtime"
        );

        // This is the same production boundary used by PreviewSession::restore.
        status
            .restore_preview_history(&db.connection, canonical.clone())
            .await
            .unwrap();
        assert_eq!(status.line_list, canonical);
        let restored = status.role_manager.memory_snapshot(role_id).unwrap();
        assert_eq!(restored.bank, formal_snapshot.bank);
        assert_eq!(restored.revision, formal_snapshot.revision);
        assert!(!restored.updating);
        assert_eq!(
            restored.bank, formal_snapshot.bank,
            "preview exit restores formal history without changing bank or revision"
        );

        // Outside preview, the same rollback remains a formal Rewrite and
        // resets the bank because it touches the processed prefix.
        status.truncate_lines(&db.connection, 0).await.unwrap();
        let reset = status.role_manager.memory_snapshot(role_id).unwrap();
        assert_eq!(reset.bank, GameMemoryBank::default());
        assert_eq!(reset.revision, formal_snapshot.revision.wrapping_add(1));

        // Normal mode resumes after exit: this new canonical line starts and
        // commits a fresh compression rather than inheriting preview state.
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: "formal after preview".into(),
                    attribute: LineAttributeExt(LineAttribute::Assistant),
                    sender_role_id: Some(role_id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(
            status
                .role_manager
                .wait_memory_updates(Duration::from_secs(2))
                .await
        );
        let resumed = status.role_manager.memory_snapshot(role_id).unwrap();
        assert_eq!(resumed.bank.meta.last_processed_global_idx, 1);
        assert_eq!(
            provider.calls(),
            8,
            "only normal history launches the new job"
        );
    }

    #[tokio::test]
    async fn preview_role_scope_removes_temporary_default_runtime_before_formal_save() {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, formal_role_id) =
            db.seed_save_role(722, "formal-preview-role").await.unwrap();
        let preview_role_id = db
            .seed_unloadable_role(723, "preview-only-role")
            .await
            .unwrap();
        let llm: LlmSlot = Arc::new(tokio::sync::RwLock::new(None));
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
            formal_role_id,
            GameRole {
                role_id: Some(formal_role_id),
                display_name: Some("formal-preview-role".into()),
                ..Default::default()
            },
        );
        let mut formal_bank = GameMemoryBank::default();
        formal_bank.meta.last_processed_global_idx = 7;
        formal_bank.data.long_term = "formal DB bank must survive preview".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_id, formal_role_id, &formal_bank)
            .await
            .unwrap();
        manager
            .load_memory_banks_from_db(&db.connection, save_id, Some(&[formal_role_id]))
            .await
            .unwrap();
        let formal_runtime = manager.memory_snapshot(formal_role_id).unwrap();
        let mut status = GameStatus::new(manager);
        status.active_save_id = Some(save_id);
        status.main_role_id = Some(preview_role_id);
        status.current_role_id = Some(formal_role_id);
        status.present_role_ids.insert(formal_role_id);
        status.line_list = vec![GameLine::from_base(
            LineBase {
                content: "formal history".into(),
                attribute: LineAttributeExt(LineAttribute::Assistant),
                sender_role_id: Some(formal_role_id),
                ..Default::default()
            },
            vec![formal_role_id],
        )];
        let formal_lines = status.line_list.clone();
        let game_status = Arc::new(Mutex::new(status));

        // The missing preview role makes `get_role` fail only after begin has
        // enabled Preview mode and captured the formal resource scope. Its
        // cleanup must therefore remove no formal objects and leave no default
        // runtime for the attempted role.
        let err = match PreviewSession::begin(
            &db.connection,
            db.directory.path(),
            &game_status,
            &preview_script(),
        )
        .await
        {
            Ok(_) => panic!("unloadable preview role unexpectedly started"),
            Err(err) => err,
        };
        assert!(err.contains("载入主角失败"));
        {
            let status = game_status.lock().await;
            assert_eq!(status.line_list, formal_lines);
            assert!(!status.role_manager.is_memory_preview());
            assert!(status.role_manager.get_loaded(preview_role_id).is_none());
            assert!(
                status
                    .role_manager
                    .memory_snapshot(preview_role_id)
                    .is_none()
            );
            let restored = status.role_manager.memory_snapshot(formal_role_id).unwrap();
            assert_eq!(restored.bank, formal_runtime.bank);
            assert_eq!(restored.revision, formal_runtime.revision);
        }

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
        service.game_status = game_status.clone();
        service.save_current_session(save_id).await.unwrap();
        let persisted = MemoryRepo::load_for_save(&db.connection, save_id)
            .await
            .unwrap();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[&formal_role_id], formal_bank);
        assert!(!persisted.contains_key(&preview_role_id));
    }

    #[tokio::test]
    async fn preview_role_scope_preserves_unloaded_role_bank_after_history_tool_rollback_and_save()
    {
        let db = TemporaryDatabase::open().await.unwrap();
        let (save_id, formal_role_id) = db
            .seed_save_role(724, "formal-preloaded-role")
            .await
            .unwrap();
        // This is intentionally not loaded before PreviewSession::begin. It
        // already exists in DB, like an editor MAIN role not used by the active
        // formal chat, and therefore reproduces the leaked-default-runtime path.
        let preview_role_id = db
            .seed_loadable_main_role(725, "preview-loaded-role")
            .await
            .unwrap();
        let mut preview_db_bank = GameMemoryBank::default();
        preview_db_bank.meta.last_processed_global_idx = 9;
        preview_db_bank.data.long_term =
            "unloaded preview role DB bank must not be defaulted".into();
        MemoryRepo::upsert_for_save_role(
            &db.connection,
            save_id,
            preview_role_id,
            &preview_db_bank,
        )
        .await
        .unwrap();
        let provider = ScriptedProvider {
            delay_ms: 200,
            ..Default::default()
        };
        let llm: LlmSlot = provider.clone().slot();
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
            formal_role_id,
            GameRole {
                role_id: Some(formal_role_id),
                display_name: Some("formal-preloaded-role".into()),
                ..Default::default()
            },
        );
        let mut formal_bank = GameMemoryBank::default();
        formal_bank.meta.last_processed_global_idx = 3;
        formal_bank.data.long_term = "non-default DB bank must survive preview".into();
        MemoryRepo::upsert_for_save_role(&db.connection, save_id, formal_role_id, &formal_bank)
            .await
            .unwrap();
        manager
            .load_memory_banks_from_db(&db.connection, save_id, Some(&[formal_role_id]))
            .await
            .unwrap();
        let retained = manager.role_resource_ids();
        let formal_runtime = manager.memory_snapshot(formal_role_id).unwrap();
        let mut status = GameStatus::new(manager);
        status.active_save_id = Some(save_id);
        status.main_role_id = Some(preview_role_id);
        status.current_role_id = Some(formal_role_id);
        status.present_role_ids.insert(formal_role_id);
        status.line_list = vec![GameLine::from_base(
            LineBase {
                content: "formal canonical history".into(),
                attribute: LineAttributeExt(LineAttribute::Assistant),
                sender_role_id: Some(formal_role_id),
                ..Default::default()
            },
            vec![formal_role_id],
        )];
        let formal_lines = status.line_list.clone();
        let game_status = Arc::new(Mutex::new(status));

        // Reproduce the former check-then-write window with the production
        // API. The writer captures formal identity, pauses at a barrier, then
        // PreviewSession enters *and restores* (so mode is Normal again) before
        // it attempts its canonical append. Only generation-aware atomic
        // append may reject this stale producer.
        let barrier = Arc::new(Barrier::new(2));
        let writer_status = game_status.clone();
        let writer_db = db.connection.clone();
        let writer_barrier = barrier.clone();
        let stale_writer = tokio::spawn(async move {
            let expected = writer_status.lock().await.history_session();
            writer_barrier.wait().await;
            writer_barrier.wait().await;
            let mut status = writer_status.lock().await;
            status
                .append_line_if_current(
                    &writer_db,
                    expected,
                    LineBase {
                        content: "must not leak after preview restore".into(),
                        attribute: LineAttributeExt(LineAttribute::Assistant),
                        sender_role_id: Some(formal_role_id),
                        ..Default::default()
                    },
                )
                .await
                .unwrap()
        });
        barrier.wait().await;
        let session = PreviewSession::begin(
            &db.connection,
            db.directory.path(),
            &game_status,
            &preview_script(),
        )
        .await
        .unwrap();
        {
            let mut status = game_status.lock().await;
            assert!(status.role_manager.get_loaded(preview_role_id).is_some());
            assert!(
                status
                    .role_manager
                    .memory_snapshot(preview_role_id)
                    .is_some()
            );
            // The same conditional production API admits a current preview
            // producer; only stale identities are rejected.
            let preview_session = status.history_session();
            assert!(
                status
                    .append_line_if_current(
                        &db.connection,
                        preview_session,
                        LineBase {
                            content: "current preview conditional write".into(),
                            attribute: LineAttributeExt(LineAttribute::Assistant),
                            sender_role_id: Some(preview_role_id),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap()
            );
            // Exercise preview-local history, tool backfill and rollback via
            // the exact production mutation APIs before leaving the scope.
            status
                .append_line(
                    &db.connection,
                    LineBase {
                        content: "preview reply".into(),
                        attribute: LineAttributeExt(LineAttribute::Assistant),
                        sender_role_id: Some(preview_role_id),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            // Preview normally suppresses compaction. Start the temporary
            // runtime through its production trigger explicitly to exercise
            // the hard cleanup case: a Running task must be detached under
            // GameStatus and abort/join only after that lock is released.
            let preview_lines = status.line_list.clone();
            let temporary_runtime = status
                .role_manager
                .memory_runtime_for_test(preview_role_id)
                .expect("preview role has a runtime");
            temporary_runtime.check_and_trigger_auto_update(&preview_lines);
            assert!(temporary_runtime.is_updating());
            status
                .insert_lines(
                    &db.connection,
                    1,
                    vec![LineBase {
                        content: "preview tool result".into(),
                        attribute: LineAttributeExt(LineAttribute::Tool),
                        ..Default::default()
                    }],
                )
                .await
                .unwrap();
            status.truncate_lines(&db.connection, 1).await.unwrap();
            assert_eq!(
                status
                    .role_manager
                    .memory_snapshot(formal_role_id)
                    .unwrap()
                    .bank,
                formal_runtime.bank
            );
        }
        tokio::time::timeout(
            Duration::from_secs(2),
            session.restore(&db.connection, &game_status),
        )
        .await
        .expect("preview exit must not deadlock on a Running temporary runtime");
        barrier.wait().await;
        assert!(
            !stale_writer.await.unwrap(),
            "a producer admitted before preview must not append after stop/restore"
        );
        {
            let status = game_status.lock().await;
            assert_eq!(status.line_list, formal_lines);
            assert_eq!(status.role_manager.role_resource_ids(), retained);
            assert!(status.role_manager.get_loaded(formal_role_id).is_some());
            assert!(status.role_manager.get_loaded(preview_role_id).is_none());
            assert!(
                status
                    .role_manager
                    .memory_snapshot(preview_role_id)
                    .is_none()
            );
            let restored = status.role_manager.memory_snapshot(formal_role_id).unwrap();
            assert_eq!(restored.bank, formal_runtime.bank);
            assert_eq!(restored.revision, formal_runtime.revision);
        }

        let mut service = AIService::new(
            db.connection.clone(),
            db.directory.path().to_path_buf(),
            llm,
            TtsConfig::default(),
            None,
            false,
            1,
            0,
            MemorySectionLimits::default(),
        )
        .await;
        service.game_status = game_status;
        service.save_current_session(save_id).await.unwrap();
        let persisted = MemoryRepo::load_for_save(&db.connection, save_id)
            .await
            .unwrap();
        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[&formal_role_id], formal_bank);
        assert_eq!(
            persisted[&preview_role_id], preview_db_bank,
            "formal save must retain an unloaded role's existing DB bank rather than writing the preview-created default runtime"
        );
    }
}
