//! 市场 API：拉取 `plugins.json` 索引、下载 zip、sha256 校验、安装/卸载。
//!
//! 索引来自市场仓库 `zhangzm0/lingchat-marketplace`（§7.1 分发流）。
//! 安装记录集中存放在 `data/plugins/market.json`，用于已装列表与卸载。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
#[cfg(desktop)]
use tauri::Manager;

use crate::init::static_copy;
use crate::plugins::installer;
#[cfg(desktop)]
use crate::AppState;

/// 市场仓库 plugins.json（main 分支，raw 直连）。
const MARKET_INDEX_URL: &str =
    "https://raw.githubusercontent.com/zhangzm0/lingchat-marketplace/main/plugins.json";

/// 安装记录文件（data/plugins/market.json）。
const MARKET_RECORD_FILE: &str = "market.json";

/// 索引内存缓存（5 分钟 TTL），避免重复拉取。
static INDEX_CACHE: Mutex<Option<(Vec<MarketPackage>, std::time::Instant)>> =
    Mutex::new(None);
const INDEX_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);

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
    pub download_url: String,
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

/// 拉取 plugins.json 索引（5 分钟缓存）。
async fn fetch_index() -> Result<Vec<MarketPackage>, String> {
    if let Ok(cache) = INDEX_CACHE.lock() {
        if let Some((ref data, ref ts)) = *cache {
            if ts.elapsed() < INDEX_CACHE_TTL {
                return Ok(data.clone());
            }
        }
    }
    let client = build_client()?;
    let resp = client
        .get(MARKET_INDEX_URL)
        .send()
        .await
        .map_err(|e| format!("拉取市场索引失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "市场索引返回 HTTP {}",
            resp.status().as_u16()
        ));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析市场索引失败: {e}"))?;
    let plugins: Vec<MarketPackage> = serde_json::from_value(
        json.get("plugins").cloned().unwrap_or_default(),
    )
    .map_err(|e| format!("市场索引格式错误: {e}"))?;
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        *cache = Some((plugins.clone(), std::time::Instant::now()));
    }
    Ok(plugins)
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
    let progress: Option<Arc<dyn Fn(crate::utils::download::DownloadProgress) + Send + Sync>> =
        Some(Arc::new(move |p| {
            let _ = app_for_progress.emit(
                "market:progress",
                serde_json::json!({
                    "id": id,
                    "phase": "download",
                    "percent": p.percent,
                    "bytes": p.bytes_done,
                }),
            );
        }));
    let client = build_client()?;
    let expected = pkg.size.unwrap_or(0);
    let _ = std::fs::remove_file(&zip_path);
    crate::utils::download::download_to_file(
        &client,
        &pkg.download_url,
        &zip_path,
        None,
        progress,
        expected,
    )
    .await
    .map_err(|e| format!("下载失败: {e}"))?;

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

    // 解包安装（同步阻塞，放 spawn_blocking）
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
    }

    let _ = app.emit(
        "market:progress",
        serde_json::json!({ "id": id, "phase": "done", "percent": 100 }),
    );
    Ok(())
}

/// 卸载市场包：删除目标目录并移除安装记录；插件类注销工具。
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
        _ => {
            let dir = PathBuf::from(&record.dir);
            if dir.exists() {
                std::fs::remove_dir_all(&dir)
                    .map_err(|e| format!("删除目录失败: {e}"))?;
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
    Ok(())
}
