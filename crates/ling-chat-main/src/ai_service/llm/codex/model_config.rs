//! Codex 模型目录独立于对话模型配置的版本偏好。

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tauri::Wry;
use tauri_plugin_store::Store;

use super::models::normalize_client_version;
use crate::config::keys;

pub(crate) fn load_client_versions(store: &Store<Wry>) -> BTreeMap<String, String> {
    store
        .get(keys::CODEX_CLIENT_VERSIONS)
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

pub(crate) fn set_client_version(store: &Store<Wry>, provider_id: &str, version: Option<&str>) {
    let mut versions = load_client_versions(store);
    if versions.get(provider_id).map(String::as_str) == version {
        return;
    }
    match version {
        Some(version) => {
            versions.insert(provider_id.to_owned(), version.to_owned());
        },
        None => {
            versions.remove(provider_id);
        },
    }
    set_client_versions(store, &versions);
}

fn set_client_versions(store: &Store<Wry>, versions: &BTreeMap<String, String>) {
    store.set(keys::CODEX_CLIENT_VERSIONS, json!(versions));
}

/// Retain fixed versions written by the preview builds that stored them in provider records.
pub(in crate::ai_service::llm) fn migrate_preview_client_versions(store: &Store<Wry>) {
    if store.has(keys::CODEX_CLIENT_VERSIONS) {
        return;
    }
    let Some(Value::Array(providers)) = store.get(keys::LLM_PROVIDERS) else {
        return;
    };

    let versions: BTreeMap<_, _> = providers
        .iter()
        .filter_map(|provider| {
            let kind = provider.get("provider")?.as_str()?;
            if !kind.eq_ignore_ascii_case("codex") {
                return None;
            }
            let id = provider.get("id")?.as_str()?;
            let version = provider.get("codex_client_version").and_then(Value::as_str);
            let version = normalize_client_version(version).ok().flatten()?;
            Some((id.to_owned(), version.to_owned()))
        })
        .collect();
    if !versions.is_empty() {
        set_client_versions(store, &versions);
        if let Err(error) = store.save() {
            tracing::warn!("Codex client version migration save failed: {error}");
        }
    }
}
