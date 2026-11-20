//! 剧本文件读写：YAML ⇄ JSON、原子写、备份。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value as JsonValue};

/// 角色设定文件名（云端/官方版本，随 git 与安装包分发）。
pub const SETTINGS_FILE: &str = "settings.yml";

/// 开发者本地覆盖文件名。存在时读写一律走它，`settings.yml` 完全不参与。
pub const SETTINGS_LOCAL_FILE: &str = "settings_local.yml";

/// 解析角色目录下的设定文件：优先 `settings_local.yml`，否则 `settings.yml`。
pub fn resolve_settings_file(character_dir: &Path) -> PathBuf {
    let local = character_dir.join(SETTINGS_LOCAL_FILE);
    if local.is_file() {
        local
    } else {
        character_dir.join(SETTINGS_FILE)
    }
}

/// 读 YAML 文件并转成 JSON 值。
pub fn read_yaml_as_json(path: &Path) -> Result<JsonValue, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("无法读取 {:?}: {}（文件必须是 UTF-8）", path, e))?;

    let value: JsonValue =
        serde_yaml::from_str(&text).map_err(|e| format!("{:?} YAML 解析失败: {}", path, e))?;

    Ok(match value {
        JsonValue::Null => JsonValue::Object(Map::new()),
        other => other,
    })
}

/// 把 JSON 值按 YAML 写入，带备份与原子替换。
pub fn write_json_as_yaml(path: &Path, value: &JsonValue) -> Result<(), String> {
    let yaml = serde_yaml::to_string(value).map_err(|e| format!("序列化 YAML 失败: {}", e))?;
    backup_if_exists(path)?;
    atomic_write(path, yaml.as_bytes())
}

/// 确保目标文件的父目录存在。
pub fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建目录 {:?}: {}", parent, e))?;
    }
    Ok(())
}

/// 覆盖前留一份 `<原名>.bak`。
fn backup_if_exists(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let mut bak = path.as_os_str().to_os_string();
    bak.push(".bak");
    fs::copy(path, Path::new(&bak)).map_err(|e| format!("备份 {:?} 失败: {}", path, e))?;
    Ok(())
}

/// 原子写：同目录临时文件 → fsync → rename。
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("目标路径没有父目录: {:?}", path))?;
    fs::create_dir_all(dir).map_err(|e| format!("无法创建目录 {:?}: {}", dir, e))?;

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());
    let tmp = dir.join(format!(".{}.tmp", file_name));

    {
        let mut f =
            fs::File::create(&tmp).map_err(|e| format!("无法创建临时文件 {:?}: {}", tmp, e))?;
        f.write_all(bytes)
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        f.sync_all()
            .map_err(|e| format!("sync 临时文件失败: {}", e))?;
    }

    fs::rename(&tmp, path).map_err(|e| {
        // rename 失败就把临时文件清掉，别在剧本目录里留垃圾
        let _ = fs::remove_file(&tmp);
        format!("替换 {:?} 失败: {}", path, e)
    })
}

/// 读 `story_config.yaml`，保证返回对象。
pub fn read_story_config(script_dir: &Path) -> Result<JsonValue, String> {
    let path = script_dir.join("story_config.yaml");
    if !path.is_file() {
        return Err(format!("{:?} 缺少 story_config.yaml", script_dir));
    }
    let v = read_yaml_as_json(&path)?;
    if !v.is_object() {
        return Err("story_config.yaml 顶层必须是键值映射".to_string());
    }
    Ok(v)
}

pub fn write_story_config(script_dir: &Path, config: &JsonValue) -> Result<(), String> {
    if !config.is_object() {
        return Err("story_config 必须是对象".to_string());
    }
    write_json_as_yaml(&script_dir.join("story_config.yaml"), config)
}

/// 取一个章节文档，归一成 `{ name, events }`。
pub struct ChapterDoc {
    pub name: Option<String>,
    pub events: Vec<JsonValue>,
    pub extra: Map<String, JsonValue>,
}

impl ChapterDoc {
    pub fn from_json(value: JsonValue) -> Result<Self, String> {
        let obj = match value {
            JsonValue::Object(m) => m,
            JsonValue::Null => Map::new(),
            _ => return Err("章节文件顶层必须是键值映射".to_string()),
        };

        let mut extra = obj.clone();
        extra.remove("name");
        extra.remove("events");

        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let events = match obj.get("events") {
            Some(JsonValue::Array(a)) => a.clone(),
            None | Some(JsonValue::Null) => Vec::new(),
            Some(_) => return Err("events 必须是列表".to_string()),
        };

        Ok(ChapterDoc {
            name,
            events,
            extra,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        let mut out = Map::new();
        if let Some(ref n) = self.name {
            out.insert("name".to_string(), JsonValue::String(n.clone()));
        }
        out.insert("events".to_string(), JsonValue::Array(self.events.clone()));
        for (k, v) in &self.extra {
            out.entry(k.clone()).or_insert_with(|| v.clone());
        }
        JsonValue::Object(out)
    }
}
