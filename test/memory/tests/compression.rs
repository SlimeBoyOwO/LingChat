#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::ai_service::game_system::game_status::GameStatus;
    use crate::ai_service::game_system::role_manager::GameRoleManager;
    use crate::ai_service::memory::{MemoryConfig, MemorySectionLimits};
    use crate::ai_service::types::{
        GameLine, GameMemoryBank, GameRole, LineAttributeExt, LineBase,
    };
    use crate::config::tts::TtsConfig;
    use crate::db::entities::line::LineAttribute;
    use crate::memory_test_api::harness::{validate_real, validate_scripted};
    use crate::memory_test_api::scripted_provider::ScriptedProvider;
    use crate::memory_test_api::temp_db::TemporaryDatabase;

    #[tokio::test]
    async fn successful_scripted_provider_returns_four_distinct_sections() {
        let sections = validate_scripted(&ScriptedProvider::default())
            .await
            .unwrap();
        assert_eq!(sections.len(), 4);
        assert_ne!(sections[0], sections[1]);
    }

    #[tokio::test]
    async fn an_empty_response_is_a_failure_without_pointer_advance() {
        let provider = ScriptedProvider {
            empty_section: Some("promises".into()),
            ..Default::default()
        };
        let result = validate_real(
            provider,
            GameMemoryBank::default(),
            7,
            None,
            4,
            1,
            0,
            crate::ai_service::memory::MemorySectionLimits::default(),
            Duration::from_secs(5),
            false,
            false,
            "Test AI",
        )
        .await
        .unwrap();
        assert!(!result.committed);
        assert_eq!(result.processed_idx, 0);
        assert_eq!(result.calls, 4);
    }

    #[tokio::test]
    async fn display_name_reaches_production_compression_prompt() {
        let provider = ScriptedProvider::default();
        let result = validate_real(
            provider.clone(),
            GameMemoryBank::default(),
            7,
            None,
            1,
            1,
            0,
            crate::ai_service::memory::MemorySectionLimits::default(),
            Duration::from_secs(5),
            false,
            false,
            "雪月花",
        )
        .await
        .unwrap();
        assert!(result.committed);
        assert!(provider.saw_prompt_text("【角色名称】：雪月花"));
    }

    #[tokio::test]
    async fn timeout_aborts_and_joins_all_four_calls_before_returning() {
        let provider = ScriptedProvider {
            delay_ms: 250,
            ..Default::default()
        };
        let error = validate_real(
            provider.clone(),
            GameMemoryBank::default(),
            7,
            None,
            4,
            1,
            0,
            MemorySectionLimits::default(),
            Duration::from_millis(5),
            false,
            false,
            "Test AI",
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert_eq!(provider.calls(), 4);
        provider.wait_idle().await;
        assert_eq!(
            provider.active.load(std::sync::atomic::Ordering::Acquire),
            0
        );
    }

    #[tokio::test]
    async fn runtime_snapshot_is_the_only_permanent_memory_source() {
        let result = validate_real(
            ScriptedProvider::default(),
            GameMemoryBank::default(),
            7,
            None,
            2,
            1,
            0,
            MemorySectionLimits::default(),
            Duration::from_secs(5),
            false,
            false,
            "Test AI",
        )
        .await
        .unwrap();
        assert!(result.committed);
        assert_eq!(result.bank.data.short_term, "[scripted:short_term]");
        // `validate_real` obtains its result from `memory_snapshot`, the same
        // immutable source used by persistence/context; GameRole has no bank
        // field that could become a competing source of truth.
        assert!(result.system_memory.contains("[scripted:long_term]"));
    }

    #[tokio::test]
    async fn production_append_preserves_persona_across_two_compression_windows() {
        check_persona_and_threshold(250).await;
        check_persona_and_threshold(10_000).await;
    }

    async fn check_persona_and_threshold(recent_window: usize) {
        const ROLE_ID: i32 = 784;
        const PERSONA: &str = "original persona must survive compression";
        let db = TemporaryDatabase::open().await.unwrap();
        let provider = ScriptedProvider::default();
        let mut manager = GameRoleManager::new(
            db.directory.path().to_path_buf(),
            provider.clone().slot(),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: true,
                update_interval: 250,
                recent_window,
                limits: MemorySectionLimits::default(),
            },
        );
        manager.loaded_roles.insert(
            ROLE_ID,
            GameRole {
                role_id: Some(ROLE_ID),
                display_name: Some("Memory Test AI".into()),
                ..Default::default()
            },
        );
        let mut status = GameStatus::new(manager);
        status.present_role_ids.insert(ROLE_ID);
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: PERSONA.into(),
                    attribute: LineAttributeExt(LineAttribute::System),
                    sender_role_id: Some(ROLE_ID),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        // This canonical line is not perceived by ROLE_ID. It must affect
        // neither its trigger count nor the rebuilt context.
        status.line_list.push(GameLine::from_base(
            LineBase {
                content: "hidden from memory role".into(),
                attribute: LineAttributeExt(LineAttribute::User),
                sender_role_id: Some(0),
                ..Default::default()
            },
            vec![],
        ));
        for index in 0..249 {
            status
                .append_line(
                    &db.connection,
                    LineBase {
                        content: format!("visible line {index}"),
                        attribute: LineAttributeExt(LineAttribute::User),
                        sender_role_id: Some(0),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        assert_eq!(provider.calls(), 0, "249 visible lines must not trigger");
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: "visible line 249".into(),
                    attribute: LineAttributeExt(LineAttribute::User),
                    sender_role_id: Some(0),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(
            status
                .role_manager
                .wait_memory_updates(Duration::from_secs(5))
                .await
        );
        assert_eq!(
            provider.calls(),
            4,
            "250 visible lines trigger exactly one batch"
        );

        // The 251st line is retained with every one of the 250 processed
        // visible lines: runtime keeps the last N processed lines plus every
        // unprocessed tail, rather than treating the 251st append as another
        // compressed window or dropping the oldest processed line.
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: "visible line 250".into(),
                    attribute: LineAttributeExt(LineAttribute::User),
                    sender_role_id: Some(0),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let memory = status.role_manager.memory_as_json(ROLE_ID).unwrap();
        assert_eq!(
            provider.calls(),
            4,
            "one unprocessed tail line is below threshold"
        );
        assert!(
            memory
                .first()
                .is_some_and(|line| line.role == "system" && line.content.contains(PERSONA))
        );
        assert!(
            memory
                .iter()
                .any(|line| line.content.contains("visible line 0"))
        );
        assert!(
            memory
                .iter()
                .any(|line| line.content.contains("visible line 249"))
        );
        assert!(
            memory
                .iter()
                .any(|line| line.content.contains("visible line 250"))
        );
        assert!(
            !memory
                .iter()
                .any(|line| line.content.contains("hidden from memory role"))
        );

        // Drive a second full 250-visible-line cycle through the same
        // GameStatus append boundary, then append one unprocessed tail line.
        for index in 251..500 {
            status
                .append_line(
                    &db.connection,
                    LineBase {
                        content: format!("visible line {index}"),
                        attribute: LineAttributeExt(LineAttribute::User),
                        sender_role_id: Some(0),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        assert!(
            status
                .role_manager
                .wait_memory_updates(Duration::from_secs(5))
                .await
        );
        assert_eq!(
            provider.calls(),
            8,
            "two 250-visible-line cycles run two batches"
        );
        status
            .append_line(
                &db.connection,
                LineBase {
                    content: "visible line 500".into(),
                    attribute: LineAttributeExt(LineAttribute::User),
                    sender_role_id: Some(0),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let memory = status.role_manager.memory_as_json(ROLE_ID).unwrap();
        assert!(
            memory
                .first()
                .is_some_and(|line| line.role == "system" && line.content.contains(PERSONA))
        );
        assert!(
            memory
                .iter()
                .any(|line| line.content.contains("visible line 250"))
        );
        assert!(
            memory
                .iter()
                .any(|line| line.content.contains("visible line 500"))
        );
        assert_eq!(
            memory
                .iter()
                .any(|line| line.content.contains("visible line 249")),
            recent_window > 250,
            "large retained windows must not inhibit compression"
        );

        assert!(memory[0].content.contains("[scripted:long_term]"));
        assert_eq!(memory[0].content.matches(PERSONA).count(), 1);
        assert_eq!(
            status.line_list[0].base.content, PERSONA,
            "canonical persona is immutable"
        );
        status.role_manager.abort_memory_updates().await;
    }

    #[tokio::test]
    async fn append_during_update_commits_original_target_and_leaves_tail() {
        let result = validate_real(
            ScriptedProvider {
                delay_ms: 15,
                ..Default::default()
            },
            GameMemoryBank::default(),
            7,
            None,
            4,
            1,
            0,
            crate::ai_service::memory::MemorySectionLimits::default(),
            Duration::from_secs(5),
            true,
            false,
            "Test AI",
        )
        .await
        .unwrap();
        assert!(result.committed);
        assert_eq!(result.first_processed_idx, 4);
        assert!(result.second_batch_committed);
        assert_eq!(result.tail_lines, 1);
        assert_eq!(result.calls, 8);
    }
}
