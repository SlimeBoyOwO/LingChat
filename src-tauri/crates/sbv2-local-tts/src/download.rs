// Streaming downloader for curated assets. Emits progress events and
// honors a shared cancellation token.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use super::registry::AssetEntry;

const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(200);
const PROGRESS_EMIT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub asset_id: String,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

fn progress_update_due(elapsed: Duration, bytes_since_last: u64) -> bool {
    elapsed >= PROGRESS_EMIT_INTERVAL || bytes_since_last >= PROGRESS_EMIT_BYTES
}

fn emit_download_progress(
    app: &AppHandle,
    entry: &AssetEntry,
    bytes_done: u64,
    total_bytes: u64,
    complete: bool,
) {
    let percent = if complete {
        100.0
    } else if total_bytes > 0 {
        (bytes_done as f64 * 100.0 / total_bytes as f64).min(100.0) as f32
    } else {
        0.0
    };
    let _ = app.emit(
        "tts://download-progress",
        DownloadProgress {
            asset_id: entry.id.clone(),
            bytes_done,
            total_bytes,
            percent,
        },
    );
}

pub async fn download_asset(
    app: &AppHandle,
    entry: &AssetEntry,
    dst: &Path,
    cancel: Arc<CancellationToken>,
) -> std::result::Result<u64, String> {
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;
    }
    let tmp = dst.with_extension("part");
    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("LingChat/0.4.6")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("client: {e}"))?
        .get(&entry.download_url)
        .header(reqwest::header::ACCEPT, "*/*")
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let final_url = resp.url().to_string();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable response body>".into());
        let body = body.trim();
        let body = if body.len() > 512 {
            format!("{}...", &body[..512])
        } else {
            body.to_string()
        };
        return Err(format!(
            "HTTP {status} for {} at {final_url}: {body}",
            entry.id
        ));
    }
    let total = resp.content_length().unwrap_or(entry.size_bytes);
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("create tmp: {e}"))?;
    let mut bytes_done: u64 = 0;
    let mut last_emit = Instant::now();
    let mut last_emitted_bytes = 0;

    while let Some(chunk) = stream.next().await {
        if cancel.is_cancelled() {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err("download cancelled".into());
        }
        let chunk = chunk.map_err(|e| format!("chunk: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("write: {e}"))?;
        bytes_done += chunk.len() as u64;
        let now = Instant::now();
        if progress_update_due(
            now.duration_since(last_emit),
            bytes_done.saturating_sub(last_emitted_bytes),
        ) {
            emit_download_progress(app, entry, bytes_done, total, false);
            last_emit = now;
            last_emitted_bytes = bytes_done;
        }
    }
    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("shutdown: {e}"))?;
    tokio::fs::rename(&tmp, dst)
        .await
        .map_err(|e| format!("rename: {e}"))?;

    emit_download_progress(app, entry, bytes_done, total, true);

    Ok(bytes_done)
}

#[cfg(test)]
mod tests {
    use super::{progress_update_due, PROGRESS_EMIT_BYTES, PROGRESS_EMIT_INTERVAL};
    use std::time::Duration;

    #[test]
    fn progress_is_due_after_time_threshold() {
        assert!(progress_update_due(PROGRESS_EMIT_INTERVAL, 0));
        assert!(!progress_update_due(Duration::from_millis(199), 0));
    }

    #[test]
    fn progress_is_due_after_byte_threshold() {
        assert!(progress_update_due(Duration::ZERO, PROGRESS_EMIT_BYTES));
        assert!(!progress_update_due(
            Duration::ZERO,
            PROGRESS_EMIT_BYTES - 1
        ));
    }
}
