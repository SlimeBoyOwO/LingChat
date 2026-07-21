// In-process SBV2 / Style-Bert-VITS2 local TTS engine.
//
// Optional alternative to the cloud TTS adapters under `adapters/`. Models are
// imported at runtime from a local file picker or downloaded from the
// registry (see `registry`) - never bundled into the APK.
//
// Sibling modules:
// - `paths`         filesystem layout + path helpers
// - `registry`      curated asset catalog (added in Task 3)
// - `archive`       zip/7z inspection + install roundtrip (Task 4-5)
// - `download`      streaming download + SHA256 + cancel (Task 6)
// - `model_manager` list/delete installed models (Task 7)
// - `engine`        LocalTtsEngine with take-and-spawn pattern (Task 8)
// - `adapter`       impl TtsAdapter for LocalTtsEngine (Task 9)
// - `commands`      tauri commands + LocalTtsState (Task 10)

mod adapter;
mod archive;
mod commands;
mod download;
mod engine;
mod model_manager;
mod paths;
mod registry;

pub use paths::{LocalTtsPaths, REQUIRED_ASSETS, VoiceInstallInfo};
