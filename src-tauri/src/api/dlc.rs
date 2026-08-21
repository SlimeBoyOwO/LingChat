//! DLC（即插即用的剧本包）管理命令。
//!
//! DLC 就是一个 zip 格式的剧本包：内含一个带 `story_config.yaml` 的剧本目录
//! （目录可以在 zip 根，也可以包一层同名文件夹）。导入 = 校验结构后解压到
//! `data/game_data/scripts/standalone/` 并立刻注册进 ScriptManager，无需重启；
//! 卸载 = 从 ScriptManager 摘除并删除目录。只有带 `dlc.json` 标记的目录能被
//! 卸载——内置剧本目录没有这个标记，不会被误删。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::utils::script_paths::sanitize_folder_name;
use crate::AppState;

/// DLC 清单文件（`dlc.json`），随剧本包分发；导入时缺省会补写一份。
/// 全字段保留序列化：补写 imported_at 时不能丢掉作者自带的 name/min_engine 等元数据。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DlcManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    /// 需要的最低游戏版本（仅展示用提示，不做强制拦截）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    #[serde(default)]
    imported_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DlcInfo {
    /// 目录名（standalone/<folder_key>）
    pub folder_key: String,
    /// story_config 的 script_name
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_warning: Option<String>,
    pub version: String,
    pub author: String,
    pub imported_at: String,
}

fn standalone_root() -> PathBuf {
    crate::init::static_copy::get_data_dir()
        .join("game_data")
        .join("scripts")
        .join("standalone")
}

fn read_manifest(dir: &Path) -> DlcManifest {
    fs::read_to_string(dir.join("dlc.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn dlc_info_of(dir: &Path) -> Option<DlcInfo> {
    // 只认带 dlc.json 标记的目录——这是「通过 DLC 包安装」与「内置剧本」的分界线
    if !dir.join("dlc.json").is_file() {
        return None;
    }
    let status = ScriptManager::read_script_config(dir).ok()?;
    let manifest = read_manifest(dir);
    Some(DlcInfo {
        folder_key: status.folder_key.clone(),
        name: status.name.clone(),
        description: status.description.clone(),
        content_warning: status.content_warning.clone(),
        version: manifest.version,
        author: manifest.author,
        imported_at: manifest.imported_at,
    })
}

#[tauri::command]
pub async fn list_dlcs(app: AppHandle) -> Result<Vec<DlcInfo>, String> {
    let _state = app.state::<AppState>();
    let root = standalone_root();
    let mut out: Vec<DlcInfo> = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(info) = dlc_info_of(&path) {
                    out.push(info);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[tauri::command]
pub async fn import_dlc(app: AppHandle, zip_path: String) -> Result<DlcInfo, String> {
    let state = app.state::<AppState>();

    // 剧本运行中禁止改动剧本集合
    {
        let service = state.ai_service.lock().await;
        if service
            .script_manager
            .is_running
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err("剧本正在运行，请先退出再安装 DLC".to_string());
        }
    }

    let zip_file =
        fs::File::open(&zip_path).map_err(|e| format!("无法打开 DLC 包 '{}': {}", zip_path, e))?;
    let mut archive =
        zip::ZipArchive::new(zip_file).map_err(|e| format!("DLC 包不是有效的 zip: {}", e))?;

    // ---- 定位剧本根：story_config.yaml 在 zip 根 → 平铺包；在唯一的顶级目录下 → 带壳包 ----
    let mut names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 DLC 包条目失败: {}", e))?;
        if let Some(enclosed) = entry.enclosed_name() {
            names.push(enclosed.to_string_lossy().replace('\\', "/"));
        } else {
            return Err("DLC 包含可疑路径条目，已拒绝安装".to_string());
        }
    }
    let root_prefix = detect_script_root(&names)?;

    // ---- 目标目录名：带壳包用壳目录名，平铺包用 zip 文件名 ----
    let folder_name = if root_prefix.is_empty() {
        Path::new(&zip_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        root_prefix.trim_end_matches('/').to_string()
    };
    let folder_name = sanitize_folder_name(&folder_name).map_err(|e| format!("DLC 目录名非法: {}", e))?;

    let target = standalone_root().join(&folder_name);
    if target.exists() {
        return Err(format!("已存在同名剧本目录 '{}'，请先卸载旧版", folder_name));
    }

    // ---- 解压（剥掉根前缀；enclosed_name 已做过 zip-slip 防护）----
    fs::create_dir_all(&target).map_err(|e| format!("创建 DLC 目录失败: {}", e))?;
    let extract_result = (|| -> Result<(), String> {
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("读取 DLC 包条目失败: {}", e))?;
            let enclosed = entry.enclosed_name().ok_or("DLC 包含可疑路径条目")?;
            let rel_str = enclosed.to_string_lossy().replace('\\', "/");
            let rel = if root_prefix.is_empty() {
                rel_str.clone()
            } else {
                match rel_str.strip_prefix(&root_prefix) {
                    Some(r) => r.to_string(),
                    None => continue, // 壳外的零散文件不装
                }
            };
            if rel.is_empty() {
                continue;
            }
            let out_path = target.join(&rel);
            if entry.is_dir() {
                fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut out_file = fs::File::create(&out_path).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })();

    if let Err(e) = extract_result {
        let _ = fs::remove_dir_all(&target);
        return Err(format!("解压 DLC 包失败: {}", e));
    }

    // ---- 结构校验：story_config.yaml 必须能解析 ----
    if let Err(e) = ScriptManager::read_script_config(&target) {
        let _ = fs::remove_dir_all(&target);
        return Err(format!("DLC 包缺少有效的 story_config.yaml: {:#}", e));
    }

    // ---- 补写/补全 dlc.json 标记（作者自带的字段全保留，只补 imported_at）----
    let manifest_path = target.join("dlc.json");
    let mut manifest = read_manifest(&target);
    if manifest.imported_at.is_empty() {
        manifest.imported_at = chrono::Local::now().to_rfc3339();
    }
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, json).map_err(|e| format!("写入 dlc.json 失败: {}", e))?;

    // ---- 立刻注册进引擎 ----
    {
        let mut service = state.ai_service.lock().await;
        service
            .script_manager
            .load_script_dir(&target)
            .map_err(|e| format!("DLC 注册失败: {:#}", e))?;
    }

    dlc_info_of(&target).ok_or_else(|| "DLC 安装后读取信息失败".to_string())
}

#[tauri::command]
pub async fn remove_dlc(app: AppHandle, folder_key: String) -> Result<(), String> {
    let state = app.state::<AppState>();

    {
        let service = state.ai_service.lock().await;
        if service
            .script_manager
            .is_running
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err("剧本正在运行，请先退出再卸载 DLC".to_string());
        }
    }

    let folder_name =
        sanitize_folder_name(&folder_key).map_err(|e| format!("DLC 目录名非法: {}", e))?;
    let target = standalone_root().join(&folder_name);
    if !target.is_dir() {
        return Err(format!("DLC 不存在: '{}'", folder_key));
    }
    // 只允许卸载带 dlc.json 标记的目录，内置剧本走不到这里
    if !target.join("dlc.json").is_file() {
        return Err("该剧本不是通过 DLC 包安装的，不能在此卸载".to_string());
    }

    {
        let mut service = state.ai_service.lock().await;
        service.script_manager.unload_script_dir(&target);
    }
    fs::remove_dir_all(&target).map_err(|e| format!("删除 DLC 目录失败: {}", e))?;
    Ok(())
}

/// 在 zip 条目名列表里定位 story_config.yaml 的位置。
/// 返回根前缀："" = 平铺包（配置在 zip 根），"<壳目录>/" = 带壳包。
fn detect_script_root(names: &[String]) -> Result<String, String> {
    const CONFIG: &str = "story_config.yaml";
    if names.iter().any(|n| n == CONFIG) {
        return Ok(String::new());
    }
    let mut roots: Vec<String> = names
        .iter()
        .filter_map(|n| n.strip_suffix(CONFIG))
        .filter(|prefix| !prefix.is_empty() && prefix.matches('/').count() == 1)
        .map(|prefix| prefix.to_string())
        .collect();
    roots.sort();
    roots.dedup();
    match roots.len() {
        0 => Err("DLC 包里找不到 story_config.yaml，不是有效的剧本包".to_string()),
        1 => Ok(roots.remove(0)),
        _ => Err("DLC 包里有多个 story_config.yaml，无法识别主剧本".to_string()),
    }
}
