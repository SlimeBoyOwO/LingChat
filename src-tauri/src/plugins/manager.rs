//! 插件管理器：扫描目录、加载 manifest、启停插件、注册/注销工具、持久化状态。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures_util::future::join_all;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::AppState;
use crate::ai_service::tools::registry::ToolRegistry;

use super::manifest;
use super::python_backend;
use super::resources::{self, PluginResourceEntry};
use super::signal::SignalRegistry;
use super::tool::PluginTool;
use super::types::{ConfigKind, PluginInfo, PluginRecord, PluginState, ResourceKind, StartupDecl};

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
    /// 宿主信号注册表与插件订阅索引（随插件启停重建）。
    signals: RwLock<SignalRegistry>,
    /// id → 取消令牌。**令牌存在 ⟺ 插件处于启用态**。
    ///
    /// 用于让「被禁用」尽快生效：重试之间的等待可以立刻打断，准备新起解释器时
    /// 也会先查一次。注意它取消的是**等待**，不是正在执行的脚本——阻塞线程里的
    /// 解释器没有任何外部中断手段，只能等它自己返回并丢弃结果。
    cancels: Mutex<HashMap<String, CancellationToken>>,
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
            signals: RwLock::new(SignalRegistry::new()),
            cancels: Mutex::new(HashMap::new()),
        };
        manager.sync_state_file();
        manager.reload();
        manager
    }

    /// 扫描根目录下的合法插件目录：是目录、非隐藏（`.` 开头为导入暂存区）、含 manifest.toml。
    fn is_plugin_dir(path: &Path) -> bool {
        path.is_dir()
            && !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'))
            && path.join("manifest.toml").exists()
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
            if !Self::is_plugin_dir(&dir) {
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
                    Ok(()) => {},
                    Err(e) => record.error = Some(e),
                }
            }
            records.insert(id, record);
        }
        self.rebuild_signal_index(&records);
        // 重扫会重建记录：取消令牌对齐到新的启用集合。旧令牌一律作废，
        // 让重扫前还在跑的启动重试立刻停下来。
        {
            let mut cancels = self.cancels.blocking_lock();
            for token in cancels.values() {
                token.cancel();
            }
            cancels.clear();
            for record in records.values() {
                if record.state.enabled && record.error.is_none() {
                    cancels.insert(record.manifest.id.clone(), CancellationToken::new());
                }
            }
        }
    }

    /// 按当前记录重建信号订阅索引。任何影响「哪些插件在跑」的操作之后都要调一次
    /// （重载 / 启停 / 删除），否则被禁用插件的 handler 仍会被派发。
    fn rebuild_signal_index(&self, records: &HashMap<String, PluginRecord>) {
        let mut signals = self.signals.write().unwrap_or_else(|e| e.into_inner());
        // 「哪些插件在跑」的判定在 SignalRegistry::rebuild 内部，不在这里重复。
        signals.rebuild(records.values());
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
                },
            }
        }
        let names: Vec<String> = record
            .manifest
            .tools
            .iter()
            .map(|t| t.name.clone())
            .collect();
        self.registry.add_available_tools(&names);
        Ok(())
    }

    /// 注销插件的所有工具，并同步移除 available_tools 展示列表。
    fn unregister_tools(&self, record: &PluginRecord) {
        let names: Vec<String> = record
            .manifest
            .tools
            .iter()
            .map(|t| t.name.clone())
            .collect();
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
    ) -> (HashMap<String, serde_json::Value>, HashMap<String, String>) {
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
    ///
    /// 启用前先查前置插件（`depends_on`）：缺一个就拒绝，错误码供前端弹窗。
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut records = self.records.lock().await;
        {
            let record = records
                .get(id)
                .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
            if record.error.is_some() {
                return Err(format!("插件 '{id}' 加载失败，无法启用"));
            }
            if record.state.enabled == enabled {
                return Ok(());
            }
            if enabled {
                let (missing, inactive) = unmet_dependencies(&records, &record.manifest.depends_on);
                if !missing.is_empty() || !inactive.is_empty() {
                    return Err(dependency_error(&missing, &inactive));
                }
            }
        }

        let record = records
            .get_mut(id)
            .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
        record.state.enabled = enabled;
        // 每次状态切换都清掉上次的启动失败原因；自动禁用会在切换之后重新写入
        // （见 `disable_with_reason`，顺序反了会被这里冲掉）。
        record.startup_error = None;
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
        self.rebuild_signal_index(&records);
        drop(records);

        if enabled {
            self.cancels
                .lock()
                .await
                .insert(id.to_string(), CancellationToken::new());
        } else {
            self.cancel_plugin(id).await;
        }
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
        self.rebuild_signal_index(&records);
        let mut states = self.load_states();
        states.remove(id);
        self.save_states(&states);
        drop(records);
        std::fs::remove_dir_all(&dir).map_err(|e| format!("删除插件目录失败: {e}"))?;
        let _ = self.registry.save_permissions(&self.data_dir);
        // 插件本体没了：让它的启动重试立刻停下。依赖它的插件由调用方级联禁用。
        self.cancel_plugin(id).await;
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
                    .filter(|e| Self::is_plugin_dir(&e.path()))
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

    // ============================================================
    // 插件携带资源（人物 / 剧本 / 音乐 / 背景图 / 环境音）
    // ============================================================

    fn game_data_dir(&self) -> PathBuf {
        self.data_dir.join("game_data")
    }

    /// 所有「启用且无加载错误」的插件记录，按 id 升序（插件间冲突时先注册者赢）。
    pub async fn enabled_sorted(&self) -> Vec<PluginRecord> {
        let records = self.records.lock().await;
        let mut list: Vec<PluginRecord> = records
            .values()
            .filter(|r| r.state.enabled && r.error.is_none() && !r.manifest.id.is_empty())
            .cloned()
            .collect();
        list.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        list
    }

    /// 启用且声明了某类资源的插件记录（供角色同步 / 剧本同步 / 列表合并）。
    pub async fn records_for(&self, kind: ResourceKind) -> Vec<PluginRecord> {
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
    pub async fn collect_kind_entries(&self, kind: ResourceKind) -> Vec<PluginResourceEntry> {
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
    pub async fn visible_file_entries(&self, kind: ResourceKind) -> Vec<PluginResourceEntry> {
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
            let records = self.records.lock().await;
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
        let mut records = self.records.lock().await;
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
        self.persist_state(id, &record.state);
        Ok(())
    }

    /// 「保留」某插件资源：复制到游戏对应目录，成功后自动隐藏插件版。
    /// 返回资源类型，供调用方决定触发哪种重扫（角色 / 剧本）。
    pub async fn keep_resource(&self, id: &str, mark: &str) -> Result<ResourceKind, String> {
        let (kind, key) =
            resources::split_hidden_mark(mark).ok_or_else(|| format!("无效的资源标记: {mark}"))?;
        let record = {
            let records = self.records.lock().await;
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

    /// 供启用插件目录列表（角色同步扫描用）：返回 (plugin_id, characters 目录, 隐藏标记集)。
    pub async fn kind_roots(&self, kind: ResourceKind) -> Vec<(String, PathBuf, Vec<String>)> {
        self.records_for(kind)
            .await
            .into_iter()
            .map(|r| {
                (
                    r.manifest.id.clone(),
                    r.dir.join(kind.subdir()),
                    r.state.hidden_resources.clone(),
                )
            })
            .collect()
    }

    // ============================================================
    // 宿主信号派发
    // ============================================================

    /// 把信号派发给命中的插件订阅。
    ///
    /// 筛选全在宿主侧完成：未登记的信号、`match` 不中的订阅都不会产生执行，
    /// 免得为不关心的插件白新建一个解释器。handler 的返回值按约定丢弃，执行失败
    /// 只记日志——信号是宿主业务的旁路，不能反过来影响发出方。
    ///
    /// **当前宿主信号登记表为空，因此还没有调用点**：接入首个信号时在发射点调用
    /// 本方法，并删掉这里的 allow。
    #[allow(dead_code)]
    pub async fn dispatch_signal(&self, app: &AppHandle, signal: &str, payload: &Value) {
        let (subscriptions, slots) = {
            let signals = self.signals.read().unwrap_or_else(|e| e.into_inner());
            (signals.matching(signal, payload), signals.slots())
        };

        for sub in subscriptions {
            // 插件可能刚被停用：不再为它起新的解释器（已在跑的不受影响）。
            if self.is_stopped(&sub.plugin_id).await {
                tracing::debug!(plugin = %sub.plugin_id, signal, "插件已停用，跳过派发");
                continue;
            }
            let Ok(permit) = slots.clone().try_acquire_owned() else {
                tracing::warn!(
                    plugin = %sub.plugin_id,
                    signal,
                    "信号 handler 并发已达上限，本次派发跳过"
                );
                continue;
            };
            let script_path = sub.dir.join(&sub.decl.script);
            let handler = sub.decl.handler.clone();
            let timeout = Duration::from_millis(sub.decl.timeout_ms);
            let plugin_id = sub.plugin_id.clone();
            let plugin_id_log = sub.plugin_id;
            let signal_name = signal.to_string();
            let signal_log = signal_name.clone();
            let payload = payload.clone();
            let app = app.clone();

            tauri::async_runtime::spawn(async move {
                // 超时中断不了阻塞线程，permit 跟着线程留到脚本真正跑完，
                // 免得慢 handler 靠超时把并发槽位「释放」出去。
                let joined = tokio::time::timeout(
                    timeout,
                    tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        let manager = app.state::<AppState>().data().plugin_manager.clone();
                        let (config, env) = manager.plugin_run_env(&plugin_id);
                        python_backend::run_plugin_handler(
                            &script_path,
                            &handler,
                            &signal_name,
                            &payload,
                            &config,
                            &env,
                            app,
                        )
                    }),
                )
                .await;
                match joined {
                    Ok(Ok(Ok(()))) => {},
                    Ok(Ok(Err(e))) => tracing::warn!(
                        plugin = %plugin_id_log,
                        signal = %signal_log,
                        "信号 handler 执行失败: {e}"
                    ),
                    Ok(Err(e)) => tracing::warn!(
                        plugin = %plugin_id_log,
                        signal = %signal_log,
                        "信号 handler 线程异常: {e}"
                    ),
                    Err(_) => tracing::warn!(
                        plugin = %plugin_id_log,
                        signal = %signal_log,
                        timeout_ms = timeout.as_millis() as u64,
                        "信号 handler 执行超时"
                    ),
                }
            });
        }
    }

    // ============================================================
    // 插件启动入口（[startup]）与前置依赖
    // ============================================================

    /// 启动时执行所有启用插件的启动入口。
    ///
    /// 按 `depends_on` 推进：每一轮挑出「前置全部已完成」的插件并行跑完，再进下一轮。
    /// 所以互不依赖的插件仍然并行，只有依赖方会等自己的前置（没有启动函数的前置算
    /// 立即完成）。
    ///
    /// **不变量：就绪判定必须在上一轮全部 `await` 完之后做**。禁用是在
    /// `run_startup_entry` 内部 await 完才生效的，只有这样下一轮才能正确识别出
    /// 「上一轮刚失败被禁用的前置」。改成 fire-and-forget 派发会立刻破坏这条。
    pub async fn run_startup_hooks(&self, app: &AppHandle) {
        let mut pending: HashMap<String, (PathBuf, Option<StartupDecl>)> = {
            let records = self.records.lock().await;
            records
                .values()
                .filter(|r| r.state.enabled && r.error.is_none() && !r.manifest.id.is_empty())
                .map(|r| {
                    (
                        r.manifest.id.clone(),
                        (r.dir.clone(), r.manifest.startup.clone()),
                    )
                })
                .collect()
        };
        if pending.is_empty() {
            return;
        }

        let mut finished: HashSet<String> = HashSet::new();
        loop {
            // 每轮重算一次判死名单。`unrunnable_in_order` 内部跑到不动点，所以一次
            // 调用就能把整条被打断的依赖链剔干净；只推进一层的话，下游节点既进不了
            // 就绪集、又不在判死名单里，会被下面当成环。
            for (id, reason) in self.unrunnable_pending(&pending).await {
                pending.remove(&id);
                tracing::warn!(plugin = %id, reason = %reason, "前置插件未就绪，跳过启动");
                self.disable_with_reason(app, &id, reason).await;
            }

            let ready: Vec<String> = {
                let records = self.records.lock().await;
                let mut ready: Vec<String> = pending
                    .keys()
                    .filter(|id| {
                        records.get(*id).is_some_and(|r| {
                            r.manifest.depends_on.iter().all(|d| finished.contains(d))
                        })
                    })
                    .cloned()
                    .collect();
                ready.sort();
                ready
            };
            if ready.is_empty() {
                break;
            }

            let batch: Vec<(String, PathBuf, Option<StartupDecl>)> = ready
                .into_iter()
                .filter_map(|id| pending.remove(&id).map(|(dir, decl)| (id, dir, decl)))
                .collect();

            let results = join_all(batch.into_iter().map(|(id, dir, decl)| {
                let app = app.clone();
                async move {
                    let ok = self.run_startup_entry(&app, &id, &dir, decl.as_ref()).await;
                    (id, ok)
                }
            }))
            .await;

            // 成功失败都算「已完成」：失败者已被禁用，依赖它的节点会在下一轮的
            // 判死里按「前置未启用」处理。
            for (id, _) in results {
                finished.insert(id);
            }
        }

        // 剩下的进不了就绪集。判死跑到不动点之后，「前置缺失/停用」已经全部在每轮
        // 开头剔除干净，所以剩下的只能是环。（判死只推进一层时这条不成立：下游会被
        // 误报成环，见 `unrunnable_in_order`。）
        if !pending.is_empty() {
            let mut cyclic: Vec<String> = pending.keys().cloned().collect();
            cyclic.sort();
            tracing::warn!(plugins = %cyclic.join(", "), "检测到循环依赖，跳过启动");
            let reason = format!("PLUGIN_DEPENDENCY_CYCLE|{}", cyclic.join(", "));
            for id in cyclic {
                self.disable_with_reason(app, &id, reason.clone()).await;
            }
        }
    }

    /// 启用插件后跑一次它的启动入口（与启动时同一套逻辑）。
    pub async fn run_startup_hook_for(&self, app: &AppHandle, id: &str) {
        let target = {
            let records = self.records.lock().await;
            records.get(id).and_then(|r| {
                (r.state.enabled && r.error.is_none())
                    .then(|| (r.dir.clone(), r.manifest.startup.clone()))
            })
        };
        let Some((dir, decl)) = target else {
            return;
        };
        self.run_startup_entry(app, id, &dir, decl.as_ref()).await;
    }

    /// 跑一个插件的启动入口，失败按 `retries` 重试。
    ///
    /// 返回是否「已就绪」：没有启动函数、执行成功、或 `required = false` 的尽力而为
    /// 失败都算就绪。重试期间插件保持启用，只有彻底放弃（且 `required`）才禁用并推事件。
    async fn run_startup_entry(
        &self,
        app: &AppHandle,
        id: &str,
        dir: &Path,
        decl: Option<&StartupDecl>,
    ) -> bool {
        let Some(decl) = decl else {
            return true;
        };
        // 只读查找，**不存在即视为已停用**。这里绝不能「没有就新建」——插件刚被
        // 禁用时令牌已被移除，新建出来的令牌是未取消的，会把本该跳过的插件放过去。
        let Some(token) = self.live_token(id).await else {
            tracing::info!(plugin = %id, "插件已停用，跳过启动入口");
            return false;
        };
        // retries = 首次失败后再试 N 次，共最多 N+1 次执行。
        let attempts = decl.retries + 1;
        let mut last_error = String::new();

        for attempt in 0..attempts {
            if token.is_cancelled() {
                tracing::info!(plugin = %id, "插件已停用，放弃启动入口");
                return false;
            }
            if attempt > 0 {
                // 重试前等待；插件在此期间被停用就立刻醒，不等满间隔。
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!(plugin = %id, "插件已停用，放弃启动重试");
                        return false;
                    },
                    _ = tokio::time::sleep(Duration::from_millis(decl.retry_interval_ms)) => {},
                }
            }
            match self.run_startup_once(app, id, dir, decl, &token).await {
                Ok(()) => {
                    tracing::info!(plugin = %id, "启动入口执行完成");
                    return true;
                },
                Err(e) => {
                    last_error = e;
                    tracing::warn!(
                        plugin = %id,
                        attempt = attempt + 1,
                        attempts,
                        "启动入口执行失败: {last_error}"
                    );
                },
            }
        }

        let reason = format!("PLUGIN_STARTUP_FAILED|{last_error}");
        if decl.required {
            tracing::warn!(plugin = %id, attempts, "启动入口重试耗尽，禁用插件");
            self.disable_with_reason(app, id, reason).await;
            false
        } else {
            // 尽力而为的初始化：记录下来但不影响插件可用性。这里不推 auto-disabled
            // 事件——插件并没有被关掉，滑块不该动。
            tracing::warn!(plugin = %id, attempts, "启动入口重试耗尽（required = false，仅记录）");
            let mut records = self.records.lock().await;
            if let Some(record) = records.get_mut(id) {
                record.startup_error = Some(reason);
            }
            true
        }
    }

    /// 跑一次启动入口（不含重试）。
    async fn run_startup_once(
        &self,
        app: &AppHandle,
        id: &str,
        dir: &Path,
        decl: &StartupDecl,
        token: &CancellationToken,
    ) -> Result<(), String> {
        let script_path = dir.join(&decl.script);
        let handler = decl.handler.clone();
        let timeout = Duration::from_millis(decl.timeout_ms);
        let plugin_id = id.to_string();
        let app_handle = app.clone();
        let token = token.clone();

        let joined = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                // 起解释器前最后一道闸：刚被停用的插件不再白跑一次。
                // 注意已经进到脚本里的执行无法中断，只能等它返回后丢弃结果。
                if token.is_cancelled() {
                    return Err("插件已被停用".to_string());
                }
                let manager = app_handle.state::<AppState>().data().plugin_manager.clone();
                let (config, env) = manager.plugin_run_env(&plugin_id);
                python_backend::run_plugin_startup(
                    &script_path,
                    &handler,
                    &config,
                    &env,
                    app_handle,
                )
            }),
        )
        .await;

        match joined {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(format!("插件线程异常: {e}")),
            Err(_) => Err(format!("执行超时（{}ms）", timeout.as_millis())),
        }
    }

    /// 取出当前待调度插件里「前置未就绪」的那批（含传递依赖），返回 (id, 错误码)。
    ///
    /// 判定本身在 [`unrunnable_in_order`] 里（纯函数，内部跑到不动点）；这里只负责
    /// 把 records 拍成它的三个输入。
    async fn unrunnable_pending(
        &self,
        pending: &HashMap<String, (PathBuf, Option<StartupDecl>)>,
    ) -> Vec<(String, String)> {
        // 以 map 的键（= 插件目录名）为准，不用 `manifest.id`：manifest 解析失败的
        // 记录里 `manifest.id` 是空的，拿它比对会把「装坏了的前置」误报成「没安装」。
        let (installed, unavailable, depends) = {
            let records = self.records.lock().await;
            let installed: HashSet<String> = records.keys().cloned().collect();
            let unavailable: HashSet<String> = records
                .iter()
                .filter(|(_, r)| !r.state.enabled || r.error.is_some())
                .map(|(id, _)| id.clone())
                .collect();
            let depends: HashMap<String, Vec<String>> = pending
                .keys()
                .filter_map(|id| {
                    records
                        .get(id)
                        .map(|r| (id.clone(), r.manifest.depends_on.clone()))
                })
                .collect();
            (installed, unavailable, depends)
        };
        unrunnable_in_order(&depends, &installed, &unavailable)
    }

    /// 禁用插件、记录原因，并通知前端（滑块动画关闭 + 显示原因）。
    ///
    /// 前端只按这个事件改 UI 状态、**不弹窗**——弹窗只留给用户主动启用被拒的场景。
    async fn disable_with_reason(&self, app: &AppHandle, id: &str, reason: String) {
        // 必须先切状态再写原因：set_enabled 会清掉上一次的原因，顺序反了会被冲掉。
        if let Err(e) = self.set_enabled(id, false).await {
            tracing::warn!(plugin = %id, "禁用失败: {e}");
            return;
        }
        {
            let mut records = self.records.lock().await;
            if let Some(record) = records.get_mut(id) {
                record.startup_error = Some(reason.clone());
            }
        }
        if let Err(e) = app.emit(
            "plugin:auto-disabled",
            serde_json::json!({ "id": id, "reason": reason }),
        ) {
            tracing::warn!(plugin = %id, "emit plugin:auto-disabled 失败: {e}");
        }
    }

    /// 级联禁用依赖该插件的其他插件（含传递依赖）。
    ///
    /// 前置消失后下游不是「没跑」而是「跑着但坏的」：上游的工具已从 registry 注销、
    /// 信号订阅已移除，下游调用时会直接失败。所以禁用与卸载都要级联。
    ///
    /// **不自动恢复**：上游重新启用后，下游需要用户手动重新启用（卡片上会显示原因）。
    pub async fn cascade_disable_dependents(&self, app: &AppHandle, upstream: &str) {
        let mut queue = vec![upstream.to_string()];
        while let Some(current) = queue.pop() {
            let (exists, dependents): (bool, Vec<String>) = {
                let records = self.records.lock().await;
                let dependents = records
                    .values()
                    .filter(|r| {
                        r.state.enabled && r.manifest.depends_on.iter().any(|d| d == &current)
                    })
                    .map(|r| r.manifest.id.clone())
                    .collect();
                (records.contains_key(&current), dependents)
            };
            // 上游还在 → 只是被停用；已经删了 → 是缺失，用户的处理动作不同。
            let reason = if exists {
                format!("PLUGIN_INACTIVE_DEPENDENCY|{current}")
            } else {
                format!("PLUGIN_MISSING_DEPENDENCY|{current}")
            };
            for id in dependents {
                tracing::info!(plugin = %id, upstream = %current, "前置插件已停用，级联禁用");
                self.disable_with_reason(app, &id, reason.clone()).await;
                queue.push(id);
            }
        }
    }

    /// 取插件当前的有效令牌：**令牌不存在（已禁用/卸载）或已被取消时返回 `None`**。
    ///
    /// 只读，绝不「没有就新建」——那会让刚被停用的插件看起来仍然是活的。
    async fn live_token(&self, id: &str) -> Option<CancellationToken> {
        let cancels = self.cancels.lock().await;
        match cancels.get(id) {
            Some(token) if !token.is_cancelled() => Some(token.clone()),
            _ => None,
        }
    }

    /// 取消并移除某插件的令牌（禁用/卸载时调用）。
    async fn cancel_plugin(&self, id: &str) {
        if let Some(token) = self.cancels.lock().await.remove(id) {
            token.cancel();
        }
    }

    /// 插件是否已停用（令牌不存在，或已被取消）。
    async fn is_stopped(&self, id: &str) -> bool {
        self.live_token(id).await.is_none()
    }
}

/// 把前置插件分成「未安装的」与「已安装但不可用的」。
///
/// `installed` / `unavailable` 都以插件目录名（= 插件 id）为准，不要用 `manifest.id`：
/// manifest 解析失败的记录里 `manifest.id` 是空的。
fn classify_deps(
    depends_on: &[String],
    installed: &HashSet<String>,
    unavailable: &HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    let mut missing = Vec::new();
    let mut inactive = Vec::new();
    for dep in depends_on {
        if !installed.contains(dep) {
            missing.push(dep.clone());
        } else if unavailable.contains(dep) {
            inactive.push(dep.clone());
        }
    }
    (missing, inactive)
}

/// 计算未满足的前置：返回（未安装的，已安装但未启用的）。
fn unmet_dependencies(
    records: &HashMap<String, PluginRecord>,
    depends_on: &[String],
) -> (Vec<String>, Vec<String>) {
    let installed: HashSet<String> = records.keys().cloned().collect();
    let unavailable: HashSet<String> = records
        .iter()
        .filter(|(_, r)| !r.state.enabled || r.error.is_some())
        .map(|(id, _)| id.clone())
        .collect();
    classify_deps(depends_on, &installed, &unavailable)
}

/// 算出待调度插件里「前置未就绪」的那批，返回 (插件 id, 错误码)。
///
/// **内部跑到不动点。** 判死一个插件意味着它不能再当前置，而它可能正是下游插件的前置
/// ——下游要等下一轮才有理由被判死，所以一次 pass 只推进一层。把单层结果直接交给
/// `run_startup_hooks` 去算就绪集，下游就会既进不了就绪集、又不在判死名单里，最后被
/// 当成循环依赖：`A → B → C` 里 C 缺失时，B 被判死，而 A 会背上 `PLUGIN_DEPENDENCY_CYCLE`。
///
/// 纯函数：只读输入、不碰状态，`blocked` 是本函数自己累积的。真正的「禁用」由调用方做，
/// 且调用方必须在自己的循环里每轮重新调一次——上一轮启动失败被禁用的前置只会体现在
/// 调用方重新拍出来的 `unavailable` 里。
fn unrunnable_in_order(
    depends: &HashMap<String, Vec<String>>,
    installed: &HashSet<String>,
    unavailable: &HashSet<String>,
) -> Vec<(String, String)> {
    // 初始不可用 = 已安装但被禁用 / 加载失败；判死的插件陆续累加进来。
    let mut blocked = unavailable.clone();
    let mut guilty: Vec<(String, String)> = Vec::new();
    loop {
        let mut round: Vec<(String, String)> = depends
            .iter()
            .filter(|(id, _)| !blocked.contains(*id))
            .filter_map(|(id, deps)| {
                let (missing, inactive) = classify_deps(deps, installed, &blocked);
                (!missing.is_empty() || !inactive.is_empty())
                    .then(|| (id.clone(), dependency_error(&missing, &inactive)))
            })
            .collect();
        if round.is_empty() {
            return guilty;
        }
        // 同一轮里的节点互不为前置（互为前置就是环，会一直留在 depends 里）。排序只为
        // 让结果稳定，不影响判定。
        round.sort();
        for (id, reason) in round {
            blocked.insert(id.clone());
            guilty.push((id, reason));
        }
    }
}

/// 前置不满足时的错误码（`错误码|补充信息`）。两类都有时先报未安装的——
/// 那是更靠前的处理动作。
fn dependency_error(missing: &[String], inactive: &[String]) -> String {
    if !missing.is_empty() {
        format!("PLUGIN_MISSING_DEPENDENCY|{}", missing.join(", "))
    } else {
        format!("PLUGIN_INACTIVE_DEPENDENCY|{}", inactive.join(", "))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::types::PluginManifest;

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// id → depends_on。
    fn deps(pairs: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(id, ds)| (id.to_string(), ds.iter().map(|d| d.to_string()).collect()))
            .collect()
    }

    /// 正常记录：manifest.id 与目录名一致。
    fn record(id: &str, enabled: bool) -> PluginRecord {
        PluginRecord {
            manifest: PluginManifest {
                id: id.to_string(),
                ..Default::default()
            },
            state: PluginState {
                enabled,
                ..Default::default()
            },
            dir: PathBuf::from(format!("/plugins/{id}")),
            error: None,
            startup_error: None,
        }
    }

    /// 加载失败的记录：manifest 保持 Default，所以 manifest.id 是空的。
    fn broken_record(id: &str) -> PluginRecord {
        PluginRecord {
            manifest: PluginManifest::default(),
            state: PluginState::default(),
            dir: PathBuf::from(format!("/plugins/{id}")),
            error: Some("manifest 解析失败".to_string()),
            startup_error: None,
        }
    }

    /// 回归测试：`A → B → C` 里 C 缺失时，A 不能被留着让外层报成循环依赖。
    ///
    /// 判死只推进一层的话，B 被判死后 A 既进不了就绪集、又不在判死名单里，会被
    /// `run_startup_hooks` 末尾当成环（reason 里写着 `PLUGIN_DEPENDENCY_CYCLE`），
    /// 而真正的原因在 C。
    #[test]
    fn transitive_missing_dependency_dooms_whole_chain() {
        let depends = deps(&[("a", &["b"]), ("b", &["c"])]);
        let guilty = unrunnable_in_order(&depends, &set(&["a", "b"]), &HashSet::new());
        assert_eq!(
            guilty,
            vec![
                ("b".to_string(), "PLUGIN_MISSING_DEPENDENCY|c".to_string()),
                ("a".to_string(), "PLUGIN_INACTIVE_DEPENDENCY|b".to_string()),
            ],
            "整条被打断的依赖链都要被判死，不能把 a 留给外层当环"
        );
    }

    #[test]
    fn mutual_dependencies_are_left_for_the_cycle_report() {
        let depends = deps(&[("a", &["b"]), ("b", &["a"])]);
        let guilty = unrunnable_in_order(&depends, &set(&["a", "b"]), &HashSet::new());
        assert!(
            guilty.is_empty(),
            "互为前置不是「前置未就绪」，该留给外层报环"
        );
    }

    #[test]
    fn disabled_dependency_dooms_dependent() {
        let depends = deps(&[("a", &["b"])]);
        let guilty = unrunnable_in_order(&depends, &set(&["a", "b"]), &set(&["b"]));
        assert_eq!(
            guilty,
            vec![("a".to_string(), "PLUGIN_INACTIVE_DEPENDENCY|b".to_string())]
        );
    }

    #[test]
    fn missing_dependency_is_reported_before_inactive_one() {
        let depends = deps(&[("a", &["x", "b"])]);
        let guilty = unrunnable_in_order(&depends, &set(&["a", "b"]), &set(&["b"]));
        assert_eq!(
            guilty,
            vec![("a".to_string(), "PLUGIN_MISSING_DEPENDENCY|x".to_string())],
            "两类都不满足时先报「未安装」——那是更靠前的处理动作"
        );
    }

    #[test]
    fn satisfied_dependencies_are_untouched() {
        let depends = deps(&[("a", &["b"]), ("b", &[])]);
        assert!(unrunnable_in_order(&depends, &set(&["a", "b"]), &HashSet::new()).is_empty());
    }

    /// 判定按 map 的键（目录名）走。manifest 坏掉的记录 `manifest.id` 是空的，
    /// 拿它比对会把「装坏了的前置」误报成「没安装」。
    #[test]
    fn unmet_dependencies_identifies_broken_manifest_by_dir_name() {
        let records = HashMap::from([("base".to_string(), broken_record("base"))]);
        let (missing, inactive) = unmet_dependencies(&records, &["base".to_string()]);
        assert!(missing.is_empty(), "已安装的前置不该报成未安装");
        assert_eq!(inactive, vec!["base".to_string()]);
    }

    #[test]
    fn unmet_dependencies_splits_missing_from_disabled() {
        let records = HashMap::from([
            ("lib".to_string(), record("lib", true)),
            ("off".to_string(), record("off", false)),
        ]);
        assert_eq!(
            unmet_dependencies(&records, &["lib".to_string()]),
            (vec![], vec![]),
            "已启用且无加载错误的前置算满足"
        );
        assert_eq!(
            unmet_dependencies(&records, &["off".to_string(), "nope".to_string()]),
            (vec!["nope".to_string()], vec!["off".to_string()])
        );
    }
}
