//! Versioned HTTPS catalog + bounded, verified game-only package installation.
use std::collections::HashSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use super::{API_VERSION, LIFECYCLE, valid_id, validate_files};
use crate::{PluginManager, manifest, types::PluginManifest};

const CATALOG_URL: &str =
    "https://raw.githubusercontent.com/sdfsfsk/LingChat-DLC/main/catalog-v2.json";
const CATALOG_LIMIT: usize = 1024 * 1024;
const PACKAGE_LIMIT: usize = 128 * 1024 * 1024;
const EXTRACT_LIMIT: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Game,
    Story,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: usize,
    pub api_version: u32,
    pub kind: ContentKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub packages: Vec<Entry>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Cache {
    pub source: String,
    pub catalog: Catalog,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            source: CATALOG_URL.into(),
            catalog: Catalog {
                schema_version: 2,
                packages: vec![],
            },
        }
    }
}

fn cache_path() -> PathBuf {
    ling_chat_main::data_dir::get_data_dir().join("dlc-catalog.json")
}

/// Recover an interrupted rename before PluginManager reconciles state.json.
/// Only our journaled, non-symlink staging directories are considered.
pub fn recover_installs(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".game-install-")
            || !entry
                .file_type()
                .is_ok_and(|t| t.is_dir() && !t.is_symlink())
        {
            continue;
        }
        let Ok(id) = std::fs::read_to_string(dir.join("install-id")) else {
            continue;
        };
        if !valid_id(&id) {
            continue;
        }
        let target = root.join(&id);
        let previous = dir.join("previous");
        if previous.exists() && !target.exists() {
            if let Err(error) = std::fs::rename(&previous, &target) {
                tracing::error!("DLC rollback retained at {}: {error}", dir.display());
                continue;
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn validate_url(text: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(text)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        bail!("Catalog/download URL must use HTTPS without credentials or fragment");
    }
    let host = url.host_str().context("Missing URL host")?;
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.parse::<std::net::IpAddr>().is_ok()
        || host.starts_with('[')
    {
        bail!("Local/IP download hosts are not allowed");
    }
    Ok(url)
}

fn validate_catalog(catalog: &Catalog) -> Result<()> {
    if catalog.schema_version != 2 || catalog.packages.len() > 256 {
        bail!("Unsupported or oversized catalog");
    }
    let mut ids = HashSet::new();
    for entry in &catalog.packages {
        if !valid_id(&entry.id) || !ids.insert(&entry.id) {
            bail!("Invalid/duplicate package id");
        }
        semver::Version::parse(&entry.version)?;
        if entry.name.is_empty()
            || entry.name.len() > 256
            || entry.description.len() > 4096
            || entry.api_version != API_VERSION
        {
            bail!("Invalid package metadata or unsupported game API");
        }
        if entry.size == 0
            || entry.size > PACKAGE_LIMIT
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            bail!("Invalid download size/checksum");
        }
        validate_url(&entry.url)?;
    }
    Ok(())
}

async fn download(
    url: &str,
    limit: usize,
    progress: Option<(&AppHandle, &str)>,
) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || validate_url(attempt.url().as_str()).is_err() {
                attempt.error("Invalid download redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()?;
    let mut response = client
        .get(validate_url(url)?)
        .header("User-Agent", "LingChat-DLC/1")
        .send()
        .await?
        .error_for_status()?;
    if response.content_length().is_some_and(|n| n > limit as u64) {
        bail!("Download exceeds size limit");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            bail!("Download exceeds size limit");
        }
        bytes.extend_from_slice(&chunk);
        if let Some((app, id)) = progress {
            let _ = app.emit_to(
                "main",
                "dlc:download-progress",
                serde_json::json!({"id":id,"received":bytes.len(),"total":limit}),
            );
        }
    }
    Ok(bytes)
}

fn read_cache() -> Result<Cache> {
    let file = cache_path();
    if !file.exists() {
        return Ok(Cache::default());
    }
    if std::fs::metadata(&file)?.len() > CATALOG_LIMIT as u64 {
        bail!("Cached catalog exceeds limit");
    }
    parse_cache(&std::fs::read(file)?)
}

fn parse_cache(bytes: &[u8]) -> Result<Cache> {
    let cache: Cache = serde_json::from_slice(bytes)?;
    // Ignore catalogs saved by older builds with a user-selected source.
    if cache.source != CATALOG_URL {
        return Ok(Cache::default());
    }
    validate_catalog(&cache.catalog)?;
    Ok(cache)
}

#[tauri::command]
pub fn game_catalog_cached() -> Result<Cache, String> {
    read_cache().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn game_catalog_sync() -> Result<Cache, String> {
    async fn sync() -> Result<Cache> {
        let bytes = download(CATALOG_URL, CATALOG_LIMIT, None).await?;
        let catalog: Catalog = serde_json::from_slice(&bytes)?;
        validate_catalog(&catalog)?;
        let cache = Cache {
            source: CATALOG_URL.into(),
            catalog,
        };
        let path = cache_path();
        let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        serde_json::to_writer(&mut temp, &cache)?;
        temp.as_file().sync_all()?;
        temp.persist(&path)?;
        Ok(cache)
    }
    sync().await.map_err(|e| e.to_string())
}

fn verify_archive(bytes: &[u8], entry: &Entry) -> Result<()> {
    if bytes.len() != entry.size
        || format!("{:x}", Sha256::digest(bytes)) != entry.sha256.to_lowercase()
    {
        bail!("Package size/SHA-256 mismatch");
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() > 1000 {
        bail!("Too many archive entries");
    }
    let mut total = 0u64;
    let mut names = HashSet::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().trim_end_matches('/');
        super::validate_relative(name)?;
        if !names.insert(name.to_lowercase()) {
            bail!("Duplicate/case-colliding archive path");
        }
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            bail!("Symlinks are not supported");
        }
        total = total
            .checked_add(file.size())
            .context("Archive size overflow")?;
        if file.size() > 64 * 1024 * 1024 || total > EXTRACT_LIMIT {
            bail!("Uncompressed package exceeds limit");
        }
    }
    Ok(())
}

fn check_package(dir: &Path, entry: &Entry) -> Result<PluginManifest> {
    let manifest_path = dir.join("manifest.toml");
    if std::fs::metadata(&manifest_path)?.len() > 65536 {
        bail!("Manifest exceeds limit");
    }
    let value = manifest::parse(&std::fs::read_to_string(manifest_path)?)?;
    if value.id != entry.id || value.version != entry.version {
        bail!("Package identity/version does not match catalog");
    }
    match entry.kind {
        ContentKind::Game => {
            if value.games.is_empty() {
                bail!("Game package has no games");
            }
            super::validate_game_only(&value)?;
        },
        ContentKind::Story => {
            if value.story_api_version != Some(entry.api_version) {
                bail!("Missing story API declaration");
            }
            super::validate_story_only(&value)?;
            super::validate_story_files(dir, &value)?;
        },
    }
    validate_files(dir, &value.games)?;
    Ok(value)
}

#[tauri::command]
pub async fn game_catalog_install(app: AppHandle, id: String) -> Result<(), String> {
    install(app, id).await.map_err(|e| e.to_string())
}

async fn install(app: AppHandle, id: String) -> Result<()> {
    let _operation = LIFECYCLE.lock().await;
    let cache = read_cache()?;
    let entry = cache
        .catalog
        .packages
        .iter()
        .find(|p| p.id == id)
        .context("Package not in synchronized catalog")?;
    let root = crate::importer::plugin_root_dir();
    std::fs::create_dir_all(&root)?;
    let target = root.join(&entry.id);
    if target.exists() {
        // Do not replace a same-id Python plugin with content from another catalog.
        let previous: PluginManifest =
            manifest::parse(&std::fs::read_to_string(target.join("manifest.toml"))?)?;
        let mut expected = entry.clone();
        expected.version = previous.version.clone();
        check_package(&target, &expected)?;
        if semver::Version::parse(&previous.version)? >= semver::Version::parse(&entry.version)? {
            bail!("Installed version is already equal or newer");
        }
    }
    let bytes = download(&entry.url, entry.size, Some((&app, &entry.id))).await?;
    verify_archive(&bytes, entry)?;
    let staging = tempfile::Builder::new()
        .prefix(".game-install-")
        .tempdir_in(&root)?;
    let zip_path = staging.path().join("package.zip");
    std::fs::write(&zip_path, bytes)?;
    let content = staging.path().join("content");
    std::fs::create_dir(&content)?;
    let extract_target = content.clone();
    tokio::task::spawn_blocking(move || {
        ling_chat_main::utils::archive::extract_zip(
            &zip_path,
            &extract_target,
            &CancellationToken::new(),
            &|_| {},
        )
    })
    .await??;
    check_package(&content, entry)?;
    super::runtime::request_close_plugin(&app, &entry.id)
        .await
        .map_err(anyhow::Error::msg)?;
    let _story = super::StoryContentGuard::acquire(&app)
        .await
        .map_err(anyhow::Error::msg)?;
    let backup = staging.path().join("previous");
    {
        use std::io::Write;
        let mut journal = std::fs::File::create(staging.path().join("install-id"))?;
        journal.write_all(entry.id.as_bytes())?;
        journal.sync_all()?;
    }
    let had_previous = target.exists();
    if had_previous {
        std::fs::rename(&target, &backup)?;
    }
    if let Err(error) = std::fs::rename(&content, &target) {
        if had_previous {
            if let Err(rollback) = std::fs::rename(&backup, &target) {
                // Keep recoverable old data when rollback itself fails.
                let retained = staging.keep();
                bail!(
                    "Install failed: {error}; rollback failed: {rollback}; old package retained at {}",
                    retained.display()
                );
            }
        }
        return Err(error.into());
    }
    // Existing state.json is preserved: updates retain enabled/config state,
    // new packages follow the plugin manager's disabled-by-default policy.
    let manager = app.state::<Arc<PluginManager>>().inner().clone();
    tokio::task::spawn_blocking(move || manager.reload()).await?;
    crate::commands::refresh_plugin_content(&app).await;
    let _ = app.emit("dlc:changed", &entry.id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(bytes: &[u8]) -> Entry {
        Entry {
            id: "test".into(),
            name: "Test".into(),
            description: String::new(),
            version: "1.0.0".into(),
            url: "https://example.com/game.zip".into(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            size: bytes.len(),
            api_version: 1,
            kind: ContentKind::Game,
        }
    }
    #[test]
    fn catalog_content_kind_must_match_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("scripts/example")).unwrap();
        std::fs::write(
            dir.path().join("scripts/example/story_config.yaml"),
            "script_name: Example\nintro_chapter: start\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("manifest.toml"), "id='test'\nname='Test'\ndescription=''\nversion='1.0.0'\nstory_api_version=1\nresources=['scripts']").unwrap();
        let mut e = entry(b"fixture");
        assert!(check_package(dir.path(), &e).is_err());
        e.kind = ContentKind::Story;
        assert!(check_package(dir.path(), &e).is_ok());
    }
    #[test]
    fn ignores_old_custom_catalogs_but_keeps_the_builtin_catalog() {
        let mut cache = Cache {
            source: "https://example.com/custom-catalog.json".into(),
            catalog: Catalog {
                schema_version: 2,
                packages: vec![entry(b"test")],
            },
        };
        let ignored = parse_cache(&serde_json::to_vec(&cache).unwrap()).unwrap();
        assert_eq!(ignored.source, CATALOG_URL);
        assert!(ignored.catalog.packages.is_empty());
        cache.source = CATALOG_URL.into();
        let retained = parse_cache(&serde_json::to_vec(&cache).unwrap()).unwrap();
        assert_eq!(retained.catalog.packages.len(), 1);
    }
    #[test]
    fn rejects_unsafe_catalog_urls_and_duplicate_ids() {
        for url in [
            "http://example.com/a",
            "https://user:secret@example.com/a",
            "https://127.0.0.1/a",
            "https://[::1]/a",
            "https://localhost/a",
        ] {
            assert!(validate_url(url).is_err());
        }
        let e = entry(b"test");
        assert!(
            validate_catalog(&Catalog {
                schema_version: 2,
                packages: vec![e.clone(), e]
            })
            .is_err()
        );
    }
    #[test]
    fn verifies_checksum_before_extraction() {
        assert!(verify_archive(b"tampered", &entry(b"original")).is_err());
    }
    #[test]
    fn rejects_archive_traversal() {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"x").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        assert!(verify_archive(&bytes, &entry(&bytes)).is_err());
    }
    #[test]
    fn restores_interrupted_update_without_losing_old_package() {
        let dir = tempfile::tempdir().unwrap();
        let pending = dir.path().join(".game-install-test");
        std::fs::create_dir_all(pending.join("previous")).unwrap();
        std::fs::write(pending.join("install-id"), "test").unwrap();
        std::fs::write(pending.join("previous/manifest.toml"), "old").unwrap();
        recover_installs(dir.path());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("test/manifest.toml")).unwrap(),
            "old"
        );
        assert!(!pending.exists());
    }
}
