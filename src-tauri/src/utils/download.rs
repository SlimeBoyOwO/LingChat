//! 通用流式 HTTP 下载工具，支持进度回调、取消令牌、原子写入。
//!
//! 调用方只需关注 URL、目标路径和进度回调；超时、UA、重定向等通用细节
//! 由本模块统一处理。TTS 下载、LAN 同步、角色包下载都复用此模块。

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

/// 下载进度快照，由 `download_to_file` 通过回调推送。
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// 已下载字节数
    pub bytes_done: u64,
    /// 总字节数（来自 Content-Length 或参数传入的估算值）
    pub total_bytes: u64,
    /// 百分比 0.0–100.0
    pub percent: f32,
}

impl DownloadProgress {
    fn new(bytes_done: u64, total_bytes: u64) -> Self {
        let percent = if total_bytes > 0 {
            (bytes_done as f64 * 100.0 / total_bytes as f64).min(100.0) as f32
        } else {
            0.0
        };
        Self { bytes_done, total_bytes, percent }
    }

    fn finished(total_bytes: u64) -> Self {
        Self { bytes_done: total_bytes, total_bytes, percent: 100.0 }
    }
}

/// 进度回调节流常量：200ms 或 1MB，避免高频事件淹没前端。
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(200);
const PROGRESS_EMIT_BYTES: u64 = 1024 * 1024;

fn progress_update_due(elapsed: Duration, bytes_since_last: u64) -> bool {
    elapsed >= PROGRESS_EMIT_INTERVAL || bytes_since_last >= PROGRESS_EMIT_BYTES
}

/// 流式下载文件到磁盘，写入 `.part` 临时文件后原子 rename 到 `dest`。
///
/// 使用 `Arc<dyn Fn + Send + Sync>` 持有进度回调，确保 future 为 `Send`
///（Tauri command 所需）。
///
/// # 参数
/// - `url`：下载地址
/// - `dest`：目标文件路径（不存在则自动创建父目录）
/// - `cancel`：可选的取消令牌，每块数据前检查
/// - `progress`：可选的进度回调，每 200ms 或 1MB 触发一次
/// - `client`：可复用的 `reqwest::Client`，避免每次下载都重建连接池
/// - `expected_size`：当服务器未返回 Content-Length 时使用的估算值
///
/// # 返回
/// 成功返回实际写入字节数；取消/IO/HTTP 错误返回 `Err(String)`。
pub async fn download_to_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: Option<Arc<CancellationToken>>,
    progress: Option<Arc<dyn Fn(DownloadProgress) + Send + Sync>>,
    expected_size: u64,
) -> Result<u64, String> {
    // 确保目标目录存在
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;
    }

    let tmp = dest.with_extension("part");

    let resp = client
        .get(url)
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
        let snippet = if body.len() > 512 {
            format!("{}...", &body[..512])
        } else {
            body.to_string()
        };
        return Err(format!("HTTP {status} from {final_url}: {snippet}"));
    }

    let total = resp.content_length().unwrap_or(expected_size);
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("create tmp: {e}"))?;

    let mut bytes_done: u64 = 0;
    let mut last_emit = Instant::now();
    let mut last_emitted_bytes: u64 = 0;

    while let Some(chunk) = stream.next().await {
        // 取消检查
        if let Some(ref token) = cancel {
            if token.is_cancelled() {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err("download cancelled".into());
            }
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
            if let Some(ref cb) = progress {
                cb(DownloadProgress::new(bytes_done, total));
            }
            last_emit = now;
            last_emitted_bytes = bytes_done;
        }
    }

    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("shutdown: {e}"))?;
    tokio::fs::rename(&tmp, dest)
        .await
        .map_err(|e| format!("rename: {e}"))?;

    // 完成回调
    if let Some(ref cb) = progress {
        cb(DownloadProgress::finished(bytes_done));
    }

    Ok(bytes_done)
}

/// 构建一个预配置的 `reqwest::Client`，适合通用下载场景。
///
/// - 600 秒超时
/// - 最多 10 次重定向
/// - 标准 User-Agent
pub fn build_download_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .user_agent("LingChat/0.4.6")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("build http client: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn progress_update_after_time_threshold() {
        assert!(progress_update_due(PROGRESS_EMIT_INTERVAL, 0));
        assert!(!progress_update_due(Duration::from_millis(199), 0));
    }

    #[test]
    fn progress_update_after_byte_threshold() {
        assert!(progress_update_due(Duration::ZERO, PROGRESS_EMIT_BYTES));
        assert!(!progress_update_due(Duration::ZERO, PROGRESS_EMIT_BYTES - 1));
    }
}
