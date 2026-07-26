// Streaming downloader for curated assets. Emits progress events and
// honors a shared cancellation token.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use super::registry::AssetEntry;

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub asset_id: String,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub percent: f32,
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
    let mut last_pct: i32 = -1;
    let mut hasher = Sha256::new();

    while let Some(chunk) = stream.next().await {
        if cancel.is_cancelled() {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err("download cancelled".into());
        }
        let chunk = chunk.map_err(|e| format!("chunk: {e}"))?;
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("write: {e}"))?;
        bytes_done += chunk.len() as u64;
        let pct = if total > 0 {
            ((bytes_done * 100) / total) as i32
        } else {
            0
        };
        if pct != last_pct {
            last_pct = pct;
            let _ = app.emit(
                "tts://download-progress",
                DownloadProgress {
                    asset_id: entry.id.clone(),
                    bytes_done,
                    total_bytes: total,
                    percent: pct as f32,
                },
            );
        }
    }
    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("shutdown: {e}"))?;
    tokio::fs::rename(&tmp, dst)
        .await
        .map_err(|e| format!("rename: {e}"))?;

    if !entry.sha256.chars().all(|c| c == '0') {
        let got = format!("{:x}", hasher.finalize());
        if !got.eq_ignore_ascii_case(&entry.sha256) {
            let _ = tokio::fs::remove_file(dst).await;
            return Err(format!("SHA256 mismatch for {}", entry.id));
        }
    }
    Ok(bytes_done)
}

#[allow(dead_code)] // helper for future callers; resolve_target owns the active flow
pub fn final_path_for(entry: &AssetEntry, base: &Path) -> PathBuf {
    let ext = super::registry::expected_extension(entry);
    base.join(format!("{}.{ext}", entry.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_path_uses_extension() {
        let e = super::super::registry::find("ling-v2").unwrap();
        let p = final_path_for(&e, Path::new("/tmp/cache"));
        assert_eq!(p, PathBuf::from("/tmp/cache/ling-v2.onnx"));
    }

    #[test]
    fn final_path_uses_extension_for_style_vectors() {
        let e = super::super::registry::find("ling-v2-style").unwrap();
        let p = final_path_for(&e, Path::new("/tmp/cache"));
        assert_eq!(p, PathBuf::from("/tmp/cache/ling-v2-style.json"));
    }
}
