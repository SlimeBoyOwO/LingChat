//! AI 角色侧的「关系」文件读写。
//!
//! 位置：`<data_dir>/game_data/characters/<角色文件夹>/relations.yml`
//!
//! 为什么独立成文件、而不是塞进该角色的 `settings.yml`：
//! `settings.yml` 会被角色编辑页**整体重写**。一旦用户降级到旧版本、
//! 又在旧版本里编辑了这个角色，塞在 settings.yml 里的关系就会被抹掉。
//! 独立文件的额外好处是旧版本程序完全不认识它，因此永远不会读写它
//! ——这正是「向前兼容」的落地方式。
//!
//! 文件格式就是一张平表，方便用户手写：
//! ```yaml
//! ai:另一个角色: "同门师兄弟"
//! me:p1730000000000: "初次见面的委托人，态度谨慎"
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const RELATIONS_FILE: &str = "relations.yml";

pub fn relations_path(data_dir: &Path, character_folder: &str) -> PathBuf {
    crate::api::resolve_character_dir_in(data_dir, character_folder).join(RELATIONS_FILE)
}

/// 读取角色的关系表。
///
/// 文件不存在、解析失败、或个别键值不合法，一律降级为空表/跳过该条：
/// 关系是**可选**数据，任何一条坏数据都不该阻断整轮对话。
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
