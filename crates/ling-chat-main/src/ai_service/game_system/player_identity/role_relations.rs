//! AI 角色侧的关系文件读写：`characters/<角色文件夹>/relations.yml`，存的是出厂初始关系。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const RELATIONS_FILE: &str = "relations.yml";

pub fn relations_path(data_dir: &Path, character_folder: &str) -> PathBuf {
    crate::api::resolve_character_dir_in(data_dir, character_folder).join(RELATIONS_FILE)
}

/// 读取角色的关系表；文件缺失或解析失败一律降级为空表。
pub fn load(data_dir: &Path, character_folder: &str) -> HashMap<String, String> {
    let path = relations_path(data_dir, character_folder);
    let Ok(raw) = fs::read_to_string(&path) else {
        return HashMap::new();
    };

    match serde_yaml::from_str::<HashMap<String, String>>(&raw) {
        Ok(map) => sanitize(&map),
        Err(e) => {
            tracing::warn!("解析关系文件失败 {:?}: {}", path, e);
            HashMap::new()
        },
    }
}

pub fn sanitize(relations: &HashMap<String, String>) -> HashMap<String, String> {
    relations
        .iter()
        .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

pub fn save(
    data_dir: &Path,
    character_folder: &str,
    relations: &HashMap<String, String>,
) -> Result<()> {
    let path = relations_path(data_dir, character_folder);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("创建角色目录失败: {:?}", parent))?;
    }

    let cleaned = sanitize(relations);
    let yaml = serde_yaml::to_string(&cleaned).context("序列化关系表失败")?;
    fs::write(&path, yaml).with_context(|| format!("写入关系文件失败: {:?}", path))?;
    Ok(())
}
