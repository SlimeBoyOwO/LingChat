//! 通用流式 HTTP 下载工具，支持进度回调、取消令牌、原子写入、断点续传、多线程分片。
//!
//! 调用方只需关注 URL、目标路径和进度回调；超时、UA、重定向等通用细节
//! 由本模块统一处理。TTS 下载、LAN 同步、角色包下载都复用此模块。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
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
/// 支持断点续传：如果 `.part` 文件已存在，尝试从断点继续下载（Range 请求）；
/// 服务器不支持 Range 时回退到重新下载。
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

    // 断点续传：检查已下载的 .part 文件大小
    let resume_from = if tmp.exists() {
        match tokio::fs::metadata(&tmp).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        }
    } else {
        0
    };

    let mut req = client.get(url).header(reqwest::header::ACCEPT, "*/*");
    if resume_from > 0 {
        // 请求从断点继续
        req = req.header(
            reqwest::header::RANGE,
            format!("bytes={resume_from}-"),
        );
    }
    let resp = req.send().await.map_err(|e| format!("request: {e}"))?;

    let status = resp.status();
    let supports_resume = status == reqwest::StatusCode::PARTIAL_CONTENT;
    let is_fresh = status.is_success() && !supports_resume;

    if !status.is_success() && !supports_resume {
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

    let total = if supports_resume {
        // 206 响应：从 Content-Range 头解析总大小（格式：bytes start-end/total）
        resp.headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cr| cr.split('/').nth(1))
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or_else(|| resume_from + resp.content_length().unwrap_or(0))
    } else {
        resp.content_length().unwrap_or(expected_size)
    };
    let declared_len = if supports_resume { Some(total) } else { resp.content_length() };

    let mut stream = resp.bytes_stream();

    // 续传：追加到现有文件；否则创建新文件
    let mut file = if supports_resume && resume_from > 0 {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&tmp)
            .await
            .map_err(|e| format!("open tmp for append: {e}"))?
    } else {
        // 服务器不支持 Range 或没有断点：从头下载
        tokio::fs::File::create(&tmp)
            .await
            .map_err(|e| format!("create tmp: {e}"))?
    };

    let mut bytes_done = if supports_resume { resume_from } else { 0 };
    let mut last_emit = Instant::now();
    let mut last_emitted_bytes: u64 = bytes_done;

    while let Some(chunk) = stream.next().await {
        // 取消检查
        if let Some(ref token) = cancel {
            if token.is_cancelled() {
                // 续传模式下不删除 .part 文件，保留断点供下次续传
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

    // Content-Length 声明了但实际字节数不足 → 连接提前中断（截断文件），必须报错而不是静默返回
    if let Some(declared) = declared_len {
        if bytes_done < declared {
            // 续传模式下不删除 .part 文件，保留断点供下次续传
            return Err(format!(
                "下载不完整（{bytes_done}/{declared} 字节，连接中断）"
            ));
        }
    }

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
/// - 600 秒总超时、8 秒连接超时（连接挂起快速失败，多源场景下及时换源；大文件下载不受影响）
/// - 最多 10 次重定向
/// - 标准 User-Agent
/// - TLS 用 webpki-roots（见 [`crate::utils::tls::build_tls_config`]），
///   绕开 rustls-platform-verifier 在 Android 上的 TLS panic
pub fn build_download_client() -> Result<reqwest::Client, String> {
    let tls_config = crate::utils::tls::build_tls_config()?;
    reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .connect_timeout(Duration::from_secs(8))
        .user_agent("LingChat/0.4.6")
        .redirect(reqwest::redirect::Policy::limited(10))
        .tls_backend_preconfigured(tls_config)
        .build()
        .map_err(|e| format!("build http client: {e}"))
}

/// 多线程分片下载：将文件分成 `num_chunks` 个片段并行下载，合并到 `dest`。
///
/// 仅当服务器支持 Range 请求时使用多线程；否则回退到单线程 `download_to_file`。
/// 每个分片独立写临时文件（`.part.N`），全部完成后按顺序合并。
///
/// # 参数
/// - `num_chunks`：分片数（推荐 4-8）
/// - 其余参数同 `download_to_file`
pub async fn download_to_file_parallel(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: Option<Arc<CancellationToken>>,
    progress: Option<Arc<dyn Fn(DownloadProgress) + Send + Sync>>,
    expected_size: u64,
    num_chunks: usize,
) -> Result<u64, String> {
    // 先探测服务器是否支持 Range
    let head_resp = client
        .head(url)
        .send()
        .await
        .map_err(|e| format!("head request: {e}"))?;

    let total_size = head_resp.content_length().unwrap_or(expected_size);
    let accepts_ranges = head_resp
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(|v| v != "none")
        .unwrap_or(false);

    // 不支持 Range 或文件太小：回退到单线程
    if !accepts_ranges || total_size < 1024 * 1024 || num_chunks <= 1 {
        return download_to_file(client, url, dest, cancel, progress, expected_size).await;
    }

    let num_chunks = num_chunks.min(8); // 最多 8 片
    let chunk_size = (total_size + num_chunks as u64 - 1) / num_chunks as u64;
    let tmp = dest.with_extension("part");

    // 确保目标目录存在
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;
    }

    // 共享的原子计数器，用于跨分片累计已下载字节
    let shared_done = Arc::new(AtomicU64::new(0));

    // 并行下载各分片
    let mut handles = Vec::with_capacity(num_chunks);
    for i in 0..num_chunks {
        let start = i as u64 * chunk_size;
        let end = ((i as u64 + 1) * chunk_size - 1).min(total_size - 1);
        if start > end {
            break;
        }
        let chunk_path = tmp.with_extension(format!("part.{i}"));
        let url = url.to_string();
        let client = client.clone();
        let cancel = cancel.clone();
        let progress = progress.clone();
        let shared_done = shared_done.clone();

        handles.push(tokio::spawn(async move {
            download_chunk(
                &client,
                &url,
                &chunk_path,
                start,
                end,
                total_size,
                cancel,
                progress,
                shared_done,
            )
            .await
        }));
    }

    // 等待所有分片完成
    for h in handles {
        match h.await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(format!("分片线程异常: {e}")),
        }
    }
    let total_done = shared_done.load(Ordering::Relaxed);

    // 合并分片
    let mut out = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("create merged: {e}"))?;
    for i in 0..num_chunks {
        let chunk_path = tmp.with_extension(format!("part.{i}"));
        if !chunk_path.exists() {
            break;
        }
        let mut chunk_file = tokio::fs::File::open(&chunk_path)
            .await
            .map_err(|e| format!("open chunk {i}: {e}"))?;
        tokio::io::copy(&mut chunk_file, &mut out)
            .await
            .map_err(|e| format!("copy chunk {i}: {e}"))?;
        let _ = tokio::fs::remove_file(&chunk_path).await;
    }
    tokio::io::AsyncWriteExt::shutdown(&mut out)
        .await
        .map_err(|e| format!("shutdown merged: {e}"))?;

    tokio::fs::rename(&tmp, dest)
        .await
        .map_err(|e| format!("rename: {e}"))?;

    if let Some(ref cb) = progress {
        cb(DownloadProgress::finished(total_done));
    }

    Ok(total_done)
}

/// 下载一个分片（Range 请求），写入 `chunk_path`。
/// `shared_done` 为跨分片共享的原子计数器，用于整体进度回调。
#[allow(clippy::too_many_arguments)]
async fn download_chunk(
    client: &reqwest::Client,
    url: &str,
    chunk_path: &Path,
    _start: u64,
    end: u64,
    total_size: u64,
    cancel: Option<Arc<CancellationToken>>,
    progress: Option<Arc<dyn Fn(DownloadProgress) + Send + Sync>>,
    shared_done: Arc<AtomicU64>,
) -> Result<u64, String> {
    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "*/*")
        .header(reqwest::header::RANGE, format!("bytes={}-{}", _start, end))
        .send()
        .await
        .map_err(|e| format!("chunk request: {e}"))?;

    if !resp.status().is_success() && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!("分片 HTTP {}", resp.status().as_u16()));
    }

    if let Some(parent) = chunk_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;
    }

    let mut file = tokio::fs::File::create(chunk_path)
        .await
        .map_err(|e| format!("create chunk: {e}"))?;
    let mut stream = resp.bytes_stream();
    let mut bytes_done: u64 = 0;
    let chunk_total = end - _start + 1;
    let mut last_emit = Instant::now();
    let mut last_reported_total: u64 = 0;

    while let Some(chunk) = stream.next().await {
        if let Some(ref token) = cancel {
            if token.is_cancelled() {
                return Err("download cancelled".into());
            }
        }
        let chunk = chunk.map_err(|e| format!("chunk: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("write: {e}"))?;
        bytes_done += chunk.len() as u64;

        // 累加到共享计数器，获取累加后的总值
        let overall_done = shared_done.fetch_add(chunk.len() as u64, Ordering::Relaxed) + chunk.len() as u64;

        let now = Instant::now();
        if progress_update_due(
            now.duration_since(last_emit),
            overall_done.saturating_sub(last_reported_total),
        ) {
            if let Some(ref cb) = progress {
                cb(DownloadProgress::new(overall_done, total_size));
            }
            last_emit = now;
            last_reported_total = overall_done;
        }
    }

    tokio::io::AsyncWriteExt::shutdown(&mut file)
        .await
        .map_err(|e| format!("shutdown: {e}"))?;

    if bytes_done < chunk_total {
        return Err(format!("分片不完整（{bytes_done}/{chunk_total}）"));
    }

    Ok(bytes_done)
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
