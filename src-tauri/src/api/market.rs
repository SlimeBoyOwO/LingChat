//! 市场 API：从 Release plugins.json 拉取市场索引（CDN/raw 兜底），
//! 下载 zip（GitHub Releases + 镜像加速）、sha256 校验、安装/卸载。
//!
//! 索引来自市场仓库 `zhangzm0/lingchat-marketplace`（§7.1 分发流）：
//! - 主源：Release 产物 `plugins.json`（永远实时，绕过 jsDelivr 缓存）。
//! - 兜底：多 CDN/raw 源链，ghproxy 镜像优先。
//! 安装记录集中存放在 `data/plugins/market.json`，用于已装列表与卸载。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::managers::role_repo::RoleRepo;
use crate::init::static_copy;
use crate::plugins::installer;
use crate::AppState;

/// 市场仓库 owner/repo（用于推导下载地址）。
const MARKET_REPO: &str = "zhangzm0/lingchat-marketplace";

/// 兜底索引 plugins.json（ghproxy 镜像优先，直连 raw / 多 CDN 靠后）。
const MARKET_INDEX_URLS: &[&str] = &[
    "https://gh-proxy.com/https://raw.githubusercontent.com/zhangzm0/lingchat-marketplace/main/plugins.json",
    "https://raw.githubusercontent.com/zhangzm0/lingchat-marketplace/main/plugins.json",
    "https://cdn.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/plugins.json",
    "https://fastly.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/plugins.json",
    "https://gcore.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/plugins.json",
];

/// plugins.json 的 GitHub Release 产物地址（永远实时，不走 jsDelivr 缓存）。
/// publish.yml 在每次发布后把 plugins.json 上传为 registry Release 的附件
///（固定标签，--clobber 覆盖），客户端 trees API 限流/失败时优先从这里拉。
const MARKET_RELEASE_PLUGINS_URL: &str =
    "https://github.com/zhangzm0/lingchat-marketplace/releases/download/registry/plugins.json";

/// GitHub Releases 下载镜像代理（下载时镜像优先、主源直连靠后；只转发不改内容，sha256 校验不受影响）。
const DOWNLOAD_MIRRORS: &[&str] = &[
    "https://gh-proxy.com",
    "https://ghfast.top",
    "https://ghproxy.net",
];

/// 安装记录文件（data/plugins/market.json）。
const MARKET_RECORD_FILE: &str = "market.json";

/// 索引磁盘缓存文件（data/plugins/ 下；动态读取是 N+1 次请求，落盘避免每次启动重拉）。
const INDEX_CACHE_FILE: &str = "market-index-cache.json";
/// 索引磁盘缓存 TTL（10 分钟）。
const INDEX_DISK_TTL: Duration = Duration::from_secs(600);

/// 索引内存缓存（5 分钟 TTL），避免重复拉取。
static INDEX_CACHE: Mutex<Option<(Vec<MarketPackage>, std::time::Instant)>> =
    Mutex::new(None);
const INDEX_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);

/// 重试参数：最多 `MAX_RETRIES` 次（含首次），退避 `BASE_DELAY * 2^attempt`。
const MAX_RETRIES: usize = 3;
const BASE_DELAY_MS: u64 = 500;

/// 带指数退避的 GET 请求，成功（2xx）即返回。
async fn get_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, String> {
    let mut last_err = String::new();
    for attempt in 0..MAX_RETRIES {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) => {
                last_err = format!("HTTP {}", resp.status().as_u16());
            }
            Err(e) => last_err = format!("网络错误: {e}"),
        }
        if attempt + 1 < MAX_RETRIES {
            tokio::time::sleep(std::time::Duration::from_millis(
                BASE_DELAY_MS * (1 << attempt),
            ))
            .await;
        }
    }
    Err(format!("GET {url} 重试 {MAX_RETRIES} 次均失败: {last_err}"))
}

/// 依次尝试多个 URL，第一个成功的响应返回（用于 CDN/镜像多源链）。
/// 并行抢答：所有源同时发起，谁先成功用谁——镜像通就走镜像，直连挂不影响；
/// 全部失败才返回错误。避免串行等待慢源（连接超时已缩短到 8s）。
async fn fetch_first(
    client: &reqwest::Client,
    urls: &[String],
) -> Result<reqwest::Response, String> {
    use futures_util::future::select_all;
    // Box::pin：select_all 需要 Future + Unpin
    let mut futures: Vec<_> = urls
        .iter()
        .map(|u| Box::pin(get_with_retry(client, u)))
        .collect();
    let mut last_err = String::new();
    while !futures.is_empty() {
        let (res, _idx, rest) = select_all(futures).await;
        futures = rest;
        match res {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                last_err = e;
                tracing::warn!("多源并行拉取中一个源失败（继续等其余源）: {last_err}");
            }
        }
    }
    Err(last_err)
}

/// plugins.json 条目（市场侧 schema，字段可能缺省，全部 default 兼容）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPackage {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub package_type: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// 下载地址（动态来源可能缺失，缺失时安装报错）。
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    /// 审核时快照的完整 manifest（展示用）。
    #[serde(default)]
    pub manifest: Option<serde_json::Value>,
    #[serde(default)]
    pub review_report_url: Option<String>,
    /// 已下架标记：为 true 时客户端从市场列表隐藏，已装用户保留。
    #[serde(default)]
    pub delisted: bool,
}

/// 已安装记录（market.json 条目）。
/// 安装时快照完整元数据，确保离线/云端加载失败时已安装列表仍有详情。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub package_type: String,
    /// 安装目标目录。
    pub dir: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    /// 审核时快照的完整 manifest（展示用）。
    #[serde(default)]
    pub manifest: Option<serde_json::Value>,
}

/// 进行中的安装任务状态（内存态，供前端切页/重挂载后恢复按钮与进度）。
/// 与已安装记录分开：只描述「此刻是否在装、装到哪一步」，不落盘。
#[derive(Debug, Clone, Serialize)]
pub struct InstallTask {
    pub id: String,
    /// download | install
    pub phase: String,
    /// 0–100
    pub percent: u8,
}

/// 进行中的安装任务集合（支持并行下载）。
/// 每个包 id 对应一个任务，多个包可同时下载。
static INSTALLING: Mutex<Option<HashMap<String, InstallTask>>> = Mutex::new(None);

/// 安装任务的取消令牌集合。每个包 id 对应一个 CancellationToken，
/// 前端取消时触发，下载阶段检查并提前退出。
static CANCELS: Mutex<Option<HashMap<String, tokio_util::sync::CancellationToken>>> =
    Mutex::new(None);

/// 每包互斥锁：防止同一包并发安装（取消后立即重触发的竞争）。
/// 新安装必须等旧任务完全结束并释放锁后才能开始。
static INSTALL_LOCKS: Mutex<Option<HashMap<String, Arc<tokio::sync::Mutex<()>>>> =
    Mutex::new(None);

/// 读取所有进行中的安装任务。用于前端恢复按钮状态。
fn installing_tasks() -> Vec<InstallTask> {
    INSTALLING
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .map(|m| m.into_values().collect())
        .unwrap_or_default()
}

/// 读取指定 id 的进行中的安装任务。
fn installing_task(id: &str) -> Option<InstallTask> {
    INSTALLING
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .and_then(|m| m.get(id).cloned())
}

/// 更新进行中任务的进度（下载/解包阶段回调）。
fn update_installing(id: &str, phase: &str, percent: u8) {
    if let Ok(mut g) = INSTALLING.lock() {
        let m = g.get_or_insert_with(HashMap::new);
        m.insert(
            id.to_string(),
            InstallTask {
                id: id.to_string(),
                phase: phase.to_string(),
                percent,
            },
        );
    }
}

/// 清除指定 id 的进行中任务（安装完成/失败/取消时调用）。
fn clear_installing(id: &str) {
    if let Ok(mut g) = INSTALLING.lock() {
        if let Some(m) = g.as_mut() {
            m.remove(id);
        }
    }
}

/// 安装开始时注册取消令牌。
fn register_cancel(id: &str) -> tokio_util::sync::CancellationToken {
    let token = tokio_util::sync::CancellationToken::new();
    if let Ok(mut g) = CANCELS.lock() {
        let m = g.get_or_insert_with(HashMap::new);
        m.insert(id.to_string(), token.clone());
    }
    token
}

/// 安装结束时注销取消令牌。
fn unregister_cancel(id: &str) {
    if let Ok(mut g) = CANCELS.lock() {
        if let Some(m) = g.as_mut() {
            m.remove(id);
        }
    }
}

/// 取消指定 id 的安装任务：触发取消令牌 + 从 INSTALLING 中移除。
/// 下载阶段会检查令牌并提前退出；安装阶段（spawn_blocking）不可中断，
/// 但移除 INSTALLING 后前端可重新触发安装。
fn cancel_install(id: &str) {
    // 触发取消令牌（下载阶段会检测并退出）
    if let Ok(mut g) = CANCELS.lock() {
        if let Some(m) = g.as_mut() {
            if let Some(token) = m.remove(id) {
                token.cancel();
            }
        }
    }
    // 从 INSTALLING 中移除，让前端可以重新触发
    clear_installing(id);
}

fn data_dir() -> PathBuf {
    static_copy::get_data_dir().clone()
}

fn plugins_root() -> PathBuf {
    data_dir().join("plugins")
}

fn record_path() -> PathBuf {
    plugins_root().join(MARKET_RECORD_FILE)
}

fn read_records() -> HashMap<String, InstalledRecord> {
    match std::fs::read_to_string(record_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn write_records(records: &HashMap<String, InstalledRecord>) -> Result<(), String> {
    let path = record_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    let text = serde_json::to_string_pretty(records)
        .map_err(|e| format!("序列化安装记录失败: {e}"))?;
    std::fs::write(&tmp, text).map_err(|e| format!("写入安装记录失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("保存安装记录失败: {e}"))
}

/// 构建市场 HTTP client（TLS webpki-roots，复用下载模块配置）。
fn build_client() -> Result<reqwest::Client, String> {
    crate::utils::download::build_download_client()
}

/// 索引磁盘缓存路径。
fn index_cache_path() -> PathBuf {
    plugins_root().join(INDEX_CACHE_FILE)
}

/// 读磁盘缓存（TTL 内按文件 mtime 判断）。
fn read_disk_cache() -> Option<Vec<MarketPackage>> {
    let path = index_cache_path();
    let meta = std::fs::metadata(&path).ok()?;
    let modified = meta.modified().ok()?;
    if std::time::SystemTime::now()
        .duration_since(modified)
        .map(|d| d > INDEX_DISK_TTL)
        .unwrap_or(true)
    {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    serde_json::from_value(json.get("plugins").cloned().unwrap_or_default()).ok()
}

/// 写磁盘缓存。
fn write_disk_cache(plugins: &[MarketPackage]) {
    let path = index_cache_path();
    let text = match serde_json::to_string_pretty(&serde_json::json!({ "plugins": plugins })) {
        Ok(t) => t,
        Err(_) => return,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// 拉取市场索引：Release plugins.json 优先，CDN/raw 兜底；
/// 两级缓存（内存 5 分钟 + 磁盘 10 分钟）。
async fn fetch_index() -> Result<Vec<MarketPackage>, String> {
    if let Ok(cache) = INDEX_CACHE.lock() {
        if let Some((ref data, ref ts)) = *cache {
            if ts.elapsed() < INDEX_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }
    if let Some(plugins) = read_disk_cache() {
        if let Ok(mut cache) = INDEX_CACHE.lock() {
            *cache = Some((plugins.clone(), std::time::Instant::now()));
        }
        return Ok(plugins);
    }

    let client = build_client()?;
    // 1) Release 产物 plugins.json（永远实时，绕过 jsDelivr 缓存）
    // 2) 失败则走 CDN/raw 多源兜底
    let plugins = match fetch_plugins_from_release(&client).await {
        Ok(pkgs) if !pkgs.is_empty() => pkgs,
        Ok(_) => {
            tracing::warn!("Release plugins.json 为空，回退 CDN/raw");
            fetch_index_static(&client).await?
        }
        Err(rel_err) => {
            tracing::warn!("Release plugins.json 失败: {rel_err}，回退 CDN/raw");
            fetch_index_static(&client).await?
        }
    };

    // 过滤已下架包：市场列表不展示，已装用户保留（由 installed 列表单独维护）
    let before = plugins.len();
    let mut plugins = plugins;
    plugins.retain(|p| !p.delisted);
    if plugins.len() < before {
        tracing::info!("市场索引: 过滤 {} 个已下架包", before - plugins.len());
    }

    if let Ok(mut cache) = INDEX_CACHE.lock() {
        *cache = Some((plugins.clone(), std::time::Instant::now()));
    }
    write_disk_cache(&plugins);
    Ok(plugins)
}

/// 从 GitHub Release 产物拉 plugins.json（永远实时，不走 jsDelivr 缓存）。
async fn fetch_plugins_from_release(
    client: &reqwest::Client,
) -> Result<Vec<MarketPackage>, String> {
    let resp = get_with_retry(client, MARKET_RELEASE_PLUGINS_URL).await?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取 Release plugins.json 响应失败: {e}"))?;
    parse_plugins_json(&text)
}

/// 把 plugins.json 文本解析成 MarketPackage 列表。
fn parse_plugins_json(text: &str) -> Result<Vec<MarketPackage>, String> {
    let json: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| format!("解析 plugins.json 失败: {e}"))?;
    serde_json::from_value(json.get("plugins").cloned().unwrap_or_default())
        .map_err(|e| format!("plugins.json 格式错误: {e}"))
}

/// 兜底：老格式 plugins.json（多 CDN/raw 源链）。
async fn fetch_index_static(client: &reqwest::Client) -> Result<Vec<MarketPackage>, String> {
    let urls: Vec<String> = MARKET_INDEX_URLS.iter().map(|u| u.to_string()).collect();
    let resp = fetch_first(client, &urls).await?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取市场索引响应失败: {e}"))?;
    parse_plugins_json(&text)
}

// ─── Tauri Commands ─────────────────────────────────────────────

/// 获取市场可安装包列表。
#[tauri::command]
pub async fn market_fetch_index() -> Result<Vec<MarketPackage>, String> {
    fetch_index().await
}

/// 已安装包列表（读 market.json）。
#[tauri::command]
pub async fn market_installed() -> Result<Vec<InstalledRecord>, String> {
    Ok(read_records().into_values().collect())
}

/// 获取所有进行中的安装任务（供前端恢复按钮/进度，无则空数组）。
/// 支持并行下载：每个包独立跟踪进度。
#[tauri::command]
pub async fn market_installing() -> Result<Vec<InstallTask>, String> {
    Ok(installing_tasks())
}

/// 下载并安装市场包。
///
/// 流程：索引查条目 → 下载 zip（带进度事件）→ sha256 校验 → 解包安装
/// → 写安装记录 → 插件类 reload 注册工具。
///
/// 支持并行下载：多个包可同时安装（各自写独立缓存路径，互不覆盖）；
/// 仅禁止同一包的重复安装。
#[tauri::command]
pub async fn market_install(app: AppHandle, id: String) -> Result<(), String> {
    // 防重入：只禁止同一包的重复安装（每个包写独立缓存路径，不同包可并行）。
    // 前端切页/重挂载后按钮状态由 market_installing 恢复，这里兜底拦截重复触发。
    if let Some(task) = installing_task(&id) {
        return Err(format!("包 '{id}' 正在安装中（{}%），请稍候", task.percent));
    }
    update_installing(&id, "download", 0);
    let cancel_token = register_cancel(&id);

    let result = market_install_inner(app, id.clone(), cancel_token.clone()).await;
    // 无论成功失败都清除进行中标记和取消令牌（cancel_install 可能已清除，幂等）
    clear_installing(&id);
    unregister_cancel(&id);
    result
}

/// 取消指定 id 的安装任务（前端点取消按钮时调用）。
#[tauri::command]
pub async fn market_cancel(id: String) -> Result<(), String> {
    cancel_install(&id);
    Ok(())
}

/// market_install 主体（下载 → 校验 → 解包 → 写记录 → 注册）。
async fn market_install_inner(
    app: AppHandle,
    id: String,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<(), String> {
    let pkg = fetch_index()
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("市场没有这个包: '{id}'"))?;

    // 下载到缓存目录
    let cache_dir = plugins_root().join(".cache");
    let zip_path = cache_dir.join(format!("{}-{}.zip", pkg.id, pkg.version));
    let app_for_progress = app.clone();
    let progress_id = id.clone();
    let progress: Option<Arc<dyn Fn(crate::utils::download::DownloadProgress) + Send + Sync>> =
        Some(Arc::new(move |p| {
            update_installing(&progress_id, "download", p.percent as u8);
            let _ = app_for_progress.emit(
                "market:progress",
                serde_json::json!({
                    "id": progress_id,
                    "phase": "download",
                    "percent": p.percent,
                    "bytes": p.bytes_done,
                }),
            );
        }));
    let client = build_client()?;
    let expected = pkg.size.unwrap_or(0);

    // 文件 >1MB 即使用多线程分片下载；小文件用单线程（分片开销不划算）
    const PARALLEL_THRESHOLD: u64 = 1024 * 1024;
    const PARALLEL_CHUNKS: usize = 8;
    let use_parallel = expected > PARALLEL_THRESHOLD;

    // 下载地址（动态索引下 build.json 缺失时已按 Release 规则推导，理论不会为空）
    let download_url = pkg
        .download_url
        .clone()
        .ok_or_else(|| format!("包 '{id}' 缺少下载地址"))?;

    // 多源下载链：镜像代理优先（只转发不改内容，sha256 校验不受影响），GitHub Releases 主源靠后。
    // 每源各自带指数退避重试，全部失败才报错。
    let mut sources: Vec<String> = Vec::with_capacity(DOWNLOAD_MIRRORS.len() + 1);
    for mirror in DOWNLOAD_MIRRORS {
        sources.push(format!("{mirror}/{download_url}"));
    }
    sources.push(download_url.clone());

    let cancel_arc = Arc::new(cancel);
    let mut last_err = String::new();
    let mut downloaded = false;
    'sources: for (src_idx, src) in sources.iter().enumerate() {
        let mut src_err = String::new();
        // 每源最多 2 次（连接超时 8s，不可达镜像很快失败并换下一源，避免长时间卡住）
        for attempt in 0..2 {
            let progress = progress.clone();
            let cancel = cancel_arc.clone();
            // 用 select! 让取消信号能立即中断下载（不用等当前 chunk 完成）
            let download_fut = if use_parallel {
                crate::utils::download::download_to_file_parallel(
                    &client,
                    src,
                    &zip_path,
                    Some(cancel.clone()),
                    progress,
                    expected,
                    PARALLEL_CHUNKS,
                )
            } else {
                crate::utils::download::download_to_file(
                    &client,
                    src,
                    &zip_path,
                    Some(cancel.clone()),
                    progress,
                    expected,
                )
            };
            let result = tokio::select! {
                _ = cancel.cancelled() => Err("download cancelled".into()),
                r = download_fut => r,
            };
            match result {
                Ok(bytes) => {
                    // 大小校验：下载字节数不足说明连接提前中断（截断文件），视为该源失败换下一源
                    if expected > 0 && bytes < expected {
                        src_err = format!("下载不完整（{bytes}/{expected} 字节）");
                        tracing::warn!("市场包 '{}' {}: {src_err}", id, src);
                        if attempt + 1 < 2 {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                BASE_DELAY_MS * (1 << attempt),
                            ))
                            .await;
                        }
                        continue;
                    }
                    // sha256 校验（fail-closed：索引声明了就必须匹配）——
                    // 校验失败直接报错，不换源重试（sha256 不匹配说明索引或源文件有误，重试无意义）
                    if let Some(declared) = &pkg.sha256 {
                        match installer::sha256_hex(&zip_path) {
                            Ok(actual) if actual.eq_ignore_ascii_case(declared) => {}
                            Ok(actual) => {
                                let _ = std::fs::remove_file(&zip_path);
                                return Err(format!(
                                    "sha256 校验失败（声明 {declared}，实际 {actual}）"
                                ));
                            }
                            Err(e) => {
                                let _ = std::fs::remove_file(&zip_path);
                                return Err(format!("sha256 计算失败: {e}"));
                            }
                        }
                    }
                    downloaded = true;
                    break 'sources;
                }
                Err(e) => {
                    src_err = e.clone();
                    // 取消是用户主动行为，不重试、不换源，立即退出
                    if e == "download cancelled" {
                        tracing::info!("市场包 '{}' 下载已取消，立即停止", id);
                        clear_installing(&id);
                        let _ = std::fs::remove_file(&zip_path);
                        // 清理并行下载的临时文件（.part 和 .part.N）
                        let part = zip_path.with_extension("part");
                        let _ = std::fs::remove_file(&part);
                        for i in 0..PARALLEL_CHUNKS {
                            let chunk = part.with_extension(format!("part.{i}"));
                            let _ = std::fs::remove_file(&chunk);
                        }
                        return Err("下载已取消".into());
                    }
                    tracing::warn!(
                        "市场包 '{}' 下载失败（源 {}，第 {} 次）: {src_err}",
                        id,
                        src_idx + 1,
                        attempt + 1
                    );
                    if attempt + 1 < 2 {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            BASE_DELAY_MS * (1 << attempt),
                        ))
                        .await;
                    }
                }
            }
        }
        last_err = format!("源 {} ({src}) 失败: {src_err}", src_idx + 1);
        // 换源前短暂停顿，避免对镜像站突发请求
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    if !downloaded {
        return Err(format!(
            "下载失败（{} 个镜像 + 主源均失败）: {last_err}",
            DOWNLOAD_MIRRORS.len()
        ));
    }

    // 下载完成后检查是否被取消（前端点取消按钮）
    if cancel_arc.is_cancelled() {
        let _ = std::fs::remove_file(&zip_path);
        clear_installing(&id);
        return Err("下载已取消".into());
    }

    // 下载成功（已通过大小 + sha256 校验），进入解包安装阶段

    // 安装阶段前再次检查取消（spawn_blocking 不可中断，提前跳过）
    if cancel_arc.is_cancelled() {
        let _ = std::fs::remove_file(&zip_path);
        clear_installing(&id);
        return Err("安装已取消".into());
    }

    // 解包安装（同步阻塞，放 spawn_blocking）；先通知进入安装阶段
    update_installing(&id, "install", 0);
    let _ = app.emit(
        "market:progress",
        serde_json::json!({ "id": id, "phase": "install", "percent": 0 }),
    );
    let data = data_dir();
    let root = plugins_root();
    let zip = zip_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        installer::install_package(&zip, &data, &root)
    })
    .await
    .map_err(|e| format!("安装线程异常: {e}"))?;
    let installed = result.map_err(|e| {
        let _ = std::fs::remove_file(&zip_path);
        e
    })?;
    let _ = std::fs::remove_file(&zip_path);

    // 写安装记录
    {
        let mut records = read_records();
        records.insert(
            pkg.id.clone(),
            InstalledRecord {
                id: pkg.id.clone(),
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                package_type: pkg.package_type.clone(),
                dir: installed.dir.display().to_string(),
                author: pkg.author.clone(),
                description: pkg.description.clone(),
                download_url: pkg.download_url.clone(),
                sha256: pkg.sha256.clone(),
                size: pkg.size,
                manifest: pkg.manifest.clone(),
            },
        );
        write_records(&records)?;
    }

    // 插件类重新扫描（注册工具）；内容类无需注册。
    // 移动端（Android/iOS）不编译插件系统（RustPython 依赖问题），
    // 插件包照常落盘 data/plugins/，但运行需桌面端。
    if installed.manifest.package_type == "plugin" {
        #[cfg(desktop)]
        {
            let manager = app.state::<AppState>().data().plugin_manager.clone();
            tokio::task::spawn_blocking(move || manager.reload())
                .await
                .map_err(|e| format!("插件重载线程异常: {e}"))?;
        }
        #[cfg(not(desktop))]
        tracing::info!(
            "移动端安装插件 '{}'：已落盘 data/plugins/，运行需桌面端",
            id
        );
    } else if installed.manifest.package_type == "character" {
        // 角色卡：get_character_list 读 DB，而角色行只在启动/手动「刷新角色列表」时
        // 由 rescan_roles 从目录同步。装完必须同步一次，否则角色列表
        // 直到重启或手动刷新都不出现（与设置页刷新按钮同一条路径）。
        if let Err(e) = crate::api::role_archive::rescan_roles(app.clone()).await {
            tracing::warn!("角色安装后重扫角色表失败: {e}");
        }
    } else if installed.manifest.package_type == "script" {
        // 剧本包：引擎启动时才扫一次剧本目录，装完必须重扫，
        // 否则主菜单剧本列表 / 羁绊冒险直到重启都不出现。
        if let Err(e) =
            crate::api::script_editor::commands::editor_rescan_scripts(app.clone()).await
        {
            tracing::warn!("剧本安装后重扫引擎失败（可能有剧本正在运行）: {e}");
        }
    }

    let _ = app.emit(
        "market:progress",
        serde_json::json!({ "id": id, "phase": "done", "percent": 100 }),
    );
    Ok(())
}

/// 卸载市场包：删除目标目录并移除安装记录；插件类注销工具，角色类走完整删除。
#[tauri::command]
pub async fn market_uninstall(app: AppHandle, id: String) -> Result<(), String> {
    let record = read_records()
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("包 '{id}' 未安装或非市场来源"))?;

    match record.package_type.as_str() {
        "plugin" => {
            #[cfg(desktop)]
            {
                let manager = app.state::<AppState>().data().plugin_manager.clone();
                manager.delete_plugin(&id).await?;
            }
            #[cfg(not(desktop))]
            {
                // 移动端没有 PluginManager：直接删目录（记录随后移除）
                let dir = PathBuf::from(&record.dir);
                if dir.exists() {
                    std::fs::remove_dir_all(&dir)
                        .map_err(|e| format!("删除目录失败: {e}"))?;
                }
            }
        }
        "character" => {
            // 复用设置页「删除角色」的完整卸载：DB 级联（存档/记忆/台词）+ 物理目录 + 广播。
            // 角色包 id 即角色目录名，rescan 后 DB 里会有一条 resource_folder == id 的 main 角色。
            let db = app.state::<AppState>().db.clone();
            let role = RoleRepo::get_main_role_by_resource_folder(&db, &id)
                .await
                .map_err(|e| format!("查询角色失败: {e}"))?;
            match role {
                Some(role) => {
                    // 完整删除（校验在场/类型，DB 级联，物理目录，role:list-updated）
                    crate::api::character::delete_main_role_core(&app, role.id, true).await?;
                }
                None => {
                    // 从未 rescan 入库（例如装完没刷新过角色列表）：退化为仅删目录
                    let dir = PathBuf::from(&record.dir);
                    if dir.exists() {
                        std::fs::remove_dir_all(&dir)
                            .map_err(|e| format!("删除目录失败: {e}"))?;
                    }
                }
            }
        }
        _ => {
            let dir = PathBuf::from(&record.dir);
            if dir.exists() {
                std::fs::remove_dir_all(&dir)
                    .map_err(|e| format!("删除目录失败: {e}"))?;
            }
            // 剧本包：引擎内存里还留着它（羁绊冒险/剧本列表读的是引擎内存），
            // 删目录后需重扫才能让它从主菜单剧本列表和羁绊冒险里消失。
            if record.package_type == "script" {
                if let Err(e) =
                    crate::api::script_editor::commands::editor_rescan_scripts(app.clone()).await
                {
                    tracing::warn!("剧本卸载后重扫引擎失败（可能有剧本正在运行）: {e}");
                }
            }
        }
    }

    let mut records = read_records();
    records.remove(&id);
    write_records(&records)?;
    Ok(())
}

/// 刷新索引缓存（强制下次重新拉取）。
#[tauri::command]
pub async fn market_clear_cache() -> Result<(), String> {
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        *cache = None;
    }
    let _ = std::fs::remove_file(index_cache_path());
    Ok(())
}
