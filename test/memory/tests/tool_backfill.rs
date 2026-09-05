#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::Database;
    use tokio::sync::RwLock;

    use crate::ai_service::game_system::game_status::GameStatus;
    use crate::ai_service::game_system::role_manager::GameRoleManager;
    use crate::ai_service::llm::LlmSlot;
    use crate::ai_service::memory::{MemoryConfig, MemorySectionLimits};
    use crate::ai_service::message_system::generator::tool_messages_to_lines;
    use crate::ai_service::types::{
        FunctionCall, GameLine, LineAttributeExt, LineBase, LlmMessage, ToolCall,
    };
    use crate::config::tts::TtsConfig;
    use crate::db::entities::line::LineAttribute;

    fn tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            type_: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: "{}".into(),
            },
        }
    }

    #[tokio::test]
    async fn tool_backfill_preserves_multiple_assistant_and_result_pairs() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let llm: LlmSlot = Arc::new(RwLock::new(None));
        let manager = GameRoleManager::new(
            std::env::temp_dir(),
            llm,
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
        status.line_list = vec![
            GameLine::from_base(
                LineBase {
                    content: "user request".into(),
                    attribute: LineAttributeExt(LineAttribute::User),
                    sender_role_id: Some(0),
                    ..Default::default()
                },
                vec![],
            ),
            GameLine::from_base(
                LineBase {
                    content: "final assistant reply".into(),
                    attribute: LineAttributeExt(LineAttribute::Assistant),
                    ..Default::default()
                },
                vec![],
            ),
        ];
        let protocol = vec![
            LlmMessage::tool(vec![tool_call("call-1", "first")]),
            LlmMessage::tool_result("call-1", r#"{"step":1}"#),
            LlmMessage::tool(vec![tool_call("call-2", "second")]),
            LlmMessage::tool_result("call-2", r#"{"step":2}"#),
        ];

        // Exercise the production conversion and GameStatus::insert_lines
        // splice path rather than testing a separately reconstructed order.
        status
            .insert_lines(&db, 1, tool_messages_to_lines(&protocol))
            .await
            .unwrap();

        assert_eq!(status.line_list.len(), 6);
        assert_eq!(status.line_list[0].base.content, "user request");
        assert!(matches!(
            status.line_list[1].attribute(),
            LineAttribute::Assistant
        ));
        assert_eq!(
            serde_json::from_str::<Vec<ToolCall>>(
                status.line_list[1].base.tool_call.as_deref().unwrap()
            )
            .unwrap()[0]
                .id,
            "call-1"
        );
        assert!(matches!(
            status.line_list[2].attribute(),
            LineAttribute::Tool
        ));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&status.line_list[2].base.content).unwrap()["tool_call_id"],
            "call-1"
        );
        assert!(matches!(
            status.line_list[3].attribute(),
            LineAttribute::Assistant
        ));
        assert_eq!(
            serde_json::from_str::<Vec<ToolCall>>(
                status.line_list[3].base.tool_call.as_deref().unwrap()
            )
            .unwrap()[0]
                .id,
            "call-2"
        );
        assert!(matches!(
            status.line_list[4].attribute(),
            LineAttribute::Tool
        ));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&status.line_list[4].base.content).unwrap()["tool_call_id"],
            "call-2"
        );
        assert_eq!(status.line_list[5].base.content, "final assistant reply");
    }
}
