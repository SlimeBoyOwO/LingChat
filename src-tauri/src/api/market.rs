//! 市场 API：动态读取市场仓库索引（registry 目录 + manifest.toml + build.json，
//! 兜底 plugins.json）、下载 zip（GitHub Releases + 镜像加速）、sha256 校验、安装/卸载。
//!
//! 索引来自市场仓库 `zhangzm0/lingchat-marketplace`（§7.1 分发流）：
//! - 动态读取：GitHub trees API 列 registry 目录（实时，镜像优先）→ 并发拉每包
//!   `registry/<id>/manifest.toml` 与 `registry/<id>/build.json`（发布 CI 回写的
//!   sha256/size/下载地址/审核链接）。加新包/新类型只需往仓库 registry/ 加目录，
//!   客户端零改动——为多类型铺路。jsDelivr data API 对 @main 目录树缓存可达一年，
//!   仅作兜底。
//! - 兜底：老格式 `plugins.json`（多 CDN 多源），兼容未重建 build.json 的旧包。
//! - 镜像加速：ghproxy 类代理（raw / Releases 下载）优先，直连靠后。
//! 安装记录集中存放在 `data/plugins/market.json`，用于已装列表与卸载。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::managers::role_repo::RoleRepo;
use crate::init::static_copy;
use crate::plugins::installer;
use crate::AppState;

/// 市场仓库 owner/repo（用于推导下载地址）。
const MARKET_REPO: &str = "zhangzm0/lingchat-marketplace";

/// 每包元数据基址（ghproxy 代理 raw 镜像优先，直连 raw / jsDelivr 官方多 CDN 靠后，依次尝试）。
/// `{base}registry/<dir>/<file>` 即元数据 URL。
const MARKET_META_BASES: &[&str] = &[
    "https://gh-proxy.com/https://raw.githubusercontent.com/zhangzm0/lingchat-marketplace/main/",
    "https://raw.githubusercontent.com/zhangzm0/lingchat-marketplace/main/",
    "https://cdn.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/",
    "https://fastly.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/",
    "https://gcore.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/",
];

/// 仓库目录清单源（GitHub trees API 实时优先；jsDelivr data API 对 @main 目录树缓存可达一年，
/// 新包上架后可能长期不刷新，故降为兜底）。镜像在前、直连在后，依次尝试。
const MARKET_TREE_URLS: &[&str] = &[
    "https://gh-proxy.com/https://api.github.com/repos/zhangzm0/lingchat-marketplace/git/trees/main?recursive=1",
    "https://api.github.com/repos/zhangzm0/lingchat-marketplace/git/trees/main?recursive=1",
    "https://gh-proxy.com/https://data.jsdelivr.com/v1/packages/gh/zhangzm0/lingchat-marketplace@main",
    "https://data.jsdelivr.com/v1/packages/gh/zhangzm0/lingchat-marketplace@main",
];

/// 兜底索引 plugins.json（ghproxy 镜像优先，直连 raw / 多 CDN 靠后）。
const MARKET_INDEX_URLS: &[&str] = &[
    "https://gh-proxy.com/https://raw.githubusercontent.com/zhangzm0/lingchat-marketplace/main/plugins.json",
    "https://raw.githubusercontent.com/zhangzm0/lingchat-marketplace/main/plugins.json",
    "https://cdn.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/plugins.json",
    "https://fastly.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/plugins.json",
    "https://gcore.jsdelivr.net/gh/zhangzm0/lingchat-marketplace@main/plugins.json",
];

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
    let mut futures: Vec<_> = urls.iter().map(|u| get_with_retry(client, u)).collect();
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
}

/// 每包构建产物（registry/<id>/build.json，发布 CI 回写）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildInfo {
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub review_report_url: Option<String>,
}

/// 已安装记录（market.json 条目）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRecord {
    pub id: String,
    pub version: String,
    #[serde(rename = "type")]
    pub package_type: String,
    /// 安装目标目录。
    pub dir: String,
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

/// 拉取市场索引：动态读取（registry 目录 + manifest.toml/build.json）优先，
/// 失败回退老格式 plugins.json；两级缓存（内存 5 分钟 + 磁盘 10 分钟）。
async fn fetch_index() -> Result<Vec<MarketPackage>, String> {
    if let Ok(cache) = INDEX_CACHE.lock() {
        if let Some((ref data, ref ts)) = *cache {
            if ts.elapsed() < INDEX_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }
    // 磁盘缓存（跨启动复用，减少动态读取的 N+1 请求）
    if let Some(plugins) = read_disk_cache() {
        if let Ok(mut cache) = INDEX_CACHE.lock() {
            *cache = Some((plugins.clone(), std::time::Instant::now()));
        }
        return Ok(plugins);
    }

    let client = build_client()?;
    let plugins = match fetch_index_dynamic(&client).await {
        Ok(pkgs) => pkgs,
        Err(dyn_err) => {
            tracing::warn!("市场动态索引失败，回退 plugins.json: {dyn_err}");
            fetch_index_static(&client).await?
        }
    };

    if let Ok(mut cache) = INDEX_CACHE.lock() {
        *cache = Some((plugins.clone(), std::time::Instant::now()));
    }
    write_disk_cache(&plugins);
    Ok(plugins)
}

/// 动态读取：列 registry 目录 → 并发拉每包 manifest.toml + build.json。
async fn fetch_index_dynamic(
    client: &reqwest::Client,
) -> Result<Vec<MarketPackage>, String> {
    let dirs = fetch_registry_tree(client).await?;
    if dirs.is_empty() {
        return Err("registry 目录为空".to_string());
    }
    // 部分包拉取失败不整体失败：跳过坏包，拿到多少显示多少
    //（例如某包 manifest 恰好解析失败 / 网络抖动，不应让整个市场列表消失）。
    let dir_names: Vec<String> = dirs.clone();
    let results = join_all(dirs.into_iter().map(|dir| fetch_pkg(client, dir))).await;
    let mut pkgs: Vec<MarketPackage> = Vec::new();
    let mut failed = 0usize;
    for (dir, res) in dir_names.into_iter().zip(results) {
        match res {
            Ok(pkg) => pkgs.push(pkg),
            Err(e) => {
                failed += 1;
                tracing::warn!("市场动态索引跳过包 '{}': {e}", dir);
            }
        }
    }
    if pkgs.is_empty() {
        return Err(format!(
            "市场索引拉取失败（{} 个目录全部不可读）",
            failed
        ));
    }
    if failed > 0 {
        tracing::warn!(
            "市场动态索引: 成功 {} 个，跳过 {} 个",
            pkgs.len(),
            failed
        );
    }
    tracing::info!("市场动态索引: 拉取 {} 个包", pkgs.len());
    Ok(pkgs)
}

/// 列出 registry 下所有包目录名（GitHub trees 实时优先 → jsDelivr data API 兜底，镜像在前）。
/// 并行抢答：所有源同时发起，第一个「成功且非空」的结果胜出——不被慢源/挂掉的镜像阻塞。
async fn fetch_registry_tree(client: &reqwest::Client) -> Result<Vec<String>, String> {
    use futures_util::future::select_all;
    let mut futures: Vec<_> = MARKET_TREE_URLS
        .iter()
        .map(|url| async move {
            let url = *url;
            let resp = get_with_retry(client, url).await?;
            parse_registry_tree(resp, url).await
        })
        .collect();
    let mut last_err = String::new();
    while !futures.is_empty() {
        let (res, _idx, rest) = select_all(futures).await;
        futures = rest;
        match res {
            Ok(dirs) if !dirs.is_empty() => return Ok(dirs),
            Ok(_) => last_err = "某源返回空目录".to_string(),
            Err(e) => last_err = e,
        }
    }
    Err(format!("列目录失败: {last_err}"))
}

/// 解析目录树响应 → 包目录名列表（兼容 jsDelivr data API 与 GitHub trees 两种格式）。
async fn parse_registry_tree(
    resp: reqwest::Response,
    url: &str,
) -> Result<Vec<String>, String> {
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读目录树失败: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("解析目录树失败: {e}"))?;
    let mut dirs = std::collections::BTreeSet::new();
    if url.contains("data.jsdelivr.com") {
        collect_jsdelivr_files(&json, "", &mut dirs);
    } else {
        // GitHub trees: tree[].path
        if let Some(tree) = json.get("tree").and_then(|t| t.as_array()) {
            for node in tree {
                if let Some(path) = node.get("path").and_then(|p| p.as_str()) {
                    if let Some(dir) = registry_dir_of_path(path) {
                        dirs.insert(dir.to_string());
                    }
                }
            }
        }
    }
    Ok(dirs.into_iter().collect())
}

/// jsDelivr data API 的 files 是嵌套结构，递归收集 registry/<dir>/manifest.toml。
fn collect_jsdelivr_files(
    node: &serde_json::Value,
    prefix: &str,
    out: &mut std::collections::BTreeSet<String>,
) {
    let files = match node.get("files").and_then(|f| f.as_array()) {
        Some(f) => f,
        None => return,
    };
    for f in files {
        let name = match f.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => continue,
        };
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        if let Some(dir) = registry_dir_of_path(&path) {
            out.insert(dir.to_string());
        }
        if f.get("files").is_some() {
            collect_jsdelivr_files(f, &path, out);
        }
    }
}

/// 从路径提取 registry 下的包目录名（形如 registry/<dir>/manifest.toml）。
fn registry_dir_of_path(path: &str) -> Option<&str> {
    let mut parts = path.split('/');
    if parts.next() != Some("registry") {
        return None;
    }
    let dir = parts.next()?;
    if parts.next() == Some("manifest.toml") {
        Some(dir)
    } else {
        None
    }
}

/// 拉取单个包的 manifest.toml + build.json，组装 MarketPackage。
async fn fetch_pkg(client: &reqwest::Client, dir: String) -> Result<MarketPackage, String> {
    // manifest.toml：多 CDN/raw 源链
    let manifest_urls: Vec<String> = MARKET_META_BASES
        .iter()
        .map(|base| format!("{base}registry/{dir}/manifest.toml"))
        .collect();
    let resp = fetch_first(client, &manifest_urls).await?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读 {dir} manifest.toml 失败: {e}"))?;
    let toml_val: toml::Value = toml::from_str(&text)
        .map_err(|e| format!("{dir} manifest.toml 解析失败: {e}"))?;

    // build.json：CI 回写的构建产物（sha256/size/下载地址/审核链接），缺失则降级
    let build_urls: Vec<String> = MARKET_META_BASES
        .iter()
        .map(|base| format!("{base}registry/{dir}/build.json"))
        .collect();
    let build: Option<BuildInfo> = match fetch_first(client, &build_urls).await {
        Ok(resp) => resp
            .json()
            .await
            .map_err(|e| format!("{dir} build.json 解析失败: {e}"))
            .ok(),
        Err(_) => None,
    };

    let manifest_json: serde_json::Value =
        serde_json::to_value(&toml_val).unwrap_or(serde_json::Value::Null);

    let get_str = |v: &toml::Value, key: &str| -> Option<String> {
        v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
    };

    let id = get_str(&toml_val, "id").unwrap_or_else(|| dir.clone());
    let version = get_str(&toml_val, "version").unwrap_or_default();
    // 下载地址：build.json 优先；缺失时按 Release 命名规则推导
    // （releases/download/<dir>-<version>/<dir>-<version>.zip）
    let download_url = build
        .as_ref()
        .and_then(|b| b.download_url.clone())
        .or_else(|| {
            Some(format!(
                "https://github.com/{MARKET_REPO}/releases/download/{dir}-{version}/{dir}-{version}.zip"
            ))
        });

    Ok(MarketPackage {
        id,
        name: get_str(&toml_val, "name").unwrap_or_else(|| dir.clone()),
        package_type: get_str(&toml_val, "type").unwrap_or_else(|| "content".to_string()),
        version,
        author: get_str(&toml_val, "author"),
        description: get_str(&toml_val, "description"),
        download_url,
        sha256: build.as_ref().and_then(|b| b.sha256.clone()),
        size: build.as_ref().and_then(|b| b.size),
        manifest: Some(manifest_json),
        review_report_url: build.as_ref().and_then(|b| b.review_report_url.clone()),
    })
}

/// 兜底：老格式 plugins.json（多 CDN/raw 源链）。
async fn fetch_index_static(client: &reqwest::Client) -> Result<Vec<MarketPackage>, String> {
    let urls: Vec<String> = MARKET_INDEX_URLS.iter().map(|u| u.to_string()).collect();
    let resp = fetch_first(client, &urls).await?;
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析市场索引失败: {e}"))?;
    serde_json::from_value(json.get("plugins").cloned().unwrap_or_default())
        .map_err(|e| format!("市场索引格式错误: {e}"))
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

/// 下载并安装市场包。
///
/// 流程：索引查条目 → 下载 zip（带进度事件）→ sha256 校验 → 解包安装
/// → 写安装记录 → 插件类 reload 注册工具。
#[tauri::command]
pub async fn market_install(app: AppHandle, id: String) -> Result<(), String> {
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

    let mut last_err = String::new();
    let mut downloaded = false;
    for (src_idx, src) in sources.iter().enumerate() {
        let mut src_err = String::new();
        // 每源最多 2 次（连接超时 8s，不可达镜像很快失败并换下一源，避免长时间卡住）
        for attempt in 0..2 {
            let _ = std::fs::remove_file(&zip_path);
            let progress = progress.clone();
            match crate::utils::download::download_to_file(
                &client,
                src,
                &zip_path,
                None,
                progress,
                expected,
            )
            .await
            {
                Ok(_) => {
                    downloaded = true;
                    break;
                }
                Err(e) => {
                    src_err = e;
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
        if downloaded {
            break;
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

    // sha256 校验（fail-closed：索引声明了就必须匹配）
    if let Some(declared) = &pkg.sha256 {
        let actual = installer::sha256_hex(&zip_path)?;
        if !actual.eq_ignore_ascii_case(declared) {
            let _ = std::fs::remove_file(&zip_path);
            return Err(format!(
                "sha256 校验失败（{id} {ver}）：声明 {declared}，实际 {actual}",
                ver = pkg.version
            ));
        }
    }

    // 解包安装（同步阻塞，放 spawn_blocking）；先通知进入安装阶段
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
                version: pkg.version.clone(),
                package_type: pkg.package_type.clone(),
                dir: installed.dir.display().to_string(),
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
