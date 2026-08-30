//! 插件携带资源（人物/剧本/音乐/背景图/环境音）的扫描与冲突处理。
//!
//! 资源目录与游戏同名同构（`plugins/<id>/{characters,scripts,musics,backgrounds,ambients}/`），
//! 运行时直读、不复制；玩家「保留」时才复制到游戏目录并自动隐藏插件版。
//! 冲突规则：与游戏同名冲突 → 游戏优先（插件条目隐藏）；插件间同名 → 先注册者（id 序）赢。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::types::{PluginRecord, ResourceKind};

pub const IMAGE_EXTENSIONS: [&str; 8] =
    ["png", "jpg", "jpeg", "webp", "bmp", "svg", "tif", "gif"];
pub const AUDIO_EXTENSIONS: [&str; 7] =
    ["mp3", "wav", "flac", "webm", "weba", "ogg", "oga"];

/// 单条插件资源条目（前端资源管理列表 & 各内容列表合并共用）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PluginResourceEntry {
    pub kind: ResourceKind,
    /// 定位 key：角色 = folder 名；剧本 = script_name；图/音 = 文件名（含扩展名）。
    pub key: String,
    /// 显示名：角色 = settings.yml title；剧本 = script_name；图/音 = 文件 stem。
    pub name: String,
    /// 资源绝对路径（目录或文件）。
    pub path: PathBuf,
    pub plugin_id: String,
    /// 与游戏现有资源同名冲突（列表中被游戏版压制）。
    pub conflict: bool,
    /// 已被玩家软删除隐藏。
    pub hidden: bool,
}

impl PluginResourceEntry {
    /// 软删除标记值：`"<kind>/<key>"`。
    pub fn hidden_mark(&self) -> String {
        format!("{}/{}", self.kind.subdir(), self.key)
    }
}

/// 拆分隐藏标记为 (kind, key)。
pub fn split_hidden_mark(mark: &str) -> Option<(ResourceKind, &str)> {
    let (prefix, key) = mark.split_once('/')?;
    let kind = match prefix {
        "characters" => ResourceKind::Characters,
        "scripts" => ResourceKind::Scripts,
        "musics" => ResourceKind::Musics,
        "backgrounds" => ResourceKind::Backgrounds,
        "ambients" => ResourceKind::Ambients,
        _ => return None,
    };
    Some((kind, key))
}

/// 扫描单个插件的某类资源目录。
pub fn scan_kind(record: &PluginRecord, kind: ResourceKind) -> Vec<PluginResourceEntry> {
    let root = record.dir.join(kind.subdir());
    if !root.is_dir() {
        return Vec::new();
    }
    let hidden: HashSet<&str> = record
        .state
        .hidden_resources
        .iter()
        .filter_map(|m| split_hidden_mark(m))
        .filter(|(k, _)| *k == kind)
        .map(|(_, key)| key)
        .collect();

    let entries = match kind {
        ResourceKind::Characters => scan_character_packages(&root),
        ResourceKind::Scripts => scan_scripts(&root),
        ResourceKind::Musics | ResourceKind::Ambients => scan_media_files(&root, kind, true),
        ResourceKind::Backgrounds => scan_media_files(&root, kind, false),
    };
    entries
        .into_iter()
        .map(|mut e| {
            e.plugin_id = record.manifest.id.clone();
            e.hidden = hidden.contains(e.key.as_str());
            e
        })
        .collect()
}

/// 角色：子目录含 settings.yml 即一个角色；key = folder 名，name = title。
fn scan_character_packages(root: &Path) -> Vec<PluginResourceEntry> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().to_string();
        if folder == "avatar" || folder.starts_with('.') {
            continue;
        }
        let settings = path.join("settings.yml");
        if !settings.exists() {
            continue;
        }
        let name = load_character_title(&settings).unwrap_or_else(|| folder.clone());
        out.push(PluginResourceEntry {
            kind: ResourceKind::Characters,
            key: folder.clone(),
            name,
            path,
            plugin_id: String::new(),
            conflict: false,
            hidden: false,
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

fn load_character_title(path: &Path) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct TitleOnly {
        title: Option<String>,
    }
    let content = fs::read_to_string(path).ok()?;
    let parsed: TitleOnly = serde_yaml::from_str(&content).ok()?;
    parsed.title.filter(|s| !s.is_empty())
}

/// 图/音：按扩展名白名单收集文件；key = 文件名（含扩展名）。
fn scan_media_files(root: &Path, kind: ResourceKind, audio: bool) -> Vec<PluginResourceEntry> {
    let allowed: &[&str] = if audio {
        &AUDIO_EXTENSIONS
    } else {
        &IMAGE_EXTENSIONS
    };
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !allowed.contains(&ext.to_lowercase().as_str()) {
            continue;
        }
        let Some(file_name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(PluginResourceEntry {
            kind,
            key: file_name,
            name,
            path,
            plugin_id: String::new(),
            conflict: false,
            hidden: false,
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// 剧本：与 ScriptManager 相同的三种布局扫描 story_config.yaml。
fn scan_scripts(root: &Path) -> Vec<PluginResourceEntry> {
    let mut out = Vec::new();
    for dir in script_package_dirs(root) {
        match crate::ai_service::game_system::script_engine::ScriptManager::read_script_config(
            &dir,
        ) {
            Ok(status) => out.push(PluginResourceEntry {
                kind: ResourceKind::Scripts,
                key: status.name.clone(),
                name: status.name,
                path: dir,
                plugin_id: String::new(),
                conflict: false,
                hidden: false,
            }),
            Err(e) => {
                tracing::warn!("[PluginResources] 跳过无效剧本目录 {:?}: {}", dir, e);
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// 枚举 scripts 根目录下三种布局的剧本包目录。
pub fn script_package_dirs(scripts_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(level1) = fs::read_dir(scripts_root) else {
        return out;
    };
    for entry in level1.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        match name.as_str() {
            "character" => {
                if let Ok(roles) = fs::read_dir(&path) {
                    for role in roles.flatten() {
                        if !role.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            continue;
                        }
                        if let Ok(scripts) = fs::read_dir(role.path()) {
                            for s in scripts.flatten() {
                                if s.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                    out.push(s.path());
                                }
                            }
                        }
                    }
                }
            }
            "standalone" => {
                if let Ok(scripts) = fs::read_dir(&path) {
                    for s in scripts.flatten() {
                        if s.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            out.push(s.path());
                        }
                    }
                }
            }
            _ => {
                if path.join("story_config.yaml").exists() {
                    out.push(path);
                }
            }
        }
    }
    out
}

/// 游戏自有剧本的 script_name 集合（用于插件剧本冲突判定）。
pub fn game_script_names(data_dir: &Path) -> HashSet<String> {
    let scripts_dir = data_dir.join("game_data").join("scripts");
    let mut names = HashSet::new();
    for dir in script_package_dirs(&scripts_dir) {
        if let Ok(status) =
            crate::ai_service::game_system::script_engine::ScriptManager::read_script_config(&dir)
        {
            names.insert(status.name);
        }
    }
    names
}

/// 目录里游戏自有资源的文件名集合（背景/音乐/环境音冲突判定）。
pub fn game_file_names(dir: &Path, audio: bool) -> HashSet<String> {
    let allowed: &[&str] = if audio {
        &AUDIO_EXTENSIONS
    } else {
        &IMAGE_EXTENSIONS
    };
    let mut out = HashSet::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !allowed.contains(&ext.to_lowercase().as_str()) {
            continue;
        }
        if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) {
            out.insert(name);
        }
    }
    out
}

/// 递归复制目录（保留结构）。
pub fn copy_dir_all(source: &Path, target: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(target)
        .map_err(|e| anyhow::anyhow!("创建目录 {:?} 失败: {e}", target))?;
    for entry in fs::read_dir(source)?.flatten() {
        let dest = target.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest).map_err(|e| {
                anyhow::anyhow!("复制 {:?} 失败: {e}", entry.path())
            })?;
        }
    }
    Ok(())
}
