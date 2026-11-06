//! Lifecycle callbacks wired by the application to its DLC runtime.
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::OnceLock,
};
use tauri::AppHandle;
pub type CloseFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
pub struct Hooks {
    pub script_directories: fn(&Path) -> Vec<PathBuf>,
    pub close_plugin: fn(AppHandle, String) -> CloseFuture,
    pub close_all: fn(AppHandle) -> CloseFuture,
    pub force_close_plugin: fn(&AppHandle, &str),
    pub force_close_all: fn(&AppHandle),
}
static HOOKS: OnceLock<Hooks> = OnceLock::new();
pub fn install(hooks: Hooks) {
    assert!(HOOKS.set(hooks).is_ok(), "DLC runtime already installed");
}
pub fn hooks() -> anyhow::Result<&'static Hooks> {
    HOOKS
        .get()
        .ok_or_else(|| anyhow::anyhow!("DLC runtime unavailable"))
}
pub async fn request_close_plugin(app: &AppHandle, id: &str) -> Result<(), String> {
    if let Some(h) = HOOKS.get() {
        (h.close_plugin)(app.clone(), id.to_owned()).await
    } else {
        Ok(())
    }
}
pub async fn request_close_all(app: &AppHandle) -> Result<(), String> {
    if let Some(h) = HOOKS.get() {
        (h.close_all)(app.clone()).await
    } else {
        Ok(())
    }
}
pub fn close_plugin(app: &AppHandle, id: &str) {
    if let Some(h) = HOOKS.get() {
        (h.force_close_plugin)(app, id);
    }
}
pub fn close_all(app: &AppHandle) {
    if let Some(h) = HOOKS.get() {
        (h.force_close_all)(app);
    }
}
