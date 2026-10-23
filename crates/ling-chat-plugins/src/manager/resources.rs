//! 插件携带资源（人物 / 剧本 / 音乐 / 背景图 / 环境音）的条目与保留操作。
//!
//! 扫描原语在 crate 根的 `resources` 模块；这里只负责跨插件合并、冲突判定与
//! 「保留到游戏目录」。

use std::collections::{HashMap, HashSet};

use ling_chat_main::plugin_contract::PluginResourceEntry;

use super::PluginManager;
use crate::resources;
use crate::types::{PluginRecord, ResourceKind};

impl PluginManager {
    fn game_data_dir(&self) -> std::path::PathBuf {
        self.data_dir.join("game_data")
    }

    /// 所有「启用且无加载错误」的插件记录，按 id 升序（插件间冲突时先注册者赢）。
    async fn enabled_sorted(&self) -> Vec<PluginRecord> {
        let records = self.store.lock().await;
        let mut list: Vec<PluginRecord> = records
            .values()
            .filter(|r| r.state.enabled && r.error.is_none() && !r.manifest.id.is_empty())
            .cloned()
            .collect();
        list.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        list
    }

    /// 启用且声明了某类资源的插件记录（供角色同步 / 剧本同步 / 列表合并）。
    async fn records_for(&self, kind: ResourceKind) -> Vec<PluginRecord> {
        self.enabled_sorted()
            .await
            .into_iter()
            .filter(|r| r.manifest.resources.contains(&kind))
            .collect()
    }

    /// 游戏自有角色目录名集合（插件角色冲突判定）。
    fn game_character_folders(&self) -> HashSet<String> {
        let dir = self.game_data_dir().join("characters");
        let mut out = HashSet::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if !entry.path().is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "avatar" || name.starts_with('.') {
                    continue;
                }
                out.insert(name);
            }
        }
        out
    }

    /// 游戏侧某类资源的 key 集合（角色 = 目录名；剧本 = script_name；图/音 = 文件名）。
    fn game_keys_for(&self, kind: ResourceKind) -> HashSet<String> {
        match kind {
            ResourceKind::Characters => self.game_character_folders(),
            ResourceKind::Scripts => resources::game_script_names(&self.data_dir),
            ResourceKind::Backgrounds => {
                resources::game_file_names(&self.game_data_dir().join("backgrounds"), false)
            },
            ResourceKind::Musics => {
                resources::game_file_names(&self.game_data_dir().join("musics"), true)
            },
            ResourceKind::Ambients => {
                resources::game_file_names(&self.game_data_dir().join("ambients"), true)
            },
        }
    }

    /// 跨所有启用插件收集某类资源条目，并对游戏同名冲突打 `conflict` 标记。
    /// 返回顺序：按插件 id 升序（先注册者在前）。
    async fn collect_kind_entries(&self, kind: ResourceKind) -> Vec<PluginResourceEntry> {
        let game_keys = self.game_keys_for(kind);
        let mut out = Vec::new();
        for record in self.records_for(kind).await {
            for mut entry in resources::scan_kind(&record, kind) {
                entry.conflict = game_keys.contains(&entry.key);
                out.push(entry);
            }
        }
        out
    }

    /// 文件类资源（背景图 / 音乐 / 环境音）的可见条目：过滤隐藏、游戏同名、插件间重复。
    pub(super) async fn visible_file_entries(
        &self,
        kind: ResourceKind,
    ) -> Vec<PluginResourceEntry> {
        let entries = self.collect_kind_entries(kind).await;
        let mut seen: HashSet<String> = HashSet::new();
        entries
            .into_iter()
            .filter(|e| !e.hidden && !e.conflict && seen.insert(e.key.clone()))
            .collect()
    }

    /// 某插件的全部资源条目（供插件管理页资源区），带 conflict / hidden 标记。
    /// 插件间冲突：同类同 key 被更小 id 的插件占据（且未被隐藏）时，本条目标 conflict。
    pub async fn plugin_resources(&self, id: &str) -> Result<Vec<PluginResourceEntry>, String> {
        let record = {
            let records = self.store.lock().await;
            records
                .get(id)
                .cloned()
                .ok_or_else(|| format!("插件 '{id}' 不存在"))?
        };
        let mut out = Vec::new();
        for kind in record.manifest.resources.clone() {
            let all = self.collect_kind_entries(kind).await;
            // 每个 key 的第一个「未隐藏未冲突」条目的插件为赢家
            let mut winner: HashMap<String, String> = HashMap::new();
            for e in all.iter().filter(|e| !e.hidden && !e.conflict) {
                winner
                    .entry(e.key.clone())
                    .or_insert_with(|| e.plugin_id.clone());
            }
            // 直接扫目标插件自己的目录（无论启用与否），否则禁用插件的资源区恒为空，
            // 玩家没法在删除前「保留」已禁用插件的资源。
            let game_keys = self.game_keys_for(kind);
            for mut e in resources::scan_kind(&record, kind) {
                // 与游戏同名 → 游戏优先（无论隐藏与否都标冲突，与启用插件行为一致）
                e.conflict = game_keys.contains(&e.key);
                // 插件间同名冲突：仅对「非隐藏非游戏冲突」的条目判谁赢
                if !e.conflict && !e.hidden {
                    if let Some(w) = winner.get(&e.key) {
                        if w != id {
                            e.conflict = true;
                        }
                    }
                }
                out.push(e);
            }
        }
        Ok(out)
    }

    /// 隐藏 / 恢复某个插件资源（软删除标记，写入 state.json 的 hidden_resources）。
    pub async fn set_resource_hidden(
        &self,
        id: &str,
        mark: &str,
        hidden: bool,
    ) -> Result<(), String> {
        let mut records = self.store.lock().await;
        let record = records
            .get_mut(id)
            .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
        if hidden {
            if !record.state.hidden_resources.iter().any(|m| m == mark) {
                record.state.hidden_resources.push(mark.to_string());
            }
        } else {
            record.state.hidden_resources.retain(|m| m != mark);
        }
        self.store.persist_state(id, &record.state);
        Ok(())
    }

    /// 「保留」某插件资源：复制到游戏对应目录，成功后自动隐藏插件版。
    /// 返回资源类型，供调用方决定触发哪种重扫（角色 / 剧本）。
    pub async fn keep_resource(&self, id: &str, mark: &str) -> Result<ResourceKind, String> {
        let (kind, key) =
            resources::split_hidden_mark(mark).ok_or_else(|| format!("无效的资源标记: {mark}"))?;
        let record = {
            let records = self.store.lock().await;
            records
                .get(id)
                .cloned()
                .ok_or_else(|| format!("插件 '{id}' 不存在"))?
        };
        let entry = resources::scan_kind(&record, kind)
            .into_iter()
            .find(|e| e.key == key)
            .ok_or_else(|| format!("插件资源不存在: {mark}"))?;

        let game = self.game_data_dir();
        match kind {
            ResourceKind::Characters => {
                let dest = game.join("characters").join(key);
                if dest.exists() {
                    return Err("游戏目录已存在同名角色".to_string());
                }
                resources::copy_dir_all(&entry.path, &dest).map_err(|e| e.to_string())?;
            },
            ResourceKind::Scripts => {
                let folder = entry
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .ok_or_else(|| "无法解析剧本目录名".to_string())?;
                let dest = game.join("scripts").join("standalone").join(&folder);
                if dest.exists() {
                    return Err("游戏目录已存在同名剧本".to_string());
                }
                resources::copy_dir_all(&entry.path, &dest).map_err(|e| e.to_string())?;
            },
            ResourceKind::Musics | ResourceKind::Backgrounds | ResourceKind::Ambients => {
                let dest = game.join(kind.subdir()).join(key);
                if dest.exists() {
                    return Err("游戏目录已存在同名文件".to_string());
                }
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::copy(&entry.path, &dest).map_err(|e| format!("复制文件失败: {e}"))?;
            },
        }

        self.set_resource_hidden(id, mark, true).await?;
        Ok(kind)
    }
}
