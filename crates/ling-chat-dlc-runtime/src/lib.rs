mod commands;
pub use commands::handler as command_handler;
pub mod games;
pub mod story;
pub fn install() {
    story::install();
    games::install();
}

#[cfg(test)]
fn install_for_tests() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(install);
}

#[cfg(test)]
mod integration_tests {
    use ling_chat_main::ai_service::game_system::script_engine::{
        events, extension, schema, validate,
    };

    #[test]
    fn registered_events_are_visible_to_host_schema_and_factory() {
        super::install_for_tests();
        extension::register_events();
        let schema = schema::build_schema();
        for kind in [
            "force_choice",
            "console_window",
            "watch_file",
            "poem_game",
            "window_title",
        ] {
            assert!(
                schema.events.iter().any(|event| event.type_key == kind),
                "{kind}"
            );
            assert!(
                events::create_event(kind, serde_json::json!({})).is_some(),
                "{kind}"
            );
        }
    }

    #[test]
    fn host_validator_keeps_native_window_authorization_after_extraction() {
        super::install_for_tests();
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Chapters")).unwrap();
        std::fs::write(
            root.path().join("story_config.yaml"),
            "script_name: Example\nintro_chapter: start\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("Chapters/start.yaml"),
            "events:\n- type: console_window\n  text: hello\n- type: chapter_end\n  next: end\n",
        )
        .unwrap();
        let report = validate::validate(root.path(), root.path(), "example", &Default::default());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "console_window.not_authorized")
        );
    }

    #[test]
    fn sdk_registers_fields_on_builtin_events_without_replacing_core_fields() {
        super::install_for_tests();
        let schema = schema::build_schema();
        let role = schema
            .events
            .iter()
            .find(|e| e.type_key == "modify_character")
            .unwrap();
        for key in ["character", "emotion", "flash", "noise", "noise_fade_in"] {
            assert_eq!(
                role.fields.iter().filter(|f| f.key == key).count(),
                1,
                "{key}"
            );
        }
        assert!(
            schema
                .story_config_fields
                .iter()
                .any(|f| f.key == "content_warning")
        );
        let background = schema
            .events
            .iter()
            .find(|e| e.type_key == "background_effect")
            .unwrap();
        let options = &background
            .fields
            .iter()
            .find(|f| f.key == "effect")
            .unwrap()
            .options;
        assert!(options.iter().any(|v| v == "Rain"));
        assert!(options.iter().any(|v| v == "Blackout"));
    }

    #[test]
    fn extension_payload_keeps_wire_format_and_does_not_copy_core_fields() {
        super::install_for_tests();
        let fields = extension::event_fields(
            "modify_character",
            &serde_json::json!({
                "characterId": 999, "action": "hide_character", "flash": " true ",
                "noise": "eyes", "noise_fade_in": "2.5"
            }),
        );
        let payload = ling_chat_main::ai_service::game_system::script_engine::responses::ModifyCharacterPayload {
            character_id: 7, emotion: Some("正常".into()), action: None, clothes: None,
            duration: Some(0.2), extension: fields,
        };
        let wire = serde_json::to_value(payload).unwrap();
        assert_eq!(wire["characterId"], 7);
        assert_eq!(wire["flash"], true);
        assert_eq!(wire["noiseFadeIn"], 2.5);
        assert!(wire.get("action").is_none());
        assert!(extension::event_fields("unknown", &wire).is_empty());
        let bg = extension::event_fields(
            "background_effect",
            &serde_json::json!({
                "text": "authored", "echo": "echo", "effect": "override"
            }),
        );
        assert_eq!(bg.len(), 2);
        assert_eq!(bg["text"], "authored");
    }
}
