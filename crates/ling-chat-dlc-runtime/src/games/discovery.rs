//! Discover immutable release assets from each package's own manifest.
//! No maintained central catalog, source-tree scan, or full ZIP download is needed.
use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{
    API_VERSION,
    catalog::{Catalog, ContentKind, Entry, PACKAGE_LIMIT, download, validate_catalog},
    valid_id, validate_game_only, validate_story_only,
};
use ling_chat_plugins::parse_manifest;

const PAGE_SIZE: usize = 100;
const MAX_PAGES: usize = 10;
const PAGE_LIMIT: usize = 8 * 1024 * 1024;
const MANIFEST_LIMIT: usize = 65536;
const SUFFIX: &str = ".manifest.toml";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Repository {
    owner: String,
    name: String,
}

impl Repository {
    pub(super) fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        ensure!(
            !input.contains('\\')
                && !input.contains('%')
                && !input.split('/').any(|part| part == "." || part == ".."),
            "Invalid GitHub repository path"
        );
        let (owner, name) = if input.starts_with("https://") {
            let url = reqwest::Url::parse(input).context("Invalid GitHub repository URL")?;
            ensure!(
                url.scheme() == "https"
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.port().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "GitHub repository URL must not contain credentials, a port, query or fragment"
            );
            let path = url.path().trim_matches('/');
            let parts: Vec<&str> = path.split('/').collect();
            match (url.host_str(), parts.as_slice()) {
                (Some("github.com"), [owner, name]) => ((*owner).to_string(), (*name).to_string()),
                (Some("api.github.com"), ["repos", owner, name, "releases"]) => {
                    ((*owner).to_string(), (*name).to_string())
                },
                (Some("raw.githubusercontent.com"), [owner, name, "main", "catalog-v2.json"]) => {
                    ((*owner).to_string(), (*name).to_string())
                },
                _ => bail!("Expected a GitHub repository in owner/name format"),
            }
        } else {
            let parts: Vec<&str> = input.split('/').collect();
            match parts.as_slice() {
                [owner, name] => ((*owner).to_string(), (*name).to_string()),
                _ => bail!("Expected a GitHub repository in owner/name format"),
            }
        };
        let valid_part = |part: &str, limit| {
            !part.is_empty()
                && part.len() <= limit
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        };
        ensure!(
            valid_part(&owner, 39) && valid_part(&name, 100),
            "Invalid GitHub repository owner or name"
        );
        Ok(Self { owner, name })
    }

    pub(super) fn name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    fn releases_url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/releases",
            self.owner, self.name
        )
    }

    pub(super) fn validate_asset_url(&self, text: &str) -> Result<()> {
        let url = super::catalog::validate_url(text)?;
        let parts: Vec<&str> = url.path_segments().context("Invalid asset path")?.collect();
        ensure!(
            url.host_str() == Some("github.com")
                && url.port().is_none()
                && url.query().is_none()
                && parts.len() >= 6
                && parts[0].eq_ignore_ascii_case(&self.owner)
                && parts[1].eq_ignore_ascii_case(&self.name)
                && parts[2] == "releases"
                && parts[3] == "download"
                && parts[4..].iter().all(|part| !part.is_empty()),
            "Asset must belong to the selected GitHub repository"
        );
        Ok(())
    }
}

#[derive(Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: usize,
    state: String,
    digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Clone)]
struct Package {
    id: String,
    version: semver::Version,
    version_text: String,
    manifest: Asset,
    archive: Asset,
}

fn checksum<'a>(asset: &'a Asset, limit: usize, repository: &Repository) -> Result<&'a str> {
    let sha = asset
        .digest
        .as_deref()
        .and_then(|s| s.strip_prefix("sha256:"))
        .context("Release asset has no SHA-256; re-upload it before publishing")?;
    ensure!(
        sha.len() == 64 && sha.bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid release asset digest"
    );
    ensure!(
        asset.state == "uploaded" && asset.size > 0 && asset.size <= limit,
        "Incomplete or oversized release asset: {}",
        asset.name
    );
    repository.validate_asset_url(&asset.browser_download_url)?;
    Ok(sha)
}

/// Asset names only provide grouping hints; the downloaded manifest is authoritative.
fn candidates(
    releases: Vec<Release>,
    repository: &Repository,
) -> Result<BTreeMap<String, Package>> {
    let mut selected: BTreeMap<String, Package> = BTreeMap::new();
    let mut versions: HashMap<(String, String), String> = HashMap::new();
    for release in releases {
        if release.draft || release.prerelease {
            continue;
        }
        ensure!(
            release.assets.len() <= 1024,
            "Too many assets in one release"
        );
        let mut assets = HashMap::new();
        for asset in &release.assets {
            ensure!(
                assets.insert(asset.name.as_str(), asset).is_none(),
                "Duplicate release asset name"
            );
        }
        for asset in &release.assets {
            let Some(stem) = asset.name.strip_suffix(SUFFIX) else {
                continue;
            };
            let (id, version_text) = stem
                .split_once('@')
                .context("Expected package-id@version.manifest.toml")?;
            ensure!(
                valid_id(id) && version_text.len() <= 128,
                "Invalid manifest asset identity"
            );
            let version = semver::Version::parse(version_text)?;
            let archive_name = format!("{id}-{version_text}.zip");
            let archive = *assets
                .get(archive_name.as_str())
                .with_context(|| format!("Missing ZIP beside {}", asset.name))?;
            checksum(asset, MANIFEST_LIMIT, repository)?;
            let digest = checksum(archive, PACKAGE_LIMIT, repository)?.to_ascii_lowercase();
            if let Some(previous) =
                versions.insert((id.into(), version_text.into()), digest.clone())
            {
                ensure!(
                    previous == digest,
                    "Published package {id}@{version_text} was changed without a version bump"
                );
            }
            // Ignore release ordering: independently published packages choose their own maximum version.
            if selected.get(id).is_some_and(|old| old.version >= version) {
                continue;
            }
            selected.insert(
                id.into(),
                Package {
                    id: id.into(),
                    version,
                    version_text: version_text.into(),
                    manifest: asset.clone(),
                    archive: archive.clone(),
                },
            );
            ensure!(selected.len() <= 256, "Too many DLC packages");
        }
    }
    Ok(selected)
}

fn entry(package: &Package, bytes: &[u8], repository: &Repository) -> Result<Entry> {
    ensure!(
        bytes.len() == package.manifest.size
            && format!("{:x}", Sha256::digest(bytes))
                == checksum(&package.manifest, MANIFEST_LIMIT, repository)?.to_ascii_lowercase(),
        "Package manifest checksum mismatch"
    );
    let value = parse_manifest(std::str::from_utf8(bytes)?)?;
    ensure!(
        value.id == package.id && value.version == package.version_text,
        "Manifest identity/version does not match its release assets"
    );
    let kind = if value.story_api_version.is_some() {
        validate_story_only(&value)?;
        ContentKind::Story
    } else {
        validate_game_only(&value)?;
        ensure!(
            !value.games.is_empty() && value.games.iter().all(|g| g.api_version == API_VERSION),
            "Manifest has no supported game entries"
        );
        ContentKind::Game
    };
    Ok(Entry {
        id: value.id,
        name: value.name,
        description: value.description,
        version: value.version,
        url: package.archive.browser_download_url.clone(),
        sha256: checksum(&package.archive, PACKAGE_LIMIT, repository)?.to_ascii_lowercase(),
        size: package.archive.size,
        api_version: API_VERSION,
        kind,
    })
}

/// Only selected manifests are fetched (normally a few KB); archives are fetched on install.
pub async fn discover(source: &str) -> Result<Catalog> {
    let repository = Repository::parse(source)?;
    let mut releases = Vec::new();
    for page in 1..=MAX_PAGES {
        let data = download(
            &format!(
                "{}?per_page={PAGE_SIZE}&page={page}",
                repository.releases_url()
            ),
            PAGE_LIMIT,
            None,
        )
        .await
        .context("Cannot read GitHub releases; the previous DLC cache is preserved")?;
        let batch: Vec<Release> = serde_json::from_slice(&data)?;
        let last = batch.len() < PAGE_SIZE;
        releases.extend(batch);
        if last {
            break;
        }
        if page == MAX_PAGES {
            bail!("Release history exceeds discovery limit; refusing an incomplete catalog");
        }
    }
    let mut packages = Vec::new();
    for package in candidates(releases, &repository)?.into_values() {
        let bytes = download(&package.manifest.browser_download_url, MANIFEST_LIMIT, None).await?;
        packages.push(
            entry(&package, &bytes, &repository)
                .with_context(|| format!("Invalid DLC {}", package.id))?,
        );
    }
    let catalog = Catalog {
        schema_version: 2,
        packages,
    };
    validate_catalog(&catalog, &repository)?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> Repository {
        Repository::parse("example/content").unwrap()
    }

    fn candidates(releases: Vec<Release>) -> Result<BTreeMap<String, Package>> {
        super::candidates(releases, &repository())
    }

    fn entry(package: &Package, bytes: &[u8]) -> Result<Entry> {
        super::entry(package, bytes, &repository())
    }

    #[test]
    fn accepts_github_repository_and_legacy_source_forms() {
        for source in [
            "example/content",
            "https://github.com/example/content/",
            "https://api.github.com/repos/example/content/releases",
            "https://raw.githubusercontent.com/example/content/main/catalog-v2.json",
        ] {
            assert_eq!(Repository::parse(source).unwrap().name(), "example/content");
        }
    }

    #[test]
    fn rejects_unsafe_or_ambiguous_repository_sources() {
        for source in [
            "",
            "example",
            "../content",
            "example/..",
            "example/content/extra",
            "http://github.com/example/content",
            "https://github.com.evil.test/example/content",
            "https://user:secret@github.com/example/content",
            "https://github.com/example/content?token=value",
            "https://github.com/example/content#fragment",
            "https://github.com/example%2fother/content",
            "https://github.com/example/content/../other",
            "https://127.0.0.1/example/content",
        ] {
            assert!(Repository::parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn release_assets_must_belong_to_the_selected_repository() {
        let repository = repository();
        assert!(
            repository
                .validate_asset_url(
                    "https://github.com/EXAMPLE/CONTENT/releases/download/v1/game.zip"
                )
                .is_ok()
        );
        for url in [
            "https://github.com/example/other/releases/download/v1/game.zip",
            "https://github.com/example/content-other/releases/download/v1/game.zip",
            "https://github.com.evil.test/example/content/releases/download/v1/game.zip",
            "https://github.com/example/content/releases/download/v1/game.zip?token=value",
            "https://127.0.0.1/example/content/releases/download/v1/game.zip",
        ] {
            assert!(repository.validate_asset_url(url).is_err(), "{url}");
        }
    }

    fn asset(name: &str, bytes: &[u8]) -> Asset {
        Asset {
            name: name.into(),
            browser_download_url: format!(
                "https://github.com/example/content/releases/download/test/{name}"
            ),
            size: bytes.len(),
            state: "uploaded".into(),
            digest: Some(format!("sha256:{:x}", Sha256::digest(bytes))),
        }
    }
    fn release(id: &str, version: &str) -> Release {
        let body = format!(
            "id='{id}'\nname='Package name'\ndescription='From its own manifest'\nversion='{version}'\n[[games]]\nid='play'\nname='Play'\nentry='web/index.html'\napi_version=1\n"
        );
        Release {
            draft: false,
            prerelease: false,
            assets: vec![
                asset(&format!("{id}@{version}{SUFFIX}"), body.as_bytes()),
                asset(&format!("{id}-{version}.zip"), b"verified-archive"),
            ],
        }
    }
    #[test]
    fn discovers_new_packages_and_selects_versions_without_release_order() {
        let mut sdk = release("sdk", "9.0.0");
        sdk.prerelease = true;
        let mut draft = release("hidden", "1.0.0");
        draft.draft = true;
        let found = candidates(vec![
            release("new-game", "1.9.0"),
            release("other", "3.0.0"),
            release("new-game", "1.10.0"),
            sdk,
            draft,
        ])
        .unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found["new-game"].version_text, "1.10.0");
    }
    #[test]
    fn duplicate_identical_versions_are_ok_but_changed_bytes_are_rejected() {
        assert!(candidates(vec![release("game", "1.0.0"), release("game", "1.0.0")]).is_ok());
        let mut changed = release("game", "1.0.0");
        changed.assets[1] = asset("game-1.0.0.zip", b"changed");
        assert!(candidates(vec![release("game", "1.0.0"), changed]).is_err());
    }
    #[test]
    fn refuses_incomplete_unverified_and_foreign_assets() {
        let mut missing = release("game", "1.0.0");
        missing.assets.pop();
        assert!(candidates(vec![missing]).is_err());
        let mut unverified = release("game", "1.0.0");
        unverified.assets[1].digest = None;
        assert!(candidates(vec![unverified]).is_err());
        let mut foreign = release("game", "1.0.0");
        foreign.assets[0].browser_download_url = "https://example.com/manifest".into();
        assert!(candidates(vec![foreign]).is_err());
    }
    #[test]
    fn metadata_and_kind_come_from_the_verified_manifest() {
        let body=b"id='story'\nname='An independent story'\ndescription='Discovered automatically'\nversion='2.0.0'\nstory_api_version=1\nresources=['scripts']\n";
        let mut release = release("story", "2.0.0");
        release.assets[0] = asset("story@2.0.0.manifest.toml", body);
        let selected = candidates(vec![release]).unwrap();
        let package = &selected["story"];
        let result = entry(package, body).unwrap();
        assert_eq!(result.name, "An independent story");
        assert_eq!(result.kind, ContentKind::Story);
        assert!(entry(package, b"modified").is_err());
    }
    #[test]
    fn mismatched_manifest_identity_is_rejected_even_with_a_valid_digest() {
        let body=b"id='different'\nname='Wrong identity'\ndescription=''\nversion='1.0.0'\nstory_api_version=1\nresources=['scripts']\n";
        let mut release = release("story", "1.0.0");
        release.assets[0] = asset("story@1.0.0.manifest.toml", body);
        let selected = candidates(vec![release]).unwrap();
        assert!(entry(&selected["story"], body).is_err());
    }
}
