//! In-process SBV2 / Style-Bert-VITS2 local TTS engine.
//!
//! Optional alternative to the cloud TTS adapters used by LingChat.
//! Models are imported at runtime from a local file picker or downloaded
//! from the registry (see `registry`) - never bundled into the APK.
//!
//! Sibling modules:
//! - `paths`         filesystem layout + path helpers
//! - `registry`      curated asset catalog
//! - `archive`       zip/7z inspection + install roundtrip
//! - `download`      streaming download + cancellation
//! - `model_manager` list/delete installed models
//! - `engine`        LocalTtsEngine with take-and-spawn pattern
//! - `import_bridge` SAF path staging for Android content:// URIs
//! - `zip_extract`   minimal zip/7z extraction helpers
//! - `commands`      Tauri commands + LocalTtsState

pub mod archive;
pub mod commands;
pub mod download;
pub mod engine;
pub mod import_bridge;
pub mod model_manager;
pub mod paths;
pub mod registry;
pub mod zip_extract;

pub use commands::LocalTtsState;
pub use engine::{LocalTtsEngine, SynthesizeRequest};
pub use paths::LocalTtsPaths;
