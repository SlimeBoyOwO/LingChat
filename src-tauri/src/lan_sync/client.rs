//! LAN 同步 HTTP 客户端。
//!
//! 封装对等设备的 HTTP API 调用，供 sync_engine 使用。
//! 使用模块级 `LazyLock<Client>` 共享连接池，支持 TCP Keep-Alive。

use std::path::Path;
use std::sync::LazyLock;

use reqwest::Client;
use tracing::info;

use super::messages::{CompleteManifest, PeerInfo};

/// 全模块共享的 HTTP 客户端（连接池复用 + Keep-Alive）。
static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(std::time::Duration::from_secs(300)) // 5 分钟总超时（大文件）
        .connect_timeout(std::time::Duration::from_secs(10))
        .no_proxy() // 局域网同步不走系统代理
        .pool_max_idle_per_host(4) // 每对端最多 4 个空闲连接
        .build()
        .expect("reqwest 客户端构建失败")
});

/// 获取对端的完整文件清单。
pub async fn fetch_remote_manifest(peer: &PeerInfo) -> Result<CompleteManifest, String> {
    let url = format!("http://{}:{}/manifest", peer.host, peer.port);
    info!("请求对端清单: {}", url);

    let client = &*HTTP_CLIENT;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求清单失败 [{}:{}]: {}", peer.host, peer.port, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "对端返回错误 [{}:{}]: {}",
            peer.host,
            peer.port,
            response.status()
        ));
    }

    let manifest: CompleteManifest = response
        .json()
        .await
        .map_err(|e| format!("解析清单失败 [{}:{}]: {}", peer.host, peer.port, e))?;

    info!(
        "获取清单成功: {} 个清单文件 + {} 个运行时文件",
        manifest.files.len(),
        manifest.runtime_files.len()
    );
    Ok(manifest)
}

/// 快速健康检查：确认对端的 HTTP 服务可达。
pub async fn check_peer_health(peer: &PeerInfo) -> Result<(), String> {
    let url = format!("http://{}:{}/health", peer.host, peer.port);
    let client = &*HTTP_CLIENT;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("对端健康检查失败 [{}:{}]: {}", peer.host, peer.port, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "对端返回异常状态 [{}:{}]: {}",
            peer.host,
            peer.port,
            response.status()
        ));
    }
    Ok(())
}

/// 从对端下载单个文件到本地目标路径。
///
/// 使用流式下载，边下边写，避免大文件撑爆内存。
/// 先写入 `.tmp` 文件，下载完成后 rename 到最终路径。
pub async fn download_file(
    peer: &PeerInfo,
    remote_path: &str,
    dest_path: &Path,
) -> Result<(), String> {
    let url = format!(
        "http://{}:{}/file?path={}",
        peer.host,
        peer.port,
        urlencoding(remote_path)
    );
    let client = &*HTTP_CLIENT;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求文件失败 [{}]: {e}", remote_path))?;

    if !response.status().is_success() {
        return Err(format!(
            "对端返回错误 [{}]: {}",
            remote_path,
            response.status()
        ));
    }

    // 确保目标目录存在
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败 [{}]: {}", remote_path, e))?;
    }

    // 写入 .tmp 文件
    let tmp_path = dest_path.with_extension(format!(
        "{}.tmp",
        dest_path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default()
    ));

    let mut dest = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("创建文件失败 [{}]: {}", remote_path, e))?;

    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("接收数据失败 [{}]: {}", remote_path, e))?;
        tokio::io::AsyncWriteExt::write_all(&mut dest, &chunk)
            .await
            .map_err(|e| format!("写入文件失败 [{}]: {}", remote_path, e))?;
    }

    // 原子重命名
    std::fs::rename(&tmp_path, dest_path)
        .map_err(|e| format!("重命名文件失败 [{}]: {}", remote_path, e))?;

    info!("已下载: {} -> {:?}", remote_path, dest_path);
    Ok(())
}

/// 向对端推送单个文件。
pub async fn upload_file(
    peer: &PeerInfo,
    local_path: &Path,
    remote_path: &str,
) -> Result<(), String> {
    let url = format!(
        "http://{}:{}/push-file?path={}",
        peer.host,
        peer.port,
        urlencoding(remote_path)
    );
    let client = &*HTTP_CLIENT;

    let data = std::fs::read(local_path)
        .map_err(|e| format!("读取本地文件失败 [{}]: {}", remote_path, e))?;

    let response = client
        .post(&url)
        .body(data)
        .send()
        .await
        .map_err(|e| format!("推送文件失败 [{}]: {e}", remote_path))?;

    if !response.status().is_success() {
        return Err(format!(
            "对端拒绝文件 [{}]: {}",
            remote_path,
            response.status()
        ));
    }

    info!("已推送: {:?} -> {}", local_path, remote_path);
    Ok(())
}

/// 向对端发送删除指令。
pub async fn push_delete(peer: &PeerInfo, remote_path: &str) -> Result<(), String> {
    let url = format!(
        "http://{}:{}/push-delete?path={}",
        peer.host,
        peer.port,
        urlencoding(remote_path)
    );
    let client = &*HTTP_CLIENT;

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("发送删除指令失败 [{}]: {e}", remote_path))?;

    if !response.status().is_success() {
        return Err(format!(
            "对端拒绝删除 [{}]: {}",
            remote_path,
            response.status()
        ));
    }

    Ok(())
}

/// URL 编码（仅编码路径中需要编码的字符，保留 `/` 作为路径分隔符）。
fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}
