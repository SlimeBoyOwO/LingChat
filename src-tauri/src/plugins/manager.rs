//! 插件管理器：扫描目录、加载 manifest、启停插件、注册/注销工具、持久化状态。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::ai_service::tools::registry::ToolRegistry;

use super::manifest;
use super::python_backend;
use super::tool::PluginTool;
use super::types::{ConfigKind, PluginInfo, PluginRecord, PluginState};

/// 集中插件状态文件名（data/plugins/state.json，仿 tool_permissions.toml）。
const STATE_FILE_NAME: &str = "state.json";

/// 插件管理器。
///
/// 持有一个 `Arc<ToolRegistry>` 引用，启用插件时把 `PluginTool` 注册进去，
/// 禁用时注销。`records` 保存扫描结果与运行期状态。
pub struct PluginManager {
    registry: Arc<ToolRegistry>,
    /// data/plugins 根目录。
    root: PathBuf,
    /// data 根目录（权限配置文件所在处）。
    data_dir: PathBuf,
    /// id → 插件记录。
    records: Mutex<HashMap<String, PluginRecord>>,
}

impl PluginManager {
    /// 创建管理器并扫描目录加载所有插件（含启停状态）。
    /// `data_dir` 是 data 根目录，`root` 是 data/plugins。
    pub fn new(data_dir: PathBuf, registry: Arc<ToolRegistry>) -> Self {
        let root = data_dir.join("plugins");
        let manager = Self {
            registry,
            root,
            data_dir,
            records: Mutex::new(HashMap::new()),
        };
        manager.sync_state_file();
        manager.reload();
        manager
    }

    /// 重新扫描目录，重建记录；已启用插件的工具重新注册。
    /// 先同步集中状态文件（补存在、删不存在），再加载。
    pub fn reload(&self) {
        self.sync_state_file();
        let mut records = self.records.blocking_lock();
        // 先注销旧记录中已启用插件的工具，避免重扫注册时触发 DuplicateName
        for record in records.values() {
            if record.state.enabled {
                self.unregister_tools(record);
            }
        }
        records.clear();
        let states = self.load_states();
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let manifest_path = dir.join("manifest.toml");
            if !manifest_path.exists() {
                continue;
            }
            let id = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let mut record = self.load_record(&dir, &id, &states);
            if record.state.enabled && record.error.is_none() {
                match self.register_tools(&record) {
                    Ok(()) => {}
                    Err(e) => record.error = Some(e),
                }
            }
            records.insert(id, record);
        }
    }

    /// 加载单个插件记录（解析 manifest + 从集中状态读取 state）。
    fn load_record(
        &self,
        dir: &PathBuf,
        id: &str,
        states: &HashMap<String, PluginState>,
    ) -> PluginRecord {
        let mut record = PluginRecord {
            manifest: Default::default(),
            state: PluginState::new(),
            dir: dir.clone(),
            error: None,
        };
        let text = match std::fs::read_to_string(dir.join("manifest.toml")) {
            Ok(t) => t,
            Err(e) => {
                record.error = Some(format!("读取 manifest.toml 失败: {e}"));
                return record;
            }
        };
        let parsed = match manifest::parse(&text) {
            Ok(m) => m,
            Err(e) => {
                record.error = Some(e.to_string());
                return record;
            }
        };
        if parsed.id != id {
            record.error = Some(format!("manifest.id '{}' 与目录名 '{id}' 不一致", parsed.id));
            return record;
        }
        record.manifest = parsed;
        record.state = states.get(id).cloned().unwrap_or_default();
        record
    }

    /// 把插件的所有工具注册进 registry，并并入 available_tools。
    fn register_tools(&self, record: &PluginRecord) -> Result<(), String> {
        let mut registered: Vec<String> = Vec::new();
        for spec in &record.manifest.tools {
            let tool = Arc::new(PluginTool::new(record.manifest.id.clone(), spec.clone()));
            match self.registry.register(tool) {
                Ok(()) => registered.push(spec.name.clone()),
                Err(e) => {
                    // 精确回滚本次已注册的部分，避免与其他插件同名工具残留
                    for name in &registered {
                        self.registry.unregister(name);
                    }
                    return Err(format!("{e}（已回滚 {} 个已注册工具）", registered.len()));
                }
            }
        }
        let names: Vec<String> = record.manifest.tools.iter().map(|t| t.name.clone()).collect();
        self.registry.add_available_tools(&names);
        Ok(())
    }

    /// 注销插件的所有工具，并同步移除 available_tools 展示列表。
    fn unregister_tools(&self, record: &PluginRecord) {
        let names: Vec<String> = record.manifest.tools.iter().map(|t| t.name.clone()).collect();
        for name in &names {
            self.registry.unregister(name);
        }
        self.registry.remove_available_tools(&names);
    }

    /// 获取插件目录。
    ///
    /// 在 `spawn_blocking` 线程内调用，`blocking_lock` 等待锁安全。
    pub fn plugin_dir(&self, id: &str) -> Option<PathBuf> {
        let records = self.records.blocking_lock();
        records.get(id).map(|r| r.dir.clone())
    }

    /// 获取插件运行所需的 config 与白名单环境变量。
    ///
    /// 在 `spawn_blocking` 线程内调用，`blocking_lock` 等待锁安全。
    pub fn plugin_run_env(
        &self,
        id: &str,
    ) -> (
        HashMap<String, serde_json::Value>,
        HashMap<String, String>,
    ) {
        let records = self.records.blocking_lock();
        let Some(record) = records.get(id) else {
            return (HashMap::new(), HashMap::new());
        };
        let config = record.state.config.clone();
        let env = python_backend::collect_env(&record.manifest);
        (config, env)
    }

    /// 列表（供前端）。
    pub async fn list(&self) -> Vec<PluginInfo> {
        let records = self.records.lock().await;
        records.values().map(PluginInfo::from).collect()
    }

    /// 启用/禁用插件：注册或注销其工具，保存状态，刷新权限。
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut records = self.records.lock().await;
        let record = records
            .get_mut(id)
            .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
        if record.error.is_some() {
            return Err(format!("插件 '{id}' 加载失败，无法启用"));
        }
        if record.state.enabled == enabled {
            return Ok(());
        }
        record.state.enabled = enabled;
        if enabled {
            self.register_tools(record).map_err(|e| {
                record.state.enabled = false;
                self.persist_state(id, &record.state);
                e
            })?;
        } else {
            self.unregister_tools(record);
        }
        self.persist_state(id, &record.state);
        let _ = self.registry.save_permissions(&self.data_dir);
        Ok(())
    }

    /// 保存插件配置（按 manifest 声明做类型归一化，无法转换的值忽略不写入）。
    pub async fn save_config(
        &self,
        id: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let mut records = self.records.lock().await;
        let record = records
            .get_mut(id)
            .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
        let mut cleaned: HashMap<String, serde_json::Value> = HashMap::new();
        for field in &record.manifest.config {
            let Some(value) = config.get(&field.key) else {
                continue;
            };
            if let Some(v) = coerce_config_value(&field.kind, value) {
                cleaned.insert(field.key.clone(), v);
            }
        }
        record.state.config = cleaned;
        self.persist_state(id, &record.state);
        Ok(())
    }

    /// 删除插件：注销其工具、移除集中状态记录、删除插件目录。
    pub async fn delete_plugin(&self, id: &str) -> Result<(), String> {
        let mut records = self.records.lock().await;
        let record = records
            .get(id)
            .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
        if record.state.enabled {
            self.unregister_tools(record);
        }
        let dir = record.dir.clone();
        records.remove(id);
        let mut states = self.load_states();
        states.remove(id);
        self.save_states(&states);
        std::fs::remove_dir_all(&dir).map_err(|e| format!("删除插件目录失败: {e}"))?;
        let _ = self.registry.save_permissions(&self.data_dir);
        Ok(())
    }

    /// 读取集中状态文件（root/state.json），不存在或损坏时返回空。
    fn load_states(&self) -> HashMap<String, PluginState> {
        let path = self.root.join(STATE_FILE_NAME);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    /// 原子写回集中状态文件（tmp + rename 防损坏）。
    fn save_states(&self, states: &HashMap<String, PluginState>) {
        let path = self.root.join(STATE_FILE_NAME);
        let tmp = path.with_extension("tmp");
        if let Ok(text) = serde_json::to_string_pretty(states) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }

    /// 把单个插件的状态并入集中状态文件并写回。
    fn persist_state(&self, id: &str, state: &PluginState) {
        let mut states = self.load_states();
        states.insert(id.to_string(), state.clone());
        self.save_states(&states);
    }

    /// 同步集中状态文件：为每个存在的插件补一条记录（默认禁用，旧插件目录
    /// 的 state.json 若已启用则迁移保留），删除已不存在的插件记录，并清理
    /// 各插件目录下的旧 state.json。
    fn sync_state_file(&self) {
        let existing: std::collections::HashSet<String> = std::fs::read_dir(&self.root)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir() && e.path().join("manifest.toml").exists())
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

    /// 供插件工具经 AppHandle 取 registry（debug 用）。
    pub fn registry(&self) -> Arc<ToolRegistry> {
        self.registry.clone()
    }
}

/// 按字段声明类型把 JSON 值归一化；无法转换的返回 `None`（调用方忽略该字段）。
///
/// 前端 number 输入框返回的是字符串，这里统一转成数字，保证插件脚本读到的类型正确。
fn coerce_config_value(kind: &ConfigKind, value: &serde_json::Value) -> Option<serde_json::Value> {
    match kind {
        ConfigKind::String | ConfigKind::Secret => match value {
            serde_json::Value::String(s) => Some(serde_json::Value::String(s.clone())),
            serde_json::Value::Null => None,
            other => Some(serde_json::Value::String(other.to_string())),
        },
        ConfigKind::Number => match value {
            serde_json::Value::Number(_) => Some(value.clone()),
            serde_json::Value::String(s) => s.trim().parse::<f64>().ok().map(|f| serde_json::json!(f)),
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
