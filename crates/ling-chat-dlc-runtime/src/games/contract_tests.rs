mod tests {
    use ling_chat_dlc_contract::*;
    #[test]
    fn story_packages_cannot_request_plugin_privileges_or_game_entries() {
        let good = r#"id="story"
name="Story"
description=""
version="1.0.0"
story_api_version=1
resources=["scripts"]
"#;
        assert!(ling_chat_plugins::parse_manifest(good).is_ok());
        for extra in [
            "send_user_message=true",
            "read=[\"secrets\"]",
            "resources=[\"characters\"]",
            "story_api_version=2",
        ] {
            let candidate = if extra.starts_with("resources=") {
                good.replace("resources=[\"scripts\"]", extra)
            } else if extra.starts_with("story_api_version=") {
                good.replace("story_api_version=1", extra)
            } else {
                format!("{good}\n{extra}")
            };
            assert!(
                ling_chat_plugins::parse_manifest(&candidate).is_err(),
                "{extra}"
            );
        }
    }
    #[test]
    fn native_story_catalog_validates_and_namespaces_roles() {
        crate::install_for_tests();
        let root = tempfile::tempdir().unwrap();
        let story = root.path().join("scripts/example");
        std::fs::create_dir_all(&story).unwrap();
        std::fs::write(
            story.join("story_config.yaml"),
            "script_name: Example\nintro_chapter: start\n",
        )
        .unwrap();
        let manifest = ling_chat_plugins::parse_manifest("id='story'\nname='Story'\ndescription=''\nversion='1.0.0'\nstory_api_version=1\nresources=['scripts']").unwrap();
        assert!(validate_story_files(root.path(), &manifest).is_ok());
        let mut script = ling_chat_main::ai_service::game_system::script_engine::ScriptManager::read_script_config(&story).unwrap();
        script.plugin_id = Some("story".into());
        assert_eq!(script.path_key(), "plugins/story/example");
        use ling_chat_main::db::entities::role::RoleType;
        let role = ling_chat_main::api::resolve_role_dir_in(
            root.path(),
            &RoleType::Npc,
            Some(&script.path_key()),
            "actor",
        )
        .unwrap();
        assert_eq!(
            role,
            root.path()
                .join("plugins/story/scripts/example/characters/actor")
        );
        script.plugin_id = Some("other".into());
        assert_eq!(script.path_key(), "plugins/other/example");
    }
    #[test]
    fn rejects_cross_platform_path_escape() {
        for path in [
            "../secret",
            "/secret",
            "C:/secret",
            "a\\b",
            "a/../b",
            "a//b",
            "a/./b",
            "a:stream",
            "",
        ] {
            assert!(validate_relative(path).is_err(), "{path}");
        }
        assert!(validate_relative("assets/music-worker.js").is_ok());
    }
    #[test]
    fn rejects_duplicate_and_unsupported_entries() {
        let game = GameSpec {
            id: "test".into(),
            name: "Test".into(),
            entry: "web/index.html".into(),
            api_version: 1,
            legacy_storage_keys: vec![],
        };
        assert!(validate_specs(&[game.clone()]).is_ok());
        assert!(validate_specs(&[game.clone(), game.clone()]).is_err());
        assert!(
            validate_specs(&[GameSpec {
                api_version: 2,
                ..game
            }])
            .is_err()
        );
    }
    #[test]
    fn rejects_privileged_game_manifest_even_without_catalog() {
        let manifest = r#"
id="example"
name="Example"
description=""
version="1.0.0"
send_user_message=true
[[games]]
id="example"
name="Example"
entry="web/index.html"
api_version=1
"#;
        assert!(ling_chat_plugins::parse_manifest(manifest).is_err());
        assert!(
            ling_chat_plugins::parse_manifest(
                &manifest.replace("send_user_message=true", "send_user_message=false")
            )
            .is_ok()
        );
    }
}
