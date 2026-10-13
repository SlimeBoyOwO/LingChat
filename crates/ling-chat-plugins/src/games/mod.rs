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
    if !value.tools.is_empty()
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
