# DLC integration

Game and native story implementations live in the
[LingChat-DLC extension SDK](https://github.com/sdfsfsk/LingChat-DLC/tree/main/extensions).
This host pins the Rust extension revision and frontend package integrity in its
lockfiles. SDK changes require a rebuild; downloadable content updates do not.

The host supplies plugin installation, dependency and resource management, script
lifecycle hooks, consent and command registration. Game webviews remain isolated
from host IPC. Content packages reuse existing plugin manifests and save locations.

See the SDK's `docs/` for per-package release discovery, package format and native story capabilities.

The [SDK 0.3 integration API](https://github.com/sdfsfsk/LingChat-DLC/blob/3ab33a9d74d650bfa9663f3b2dea6db8fb292b61/extensions/docs/extension-api.md)
keeps menu items, presentation, translations and native command implementations in the SDK.
