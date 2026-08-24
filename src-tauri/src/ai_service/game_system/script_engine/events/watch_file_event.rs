//! Watch file event — DDLC Act 3's live `monika.chr` check, adapted safely.
//!
//! Starts a background watcher (2s poll) on a declared `.chr` story marker.
//! The moment the player deletes the file, the watcher records the target
//! chapter in `channels.watch_jump` and drops the pending input/choice
//! channels, so a blocking event fails fast and the chapter loop routes to the
//! corruption sequence at the next opportunity — regardless of what the player
//! was doing. Only markers declared in `script_settings.character_files` can be
//! watched; the path is resolved with the same anti-traversal rules as
//! `character_file`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    character_file_event, register_event, ScriptContext, ScriptEvent,
};

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

pub struct WatchFileEvent {
    action: String,
    file: String,
    on_missing: String,
}

impl WatchFileEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            action: data
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("start")
                .to_string(),
            file: data
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            on_missing: data
                .get("on_missing")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }
}

#[async_trait]
impl ScriptEvent for WatchFileEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let mut channels = ctx.channels.lock().await;

        // 任何 watch_file 事件都先停掉旧监视（换目标/停止/幂等重入都安全）
        if let Some(task) = channels.watch_task.take() {
            task.abort();
        }

        if self.action == "stop" {
            channels.watch_jump = None;
            return Ok(None);
        }

        if self.file.is_empty() || self.on_missing.is_empty() {
            return Err(anyhow!("watch_file 需要 file 与 on_missing"));
        }

        let (script_path_key, target) = {
            let gs = ctx.game_status.lock().await;
            let script = gs
                .script_status
                .as_ref()
                .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;
            let target = character_file_event::resolve_declared_target_path(
                script,
                ctx.data_dir,
                &self.file,
            )?;
            (script.path_key(), target)
        };

        // 文件已经不在：不用开监视，直接排队跳转
        if !target.exists() {
            channels.watch_jump = Some(self.on_missing.clone());
            channels.input_tx = None;
            channels.choice_tx = None;
            channels.choice_allow_free = false;
            return Ok(None);
        }

        let on_missing = self.on_missing.clone();
        let channels_arc = ctx.channels.clone();
        tracing::info!(
            "[WatchFileEvent] 开始监视 {}（{}）→ 消失跳 '{}'",
            self.file,
            script_path_key,
            on_missing
        );
        let task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(POLL_INTERVAL).await;
                if target.exists() {
                    continue;
                }
                let mut channels = channels_arc.lock().await;
                // 同一拍只触发一次
                if channels.watch_jump.is_none() {
                    tracing::info!("[WatchFileEvent] 监视目标消失，跳转到 '{}'", on_missing);
                    channels.watch_jump = Some(on_missing.clone());
                    // 丢弃挂起通道：阻塞中的 input/choices 事件立即收 Err 让位
                    channels.input_tx = None;
                    channels.choice_tx = None;
                    channels.choice_allow_free = false;
                }
                break;
            }
        });
        channels.watch_task = Some(task);
        Ok(None)
    }

    fn event_type() -> &'static str {
        "watch_file"
    }
}

pub fn register() {
    register_event(WatchFileEvent::event_type(), |data| {
        Box::new(WatchFileEvent::from_event_data(&data))
    });
}
