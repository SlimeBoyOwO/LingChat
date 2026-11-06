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
pub fn validate_game_only(value: &impl Serialize) -> Result<()> {
    let value = view(value)?;
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
pub fn validate_story_only(value: &impl Serialize) -> Result<()> {
    let value = view(value)?;
    let Some(version) = value.story_api_version else {
        return Ok(());
    };
    semver::Version::parse(&value.version)?;
    if version != API_VERSION
        || !value.games.is_empty()
        || value.resources != ["scripts"]
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

pub fn validate_story_files(root: &Path, manifest: &impl Serialize) -> Result<()> {
    if view(manifest)?.story_api_version.is_none() {
        return Ok(());
    }
    let dirs = (runtime::hooks()?.script_directories)(&root.join("scripts"));
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

#[derive(Deserialize)]
struct ManifestView {
    version: String,
    #[serde(default)]
    games: Vec<GameSpec>,
    #[serde(default)]
    story_api_version: Option<u32>,
    #[serde(default)]
    resources: Vec<String>,
    #[serde(default)]
    tools: Vec<serde_json::Value>,
    #[serde(default)]
    subscribe: Vec<serde_json::Value>,
    #[serde(default)]
    ws: Vec<serde_json::Value>,
    #[serde(default)]
    ws_allow: Vec<serde_json::Value>,
    #[serde(default)]
    startup: Option<serde_json::Value>,
    #[serde(default)]
    read: Vec<serde_json::Value>,
    #[serde(default)]
    env: Vec<serde_json::Value>,
    #[serde(default)]
    config: Vec<serde_json::Value>,
    #[serde(default)]
    send_user_message: bool,
    #[serde(default)]
    depends_on: Vec<String>,
}
fn view(value: &impl Serialize) -> Result<ManifestView> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}
