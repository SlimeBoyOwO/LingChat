# Downloadable game and story DLC (API v1)

The host discovers games from plugin manifests. Game code, artwork, music and
level data are distributed separately; no specific game is compiled into LingChat.
The initial content repository is <https://github.com/sdfsfsk/LingChat-DLC>.

## Player flow

Open **Settings → Plugins / DLC → DLC**, synchronize the catalog, install a package,
enable it, then use **Start Game → Mini Games** to play inside the main window.
Native stories appear under **Start Game → Story Mode**. New packages are disabled by default, matching plugins.
The mini-game list reuses the LC main-menu background, typography and transitions.
Games fill the content area without a host title/Back toolbar; their own Return
action reopens that same mini-game menu.
Updates retain their enabled state. Catalog sync never silently downloads or runs
code. The catalog URL is fixed in the host; local ZIP/7z import reuses plugin import.
Catalog caches from older, custom sources are ignored until the built-in catalog
is synchronized. Settings manages content; the game-mode menu lists playable content.
Uninstall removes the package, while `data/game_saves/<plugin>/<game>.json` remains.

## Package format

```text
manifest.toml
web/index.html
web/assets/...
LICENSE
```

```toml
id = "example-game"
name = "Example"
description = "A downloadable game"
version = "1.0.0"
author = "Example author"

[[games]]
id = "example"
name = "Example"
entry = "web/index.html"
api_version = 1
# Optional, explicitly migrated by the player before creating a new save.
# Keys must begin with the game id plus '-'. No automatic localStorage reads.
legacy_storage_keys = ["example-settings"]
```

There may be up to 32 entries in a plugin. IDs are at most 80 ASCII letters,
digits, hyphens or underscores. Entries must be existing HTML files under `web/`;
asset requests are confined to the entry's parent directory. Build with relative
asset paths (`base: './'` in Vite), including module Worker paths. A content-only
game package needs no Python scripts. Old plugin manifests remain compatible.

## Catalog and install transaction

```json
{
  "schema_version": 2,
  "packages": [
    {
      "id": "example-game",
      "name": "Example",
      "description": "A downloadable game",
      "version": "1.0.0",
      "url": "https://example.com/releases/example-game-1.0.0.zip",
      "sha256": "<64 hexadecimal digits>",
      "size": 12345,
      "api_version": 1,
      "kind": "game"
    }
  ]
}
```

The catalog is limited to 1 MiB / 256 entries; downloads to 128 MiB, archives to
1000 entries / 256 MiB expanded / 64 MiB per file. Downloads and every redirect
must use HTTPS. Local/IP literal hosts, URL credentials, duplicate IDs, unsupported
versions, archive path escapes, duplicate/case-colliding paths and symlinks are
rejected. The existing archive extractor also enforces compression-ratio limits.
The exact download length and SHA-256 are verified before extraction; manifest
identity/version and entry files are checked before replacement.

The fixed `catalog-v2.json` supports `kind: "game"` and `kind: "story"`.
The content repository retains `catalog.json` as a game-only schema-v1 catalog
for older hosts. Catalog installs reject Python tools/startup hooks,
subscriptions and privileged plugin declarations. Game packages cannot inject
resources; native story packages may declare only `resources = ["scripts"]`
with `story_api_version = 1`. The same restrictions apply to local imports.
Existing plugins without game entries retain their current behavior.
The catalog is a trust source fixed by the host; checksums establish integrity
against that catalog, not an independent author signature.

Updates stage under the plugin root, ask active game sessions to flush saves and
exit (an unresponsive game cancels the operation after five seconds), journal the
operation, rename the old package aside, then commit the validated package.
Rename failures roll back. Startup recovers journaled interrupted renames before
reconciling plugin state; game saves are never inside either installation tree.
Downgrades/equal-version replacement are rejected by catalog installation.

## Runtime boundary and API

Desktop games run in a child `dlc-*` WebView inside the main OS window, outside the host DOM. Do not add these
labels to existing capabilities. The application invoke handler rejects all
native commands from these WebViews (application commands otherwise bypass the
capability files by default). Capabilities select trusted **webview labels**,
not the containing window: a child game must not inherit the main window's
plugin/core capabilities. The trusted host controls the full content-area bounds.
Navigation is confined to the session URL; popups are denied. Responses enforce
CSP, no frames, no external scripts/connections, and no MIME sniffing.

A listener binds only `127.0.0.1` on a random port. Each launch receives a random
session token. Requests must match that token, Host and Origin; cross-site Fetch
requests are rejected. No CORS access is granted. The bridge exposes only:

- `GET <session>/_host/config`: API version and explicitly selected display/audio preferences.
- `GET/PUT <session>/_host/save`: this game's JSON object, up to 8 MiB, atomically saved.
- `POST <session>/_host/exit`: remove this child WebView, stop its listener and return to the game list.

Package code cannot choose arbitrary save paths or invoke host file, shell, LLM
or settings APIs. Save writes require JSON, and backend identities determine the
save location. Disabling, updating or deleting a package closes its game views.
Session cancellation stops the listener; a lifecycle monitor also handles
automatic/dependency-triggered disables.

The content repository SDK loads a save before mounting the game and provides
synchronous storage getters/setters backed by a serialized save queue. A normal
game Return action or application exit requests a flush. Closing a game
tears down its WebView, Workers, animation loops and AudioContexts. Device routing
uses `AudioContext.setSinkId` when supported; if the device ID is unavailable in
the game's origin it retains system routing rather than interrupting playback.

Legacy migration is explicit and copies only listed keys, retaining the original
browser data. It initializes a missing save only; migrate before the first play.

## Platforms and validation

Desktop uses Tauri's child-WebView API. Android and iOS currently return an
explicit unsupported-platform message for embedded games; the previous separate
activity is not used as a fallback. Mobile embedding needs native device validation.

Verification must cover host-native command denial, loopback origin/path checks,
package tampering, interrupted update recovery, storage reload, game exit and
Worker/audio cleanup. Desktop/phone-size browser tests are not mobile-device
tests; Android and iOS native integration still requires device CI/manual checks.

## Native story packages

```toml
id = "example-story"
name = "Example story"
description = "A native story DLC"
version = "1.0.0"
story_api_version = 1
resources = ["scripts"]
```

Put `story_config.yaml`, chapters and assets under `scripts/<folder>/`. Keep the
story's license and third-party asset notices in the package. The unified manifest
replaces the legacy importer; do not add `dlc.json` with an unrelated app-version gate.
Enabling registers the scripts with the existing story engine. Horror content
retains the normal content warning, native effect tickets, bounded cursor effects,
window limits, cancellation and declared `.chr` marker rules.

Plugin story state and roles are namespaced by `plugins/<plugin>/<script path>`.
Updates/uninstall retain persistent state and markers outside the package; the
story menu's reset action explicitly resets them. Content mutations reserve the
engine's run flag and reject while a story or editor preview is active.
Self-contained stories may bind a registered script-local character as MAIN;
global character folders remain supported. A same-name legacy game script keeps
priority: back it up and move it outside the scanned script root before enabling
its replacement DLC. Never delete old saves as part of content installation.
