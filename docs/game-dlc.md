# Downloadable game DLC (API v1)

The host discovers games from plugin manifests. Game code, artwork, music and
level data are distributed separately; no specific game is compiled into LingChat.
The initial content repository is <https://github.com/sdfsfsk/LingChat-DLC>.

## Player flow

Open **Game modes → Games / DLC**, synchronize the catalog, install a package,
enable it and choose Play. New packages are disabled by default, matching plugins.
Updates retain their enabled state. Catalog sync never silently downloads or runs
code. The catalog URL can be changed; local ZIP/7z import reuses plugin import.
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
  "schema_version": 1,
  "packages": [
    {
      "id": "example-game",
      "name": "Example",
      "description": "A downloadable game",
      "version": "1.0.0",
      "url": "https://example.com/releases/example-game-1.0.0.zip",
      "sha256": "<64 hexadecimal digits>",
      "size": 12345,
      "api_version": 1
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

Catalog installs accept **game-only** packages: Python tools/startup hooks,
subscriptions, resource injection and other privileged plugin declarations are
rejected. The same rule applies to manually imported/sideloaded manifests with
game entries, so enabling a game cannot silently enable Python capabilities.
Existing plugins without game entries retain their current behavior.
The catalog is a trust source chosen by the player; checksums establish integrity
against that catalog, not an independent author signature.

Updates stage under the plugin root, ask active game sessions to flush saves and
exit (an unresponsive game cancels the operation after five seconds), journal the
operation, rename the old package aside, then commit the validated package.
Rename failures roll back. Startup recovers journaled interrupted renames before
reconciling plugin state; game saves are never inside either installation tree.
Downgrades/equal-version replacement are rejected by catalog installation.

## Runtime boundary and API

Games run in a distinct `dlc-*` WebView, outside the host DOM. Do not add these
labels to existing capabilities. The application invoke handler rejects all
native commands from these windows (application commands otherwise bypass the
capability files by default). Plugin/core APIs have no matching capabilities.
Navigation is confined to the session URL; popups are denied. Responses enforce
CSP, no frames, no external scripts/connections, and no MIME sniffing.

A listener binds only `127.0.0.1` on a random port. Each launch receives a random
session token. Requests must match that token, Host and Origin; cross-site Fetch
requests are rejected. No CORS access is granted. The bridge exposes only:

- `GET <session>/_host/config`: API version and explicitly selected display/audio preferences.
- `GET/PUT <session>/_host/save`: this game's JSON object, up to 8 MiB, atomically saved.
- `POST <session>/_host/exit`: close this game window and stop its listener.

Package code cannot choose arbitrary save paths or invoke host file, shell, LLM
or settings APIs. Save writes require JSON, and backend identities determine the
save location. Disabling, updating or deleting a package closes its windows.
Window destruction cancels the listener; a lifecycle monitor also handles
automatic/dependency-triggered disables.

The content repository SDK loads a save before mounting the game and provides
synchronous storage getters/setters backed by a serialized save queue. A normal
window close requests a flush; a second close can force exit. Closing a game
tears down its WebView, Workers, animation loops and AudioContexts. Device routing
uses `AudioContext.setSinkId` when supported; if the device ID is unavailable in
the game's origin it retains system routing rather than interrupting playback.

Legacy migration is explicit and copies only listed keys, retaining the original
browser data. It initializes a missing save only; migrate before the first play.

## Platforms and validation

Desktop uses a separate game window. Android wiring provides a separate
`DlcActivity` and allows cleartext traffic only to loopback in release builds;
debug builds retain LAN development-server support. iOS enables scene support
and local networking. Platforms without Tauri multi-window support return an
explicit unsupported-platform error (Android support requires API 32+).

Verification must cover host-native command denial, loopback origin/path checks,
package tampering, interrupted update recovery, storage reload, game exit and
Worker/audio cleanup. Desktop/phone-size browser tests are not mobile-device
tests; Android and iOS native integration still requires device CI/manual checks.
