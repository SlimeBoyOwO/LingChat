//! Small, opt-in persistence layer for story variables.
//!
//! A script opts in with `script_settings.persistent_vars`. Only those named
//! variables are stored, so ordinary route flags cannot leak into later runs.
//! The state file lives under `data/` and never touches user files or scripts.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ai_service::types::ScriptStatus;

const STATE_FILE_NAME: &str = "script_runtime_state.json";
const STATE_VERSION: u32 = 1;
const MAX_PLAYTHROUGH: i64 = 7;

#[derive(Default, Deserialize, Serialize)]
struct RuntimeState {
    #[serde(default = "default_state_version")]
    version: u32,
    #[serde(default)]
    scripts: HashMap<String, Map<String, Value>>,
}

const fn default_state_version() -> u32 {
    STATE_VERSION
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATE_FILE_NAME)
}

fn persistent_keys(script: &ScriptStatus) -> Vec<String> {
    script
        .settings
        .get("persistent_vars")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn read_state(data_dir: &Path) -> Result<RuntimeState> {
    let path = state_path(data_dir);
    if !path.exists() {
        return Ok(RuntimeState {
            version: STATE_VERSION,
            ..RuntimeState::default()
        });
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("无法读取剧本运行状态: {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("无法解析剧本运行状态: {}", path.display()))
}

fn write_state(data_dir: &Path, state: &RuntimeState) -> Result<()> {
    let path = state_path(data_dir);
    let content = serde_json::to_vec_pretty(state).context("无法序列化剧本运行状态")?;
    crate::ai_service::tools::atomic_replace(&path, &content)
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("无法保存剧本运行状态: {}", path.display()))
}

fn selected_values(script: &ScriptStatus, keys: &[String]) -> Map<String, Value> {
    keys.iter()
        .filter_map(|key| {
            script
                .vars
                .get(key)
                .cloned()
                .map(|value| (key.clone(), value))
        })
        .collect()
}

/// Prepare a real playthrough. `playthrough`, when opted in, is incremented
/// immediately so closing the app mid-run still counts as a return visit.
pub fn prepare_playthrough(script: &ScriptStatus, data_dir: &Path) -> ScriptStatus {
    let mut prepared = script.clone();
    let keys = persistent_keys(&prepared);
    if keys.is_empty() {
        return prepared;
    }

    let mut state = match read_state(data_dir) {
        Ok(state) => Some(state),
        Err(error) => {
            tracing::warn!("[ScriptState] {}；本次按首次进入运行且不覆盖原文件", error);
            None
        }
    };

    if let Some(saved) = state
        .as_ref()
        .and_then(|state| state.scripts.get(&prepared.path_key()))
    {
        for key in &keys {
            if let Some(value) = saved.get(key) {
                prepared.vars.insert(key.clone(), value.clone());
            }
        }
    }

    if keys.iter().any(|key| key == "playthrough") {
        let previous = prepared
            .vars
            .get("playthrough")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        prepared.vars.insert(
            "playthrough".to_string(),
            Value::from((previous + 1).clamp(1, MAX_PLAYTHROUGH)),
        );
    }

    if let Some(state) = state.as_mut() {
        state.version = STATE_VERSION;
        state
            .scripts
            .insert(prepared.path_key(), selected_values(&prepared, &keys));
        if let Err(error) = write_state(data_dir, state) {
            tracing::warn!("[ScriptState] 启动计数保存失败: {:#}", error);
        }
    }

    prepared
}

/// Preview always starts from the first-run route and never reads or writes
/// the player's real persistent story state.
pub fn prepare_preview(script: &ScriptStatus) -> ScriptStatus {
    let mut prepared = script.clone();
    if persistent_keys(&prepared)
        .iter()
        .any(|key| key == "playthrough")
    {
        prepared
            .vars
            .insert("playthrough".to_string(), Value::from(1));
    }
    prepared
}

pub fn save_playthrough(script: &ScriptStatus, data_dir: &Path) -> Result<()> {
    let keys = persistent_keys(script);
    if keys.is_empty() {
        return Ok(());
    }

    let mut state = read_state(data_dir)?;
    state.version = STATE_VERSION;
    state
        .scripts
        .insert(script.path_key(), selected_values(script, &keys));
    write_state(data_dir, &state)
}
