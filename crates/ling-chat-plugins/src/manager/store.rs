//! 插件记录与集中状态文件（`data/plugins/state.json`）。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tokio::sync::{Mutex, MutexGuard};

use crate::manifest;
use crate::types::{ConfigKind, PluginRecord, PluginState};

/// 集中插件状态文件名（data/plugins/state.json，仿 tool_permissions.toml）。
const STATE_FILE_NAME: &str = "state.json";

/// 扫描结果与持久化状态的唯一持有者。
pub(super) struct PluginStore {
    /// data/plugins 根目录。
    root: PathBuf,
    /// id → 插件记录。
    records: Mutex<HashMap<String, PluginRecord>>,
}

impl PluginStore {
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            root,
            records: Mutex::new(HashMap::new()),
        }
    }

    pub(super) async fn lock(&self) -> MutexGuard<'_, HashMap<String, PluginRecord>> {
        self.records.lock().await
    }

    pub(super) fn blocking_lock(&self) -> MutexGuard<'_, HashMap<String, PluginRecord>> {
        self.records.blocking_lock()
    }

    /// 扫描根目录下的合法插件目录，解析 manifest 并合并集中状态。
    ///
    /// 根目录读不到时返回 `None`：调用方沿用原来的「保留当前运行状态」语义。
    pub(super) fn scan(
        &self,
        states: &HashMap<String, PluginState>,
    ) -> Option<Vec<(String, PluginRecord)>> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return None;
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            if !is_plugin_dir(&dir) {
                continue;
            }
            let id = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let record = self.load_record(&dir, &id, states);
            out.push((id, record));
        }
        Some(out)
    }

    /// 加载单个插件记录（解析 manifest + 从集中状态读取 state）。
    fn load_record(
        &self,
        dir: &Path,
        id: &str,
        states: &HashMap<String, PluginState>,
    ) -> PluginRecord {
        let mut record = PluginRecord {
            manifest: Default::default(),
            state: PluginState::new(),
            dir: dir.to_path_buf(),
            error: None,
            startup_error: None,
        };
        let text = match std::fs::read_to_string(dir.join("manifest.toml")) {
            Ok(t) => t,
            Err(e) => {
                record.error = Some(format!("读取 manifest.toml 失败: {e}"));
                return record;
            },
        };
        let parsed = match manifest::parse(&text) {
            Ok(m) => m,
            Err(e) => {
                record.error = Some(e.to_string());
                return record;
            },
        };
        if parsed.id != id {
            record.error = Some(format!(
                "manifest.id '{}' 与目录名 '{id}' 不一致",
                parsed.id
            ));
            return record;
        }
        record.manifest = parsed;
        record.state = states.get(id).cloned().unwrap_or_default();
        record
    }

    /// 读取集中状态文件（root/state.json），不存在或损坏时返回空。
    pub(super) fn load_states(&self) -> HashMap<String, PluginState> {
        let path = self.root.join(STATE_FILE_NAME);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    /// 原子写回集中状态文件（tmp + rename 防损坏）。
    pub(super) fn save_states(&self, states: &HashMap<String, PluginState>) {
        let path = self.root.join(STATE_FILE_NAME);
        let tmp = path.with_extension("tmp");
        if let Ok(text) = serde_json::to_string_pretty(states) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }

    /// 把单个插件的状态并入集中状态文件并写回。
    pub(super) fn persist_state(&self, id: &str, state: &PluginState) {
        let mut states = self.load_states();
        states.insert(id.to_string(), state.clone());
        self.save_states(&states);
    }

    /// 同步集中状态文件：为每个存在的插件补一条记录（默认禁用，旧插件目录
    /// 的 state.json 若已启用则迁移保留），删除已不存在的插件记录，并清理
    /// 各插件目录下的旧 state.json。
    pub(super) fn sync_state_file(&self) {
        let existing: HashSet<String> = std::fs::read_dir(&self.root)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| is_plugin_dir(&e.path()))
                    .filter_map(|e| e.file_name().to_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let mut states = self.load_states();
        for id in &existing {
            if !states.contains_key(id) {
                let legacy = std::fs::read_to_string(self.root.join(id).join(STATE_FILE_NAME))
                    .ok()
                    .and_then(|s| serde_json::from_str::<PluginState>(&s).ok());
                states.insert(id.clone(), legacy.unwrap_or_default());
            }
        }
        states.retain(|id, _| existing.contains(id));
        self.save_states(&states);
        // 清理各插件目录下的旧 state.json，集中文件是唯一权威源
        for id in &existing {
            let legacy_path = self.root.join(id).join(STATE_FILE_NAME);
            if legacy_path.exists() {
                let _ = std::fs::remove_file(legacy_path);
            }
        }
    }
}

/// 合法插件目录：是目录、非隐藏（`.` 开头为导入暂存区）、含 manifest.toml。
fn is_plugin_dir(path: &Path) -> bool {
    path.is_dir()
        && !path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        && path.join("manifest.toml").exists()
}

/// 按字段声明类型把 JSON 值归一化；无法转换的返回 `None`（调用方忽略该字段）。
///
/// 前端 number 输入框返回的是字符串，这里统一转成数字，保证插件脚本读到的类型正确。
pub(super) fn coerce_config_value(
    kind: &ConfigKind,
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    match kind {
        ConfigKind::String | ConfigKind::Secret => match value {
            serde_json::Value::String(s) => Some(serde_json::Value::String(s.clone())),
            serde_json::Value::Null => None,
            other => Some(serde_json::Value::String(other.to_string())),
        },
        ConfigKind::Number => match value {
            serde_json::Value::Number(_) => Some(value.clone()),
            serde_json::Value::String(s) => {
                s.trim().parse::<f64>().ok().map(|f| serde_json::json!(f))
            },
            _ => None,
        },
        ConfigKind::Boolean => match value {
            serde_json::Value::Bool(_) => Some(value.clone()),
            serde_json::Value::String(s) => match s.trim() {
                "true" | "1" => Some(serde_json::Value::Bool(true)),
                "false" | "0" => Some(serde_json::Value::Bool(false)),
                _ => None,
            },
            _ => None,
        },
    }
}
