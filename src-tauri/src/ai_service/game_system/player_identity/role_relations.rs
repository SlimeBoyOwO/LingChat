//! AI 角色侧的「关系」文件读写：`characters/<角色文件夹>/relations.yml`。
//!
//! 独立成文件而不是塞进 `settings.yml`：后者会被角色编辑页整体重写，
//! 旧版本编辑该角色时会把关系抹掉；独立文件旧版本完全不认识，天然向前兼容。
//!
//! 格式是一张平表，方便手写：
//! ```yaml
//! ai:另一个角色: "同门师兄弟"
//! me:p1730000000000: "初次见面的委托人"
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const RELATIONS_FILE: &str = "relations.yml";

pub fn relations_path(data_dir: &Path, character_folder: &str) -> PathBuf {
    crate::api::resolve_character_dir_in(data_dir, character_folder).join(RELATIONS_FILE)
}

/// 读取角色的关系表。文件不存在 / 解析失败 / 个别键值非法一律降级为空表，
/// 关系是可选数据，坏一条不该阻断整轮对话。
pub fn load(data_dir: &Path, character_folder: &str) -> HashMap<String, String> {
    let path = relations_path(data_dir, character_folder);
    let Ok(raw) = fs::read_to_string(&path) else {
        return HashMap::new();
    };

    match serde_yaml::from_str::<HashMap<String, String>>(&raw) {
        Ok(map) => map
            .into_iter()
            .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
            .collect(),
        Err(e) => {
            tracing::warn!("解析关系文件失败 {:?}: {}", path, e);
            HashMap::new()
        },
    }
}

/// 写入角色的关系表。空表会写成空文件（不删文件，避免和「文件不存在」混淆）。
pub fn save(
    data_dir: &Path,
    character_folder: &str,
    relations: &HashMap<String, String>,
) -> Result<()> {
    let path = relations_path(data_dir, character_folder);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("创建角色目录失败: {:?}", parent))?;
    }

    let cleaned: HashMap<&str, &str> = relations
        .iter()
        .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();

    let yaml = serde_yaml::to_string(&cleaned).context("序列化关系表失败")?;
    fs::write(&path, yaml).with_context(|| format!("写入关系文件失败: {:?}", path))?;
    Ok(())
}
