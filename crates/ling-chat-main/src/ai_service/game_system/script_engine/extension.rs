//! Optional script lifecycle integration. Implementations live outside the core engine.
use crate::ai_service::types::ScriptStatus;
use std::{collections::HashMap, future::Future, path::Path, pin::Pin, sync::OnceLock};
use tauri::AppHandle;
pub type InitFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;
pub struct Hooks {
    pub validate_choice:
        fn(&mut super::events::ScriptChannels, &str, Option<&str>) -> Result<(), String>,
    pub check_config: fn(&Path, &str) -> anyhow::Result<()>,
    pub initialize: for<'a, 'b> fn(
        &'a ScriptStatus,
        &'a mut super::events::ScriptContext<'b>,
    ) -> InitFuture<'a>,
    pub register_events: fn(),
    pub validate_config:
        fn(&Path, &Path, &serde_json::Value, &mut Vec<super::validate::Diagnostic>),
    pub validate_event: fn(&str, &mut super::validate::ExtensionEventContext<'_, '_>),
    pub begin: fn(&AppHandle) -> u64,
    pub close: fn(&AppHandle),
    pub restore_title: fn(&AppHandle),
    pub schemas: fn() -> Vec<super::schema::EventSpec>,
    pub prepare: fn(&ScriptStatus, &Path, bool) -> ScriptStatus,
    pub persist: fn(&ScriptStatus, &Path) -> anyhow::Result<()>,
    pub entry_error: fn(
        &Path,
        &str,
        &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<Option<String>>,
    pub recover: fn(&Path, &HashMap<String, ScriptStatus>),
}
static HOOKS: OnceLock<Hooks> = OnceLock::new();
pub fn install(hooks: Hooks) {
    assert!(HOOKS.set(hooks).is_ok(), "Script hooks already installed");
}
pub fn validate_choice(
    channels: &mut super::events::ScriptChannels,
    choice: &str,
    request: Option<&str>,
) -> Result<(), String> {
    if let Some(h) = HOOKS.get() {
        (h.validate_choice)(channels, choice, request)
    } else if request.is_some() {
        Err("当前未注册扩展选择事件".into())
    } else {
        Ok(())
    }
}
pub fn check_config(path: &Path, engine: &str) -> anyhow::Result<()> {
    HOOKS
        .get()
        .map_or(Ok(()), |h| (h.check_config)(path, engine))
}
pub async fn initialize(
    script: &ScriptStatus,
    ctx: &mut super::events::ScriptContext<'_>,
) -> anyhow::Result<()> {
    if let Some(h) = HOOKS.get() {
        (h.initialize)(script, ctx).await
    } else {
        Ok(())
    }
}
pub fn register_events() {
    if let Some(h) = HOOKS.get() {
        (h.register_events)();
    }
}
pub fn begin(app: &AppHandle) -> u64 {
    HOOKS.get().map_or(0, |h| (h.begin)(app))
}
pub fn close(app: &AppHandle) {
    if let Some(h) = HOOKS.get() {
        (h.close)(app);
    }
}
pub fn prepare(script: &ScriptStatus, data: &Path, preview: bool) -> ScriptStatus {
    HOOKS
        .get()
        .map_or_else(|| script.clone(), |h| (h.prepare)(script, data, preview))
}
pub fn persist(script: &ScriptStatus, data: &Path) -> anyhow::Result<()> {
    HOOKS.get().map_or(Ok(()), |h| (h.persist)(script, data))
}
pub fn entry_error(
    data: &Path,
    key: &str,
    settings: &serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<Option<String>> {
    HOOKS
        .get()
        .map_or(Ok(None), |h| (h.entry_error)(data, key, settings))
}
pub fn recover(data: &Path, scripts: &HashMap<String, ScriptStatus>) {
    if let Some(h) = HOOKS.get() {
        (h.recover)(data, scripts);
    }
}

pub fn restore_title(app: &AppHandle) {
    if let Some(h) = HOOKS.get() {
        (h.restore_title)(app);
    }
}
pub fn schemas() -> Vec<super::schema::EventSpec> {
    HOOKS.get().map_or_else(Vec::new, |h| (h.schemas)())
}

pub fn validate_config(
    data: &Path,
    script: &Path,
    config: &serde_json::Value,
    diags: &mut Vec<super::validate::Diagnostic>,
) {
    if let Some(h) = HOOKS.get() {
        (h.validate_config)(data, script, config, diags);
    }
}
pub fn validate_event(kind: &str, ctx: &mut super::validate::ExtensionEventContext<'_, '_>) {
    if let Some(h) = HOOKS.get() {
        (h.validate_event)(kind, ctx);
    }
}
