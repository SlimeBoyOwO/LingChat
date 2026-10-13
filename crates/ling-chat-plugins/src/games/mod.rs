//! Downloadable games share plugin installation/state, but execute in a separate
//! WebView with no Tauri capabilities. HTTP exposes only this game's files/save.
pub mod catalog;
pub mod runtime;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub static LIFECYCLE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
pub const API_VERSION: u32 = 1;

/// A web game cannot obtain Python/host privileges by being enabled in the game
/// library. Applies to manual imports and sideloaded manifests as well as catalogs.
pub fn validate_game_only(value: &crate::types::PluginManifest) -> Result<()> {
    semver::Version::parse(&value.version).context("Game package version must use SemVer")?;
    if value.story_api_version.is_some()
        || !value.tools.is_empty()
        || !value.resources.is_empty()
        || !value.subscribe.is_empty()
        || !value.ws.is_empty()
        || value.startup.is_some()
        || !value.read.is_empty()
        || !value.env.is_empty()
        || !value.config.is_empty()
        || value.send_user_message
        || !value.depends_on.is_empty()
    {
        bail!("Game packages cannot declare privileged plugin capabilities");
    }
    Ok(())
}

/// Story DLCs carry declarative scripts/assets, never Python or network hooks.
pub fn validate_story_only(value: &crate::types::PluginManifest) -> Result<()> {
    let Some(version) = value.story_api_version else {
        return Ok(());
    };
    semver::Version::parse(&value.version)?;
    if version != API_VERSION
        || !value.games.is_empty()
        || value.resources != [crate::types::ResourceKind::Scripts]
        || !value.tools.is_empty()
        || !value.subscribe.is_empty()
        || !value.ws.is_empty()
        || !value.ws_allow.is_empty()
        || value.startup.is_some()
        || !value.read.is_empty()
        || !value.env.is_empty()
        || !value.config.is_empty()
        || value.send_user_message
        || !value.depends_on.is_empty()
    {
        bail!("Story DLC must declare only scripts and a supported story API");
    }
    Ok(())
}

pub fn validate_story_files(root: &Path, manifest: &crate::types::PluginManifest) -> Result<()> {
    if manifest.story_api_version.is_none() {
        return Ok(());
    }
    let dirs = crate::resources::script_package_dirs(&root.join("scripts"));
    if dirs.is_empty() {
        bail!("Story DLC has no scripts");
    }
    for dir in dirs {
        let relative = dir
            .strip_prefix(root)?
            .join("story_config.yaml")
            .to_string_lossy()
            .replace('\\', "/");
        contained_file(root, &relative)?;
        ling_chat_main::ai_service::game_system::script_engine::ScriptManager::read_script_config(
            &dir,
        )?;
    }
    Ok(())
}

/// Shares the native engine's run reservation, so installing/disabling content
/// cannot race an active script or an editor preview reading the same assets.
pub struct StoryContentGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl StoryContentGuard {
    pub async fn acquire(app: &tauri::AppHandle) -> Result<Self, String> {
        use std::sync::atomic::Ordering;
        use tauri::Manager;
        let state = app.state::<ling_chat_main::AppState>();
        let flag = state
            .ai_service
            .lock()
            .await
            .script_manager
            .is_running
            .clone();
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "请先退出当前剧本或试玩，再管理 DLC / 插件".to_string())?;
        Ok(Self(flag))
    }
}
impl Drop for StoryContentGuard {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameSpec {
    pub id: String,
    pub name: String,
    pub entry: String,
    pub api_version: u32,
    /// Only read by the main UI after the player explicitly requests migration.
    #[serde(default)]
    pub legacy_storage_keys: Vec<String>,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

pub fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 512
        || path.contains(['\\', ':', '\0', '?', '#'])
        || path
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!("Invalid relative game path");
    }
    Ok(())
}

pub fn validate_specs(games: &[GameSpec]) -> Result<()> {
    if games.len() > 32 {
        bail!("Too many game entries");
    }
    let mut ids = HashSet::new();
    for game in games {
        if !valid_id(&game.id)
            || !ids.insert(&game.id)
            || game.name.trim().is_empty()
            || game.name.len() > 256
        {
            bail!("Invalid or duplicate game id/name");
        }
        if game.api_version != API_VERSION {
            bail!("Unsupported game API version: {}", game.api_version);
        }
        validate_relative(&game.entry)?;
        if game.legacy_storage_keys.len() > 8
            || game
                .legacy_storage_keys
                .iter()
                .any(|key| key.len() > 128 || !key.starts_with(&format!("{}-", game.id)))
        {
            bail!("Invalid legacy storage keys");
        }
        if !game.entry.starts_with("web/") || !game.entry.ends_with(".html") {
            bail!("Game entry must be an HTML file under web/");
        }
    }
    Ok(())
}

/// Canonical containment also rejects symlinks pointing outside the served root.
pub fn contained_file(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    let root = root.canonicalize().context("Game root missing")?;
    let file = root
        .join(relative)
        .canonicalize()
        .context("Game file missing")?;
    if !file.starts_with(&root) || !file.is_file() {
        bail!("Game file outside package");
    }
    Ok(file)
}

pub fn validate_files(root: &Path, games: &[GameSpec]) -> Result<()> {
    for game in games {
        contained_file(root, &game.entry)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn story_packages_cannot_request_plugin_privileges_or_game_entries() {
        let good = r#"id="story"
name="Story"
description=""
version="1.0.0"
story_api_version=1
resources=["scripts"]
"#;
        assert!(crate::manifest::parse(good).is_ok());
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
            assert!(crate::manifest::parse(&candidate).is_err(), "{extra}");
        }
    }
    #[test]
    fn native_story_catalog_validates_and_namespaces_roles() {
        let root = tempfile::tempdir().unwrap();
        let story = root.path().join("scripts/example");
        std::fs::create_dir_all(&story).unwrap();
        std::fs::write(
            story.join("story_config.yaml"),
            "script_name: Example\nintro_chapter: start\n",
        )
        .unwrap();
        let manifest = crate::manifest::parse("id='story'\nname='Story'\ndescription=''\nversion='1.0.0'\nstory_api_version=1\nresources=['scripts']").unwrap();
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
        assert!(crate::manifest::parse(manifest).is_err());
        assert!(
            crate::manifest::parse(
                &manifest.replace("send_user_message=true", "send_user_message=false")
            )
            .is_ok()
        );
    }
}
