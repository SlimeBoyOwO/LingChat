// Main-application bridge for the extracted SBV2 local TTS crate.
pub mod adapter;
pub use sbv2_local_tts::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::config;

/// Runtime gate for the embedded local TTS engine.
#[derive(Clone, Debug)]
pub struct LocalTtsSwitch {
    enabled: Arc<AtomicBool>,
}

impl LocalTtsSwitch {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalTtsSwitchStatus {
    pub configured_enabled: bool,
    pub effective_enabled: bool,
}

fn read_configured_enabled(app: &AppHandle) -> Result<bool, String> {
    let store = config::settings_store(app).map_err(|e| e.to_string())?;
    Ok(store
        .get(config::keys::ENABLE_LOCAL_TTS)
        .and_then(|value| value.as_bool())
        .unwrap_or(false))
}

pub fn load_configured_enabled(app: &AppHandle) -> bool {
    read_configured_enabled(app).unwrap_or(false)
}

#[tauri::command]
pub fn tts_local_get_enabled(
    app: AppHandle,
    switch: State<'_, LocalTtsSwitch>,
) -> Result<LocalTtsSwitchStatus, String> {
    Ok(LocalTtsSwitchStatus {
        configured_enabled: read_configured_enabled(&app)?,
        effective_enabled: switch.is_enabled(),
    })
}

#[tauri::command]
pub fn tts_local_set_enabled(
    app: AppHandle,
    switch: State<'_, LocalTtsSwitch>,
    local_state: State<'_, LocalTtsState>,
    enabled: bool,
) -> Result<LocalTtsSwitchStatus, String> {
    if enabled {
        local_state.paths.ensure()?;
    }

    let store = config::settings_store(&app).map_err(|e| e.to_string())?;
    let previous = store.get(config::keys::ENABLE_LOCAL_TTS);
    store.set(config::keys::ENABLE_LOCAL_TTS, enabled);
    if let Err(error) = store.save() {
        if let Some(value) = previous {
            store.set(config::keys::ENABLE_LOCAL_TTS, value);
        } else {
            store.delete(config::keys::ENABLE_LOCAL_TTS);
        }
        return Err(format!("save local TTS switch: {error}"));
    }

    switch.set_enabled(enabled);
    Ok(LocalTtsSwitchStatus {
        configured_enabled: enabled,
        effective_enabled: enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::LocalTtsSwitch;

    #[test]
    fn local_tts_switch_can_be_changed_at_runtime() {
        let switch = LocalTtsSwitch::new(false);
        assert!(!switch.is_enabled());
        switch.set_enabled(true);
        assert!(switch.is_enabled());
        switch.set_enabled(false);
        assert!(!switch.is_enabled());
    }
}
