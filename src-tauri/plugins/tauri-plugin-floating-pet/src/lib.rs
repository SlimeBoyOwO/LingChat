//! `tauri-plugin-floating-pet` \u2014 Android \u7cf5\u6d6e\u684c\u5ba0\u63a5\u53e3 (B1 \u539f\u751f View \u8def\u7ebf)
//!
//! \u4ec5\u5728 Android \u4e0a\u5b9e\u9645\u6267\u884c\uff0c\u5176\u4ed6\u5e73\u53f0\u8c03\u7528\u8fd4\u56de [`PetError::UnsupportedPlatform`] \u6216\u65e0\u6548\u8fd4\u56de\u503c\u3002
//! \u8be6\u7ec6\u67b6\u6784\u89c1 `android-floating-pet-spec.md` \u3002

#![cfg_attr(not(target_os = "android"), allow(unused_variables))]

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

pub mod commands;
pub mod error;
pub mod state;
pub mod bridge;

pub use commands::*;
pub use error::PetError;
pub use state::CachedPetState;
pub use bridge::{PetBridge, PetBridgeExt};

pub const PLUGIN_NAME: &str = "floating-pet";

/// \u521d\u59cb\u5316\u63d2\u4ef6\u3002
///
/// \u5728 Android \u4e0a\u8d1f\u8d23\u5411 Tauri \u6ce8\u518c Kotlin `\u539f\u751f\u63d2\u4ef6\u7c7b\uff08FloatingPetPlugin\uff09`\u3002
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::<R>::new(PLUGIN_NAME)
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle = _api.register_android_plugin(
                    "com.noiq.lingchat.floatingpet",
                    "FloatingPetPlugin",
                )?;
                _app.manage(PetBridge::new(handle));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_overlay_permission,
            commands::request_overlay_permission,
            commands::show_floating_pet,
            commands::hide_floating_pet,
            commands::stop_floating_pet_service,
            commands::update_pet_state,
            commands::start_permission_explanation,
            commands::mark_permission_explanation_shown,
        ])
        .build()
}
