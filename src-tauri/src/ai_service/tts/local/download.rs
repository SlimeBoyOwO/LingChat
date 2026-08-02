//! TTS 资产下载器，包装 `crate::utils::download`，增加 Tauri 进度事件发射。

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use super::registry::AssetEntry;
use crate::utils::download::{self, DownloadProgress as CoreProgress};

/// TTS 专用的下载进度结构（包含 `asset_id` 供前端识别）。
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub asset_id: String,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

/// 懒加载的 HTTP 客户端，避免每次下载都重建连接池。
fn download_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| download::build_download_client().expect("build download client"))
}

/// 下载一个 TTS 资产到磁盘，向 Tauri 前端发射进度事件。
///
/// 核心下载逻辑委托给 `crate::utils::download::download_to_file`。
pub async fn download_asset(
    app: &AppHandle,
    entry: &AssetEntry,
    dst: &Path,
    cancel: Arc<CancellationToken>,
) -> Result<u64, String> {
    let client = download_client();
    let asset_id = entry.id.clone();

    // 通过 Arc 闭包将通用进度转为 Tauri 事件（Arc 保证 Send + Sync）
    let app_for_progress = app.clone();
    let entry_id = asset_id.clone();
    let on_progress: Arc<dyn Fn(CoreProgress) + Send + Sync> = Arc::new(move |p| {
        let _ = app_for_progress.emit(
            "tts://download-progress",
            DownloadProgress {
                asset_id: entry_id.clone(),
                bytes_done: p.bytes_done,
                total_bytes: p.total_bytes,
                percent: p.percent,
            },
        );
    });

    download::download_to_file(
        client,
        &entry.download_url,
        dst,
        Some(cancel),
        Some(on_progress),
        entry.size_bytes,
    )
    .await
}

#[cfg(test)]
mod tests {
    use crate::utils::download::build_download_client;

    #[test]
    fn download_client_is_reusable() {
        let client = build_download_client().expect("build download client");
        assert!(client
            .get("https://example.com")
            .build()
            .is_ok());
    }
}
