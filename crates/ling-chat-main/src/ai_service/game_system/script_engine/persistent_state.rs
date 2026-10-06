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
        .with_context(|| format!("无法保存剧本运行状态: {}", path.display()))?;
    super::dlc_transaction::sync_directory(data_dir).context("提交剧本运行状态目录项失败")
}

/// 有运行状态记录的剧本 path_key 列表（= 至少真正进入过一次的剧本）。
/// 读取失败时返回空列表——删文件彩蛋宁可缺席，不可误判。
pub(crate) fn played_script_keys(data_dir: &Path) -> Vec<String> {
    read_state(data_dir)
        .map(|state| state.scripts.keys().cloned().collect())
        .unwrap_or_default()
}

/// Opt-in story ending gate. Reads one owner's saved boolean without advancing
/// playthroughs, writing state or changing character files. Reset removes it.
pub(crate) fn entry_error(
    data_dir: &Path,
    owner: &str,
    settings: &Map<String, Value>,
) -> Result<Option<String>> {
    let Some(config) = settings.get("entry_error").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(variable) = config.get("variable").and_then(Value::as_str) else {
        return Ok(None);
    };
    let declared = settings
        .get("persistent_vars")
        .and_then(Value::as_array)
        .is_some_and(|keys| keys.iter().any(|key| key.as_str() == Some(variable)));
    if !declared || variable.is_empty() {
        return Ok(None);
    }
    let Some(message) = config.get("message").and_then(Value::as_str).map(str::trim) else {
        return Ok(None);
    };
    if message.is_empty()
        || message.chars().count() > 512
        || message
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Ok(None);
    }
    let state = read_state(data_dir)?;
    let locked = state
        .scripts
        .get(owner)
        .and_then(|values| values.get(variable))
        .and_then(Value::as_bool)
        == Some(true);
    Ok(locked.then(|| message.to_string()))
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
        },
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

pub(crate) type ScriptStateBackup = Map<String, Value>;

/// Atomically remove one script state and return its previous values. The DLC
/// uninstaller uses the backup to roll back when Windows refuses package
/// deletion, while a successful uninstall can simply discard it.
pub(crate) fn take_playthrough(
    data_dir: &Path,
    path_key: &str,
) -> Result<Option<ScriptStateBackup>> {
    let path = state_path(data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let mut state = read_state(data_dir)?;
    let Some(previous) = state.scripts.remove(path_key) else {
        return Ok(None);
    };
    write_state(data_dir, &state)?;
    Ok(Some(previous))
}

pub(crate) fn restore_playthrough(
    data_dir: &Path,
    path_key: &str,
    backup: ScriptStateBackup,
) -> Result<()> {
    let mut state = read_state(data_dir)?;
    state.version = STATE_VERSION;
    state.scripts.insert(path_key.to_string(), backup);
    write_state(data_dir, &state)
}

/// Drop one script's persisted runtime state so its next visit starts from
/// the first-run route again. Returns true when anything was removed.
pub fn reset_playthrough(data_dir: &Path, path_key: &str) -> Result<bool> {
    Ok(take_playthrough(data_dir, path_key)?.is_some())
}
