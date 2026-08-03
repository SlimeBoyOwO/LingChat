//! 剧本文件读写：YAML ⇄ JSON、原子写、备份。
//!
//! 前端只操作 JSON —— 这样只有 Rust 一侧存在 YAML 语义，不会出现两套解析
//! 行为分歧，也顺便绕开了 `fs` 插件 scope 覆盖不到剧本目录的问题。
//!
//! 所有写入都是「写临时文件 → fsync → rename」，并在覆盖前留一份 `.bak`。
//! 原型编辑器是 `open(f, "w")` 直接截断后再写，中途崩溃或断电会把章节清零。
//!
//! 原为 `api/script_editor/io.rs`，迁到 utils 后成为通用的 YAML 文件工具。

use std::fs;
use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value as JsonValue};

/// 读 YAML 文件并转成 JSON 值。
///
/// 空文件 / 只有注释的文件会得到 `Value::Null`，这里统一归一成空对象，
/// 免得调用方到处判空。
pub fn read_yaml_as_json(path: &Path) -> Result<JsonValue, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("无法读取 {:?}: {}（文件必须是 UTF-8）", path, e))?;

    let value: JsonValue = serde_yaml::from_str(&text)
        .map_err(|e| format!("{:?} YAML 解析失败: {}", path, e))?;

    Ok(match value {
        JsonValue::Null => JsonValue::Object(Map::new()),
        other => other,
    })
}

/// 把 JSON 值按 YAML 写入，带备份与原子替换。
pub fn write_json_as_yaml(path: &Path, value: &JsonValue) -> Result<(), String> {
    let yaml = serde_yaml::to_string(value)
        .map_err(|e| format!("序列化 YAML 失败: {}", e))?;
    backup_if_exists(path)?;
    atomic_write(path, yaml.as_bytes())
}

/// 确保目标文件的父目录存在。
///
/// `script_paths::resolve_*` 刻意不建目录（解析路径不该有副作用），所以真正要写盘的
/// 调用方需要显式调它一次。
pub fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建目录 {:?}: {}", parent, e))?;
    }
    Ok(())
}

/// 覆盖前留一份 `<原名>.bak`。
///
/// 只保留最近一份 —— 真正的历史由编辑器的撤销栈负责，`.bak` 只是防「写坏了
/// 而且已经关掉编辑器」这一种情况。
fn backup_if_exists(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let mut bak = path.as_os_str().to_os_string();
    bak.push(".bak");
    fs::copy(path, Path::new(&bak))
        .map_err(|e| format!("备份 {:?} 失败: {}", path, e))?;
    Ok(())
}

/// 原子写：同目录临时文件 → fsync → rename。
///
/// 临时文件必须与目标同目录，否则 rename 可能跨设备而退化成非原子的复制。
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
        let mut f = fs::File::create(&tmp)
            .map_err(|e| format!("无法创建临时文件 {:?}: {}", tmp, e))?;
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
///
/// 引擎只读这两个键（`Chapter::new`），其余顶层键一律忽略；这里把它们原样
/// 保留在 `extra` 里，写回时不丢作者的自定义字段。
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chapter_doc_normalizes_missing_fields() {
        let d = ChapterDoc::from_json(json!({})).unwrap();
        assert_eq!(d.name, None);
        assert!(d.events.is_empty());

        let d = ChapterDoc::from_json(JsonValue::Null).unwrap();
        assert!(d.events.is_empty());
    }

    #[test]
    fn chapter_doc_preserves_unknown_top_level_keys() {
        let d = ChapterDoc::from_json(json!({
            "name": "第一章",
            "events": [{ "type": "narration", "text": "hi" }],
            "author_note": "别删我"
        }))
        .unwrap();
        assert_eq!(d.name.as_deref(), Some("第一章"));
        assert_eq!(d.events.len(), 1);

        let back = d.to_json();
        assert_eq!(back["author_note"], json!("别删我"));
        assert_eq!(back["name"], json!("第一章"));
        assert_eq!(back["events"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn chapter_doc_rejects_wrong_shapes() {
        assert!(ChapterDoc::from_json(json!([])).is_err());
        assert!(ChapterDoc::from_json(json!({ "events": 3 })).is_err());
    }

    #[test]
    fn atomic_write_replaces_and_backs_up() {
        let dir = std::env::temp_dir().join(format!("lc_editor_io_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let f = dir.join("c.yaml");

        write_json_as_yaml(&f, &json!({ "events": [] })).unwrap();
        assert!(f.is_file());
        // 首次写入不该产生 .bak
        assert!(!dir.join("c.yaml.bak").is_file());

        write_json_as_yaml(&f, &json!({ "events": [1] })).unwrap();
        assert!(dir.join("c.yaml.bak").is_file());

        // 不留临时文件
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "残留临时文件: {:?}", leftovers);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn yaml_round_trip_keeps_structure() {
        let dir = std::env::temp_dir().join(format!("lc_editor_rt_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let f = dir.join("r.yaml");

        let original = json!({
            "name": "1 舒适的一天~",
            "events": [
                { "type": "narration", "text": "今天是个出去玩的好日子呢。" },
                { "type": "chapter_end", "end_type": "linear", "next_chapter": "main2" }
            ]
        });
        write_json_as_yaml(&f, &original).unwrap();
        let back = read_yaml_as_json(&f).unwrap();
        assert_eq!(back, original);

        let _ = fs::remove_dir_all(&dir);
    }
}
