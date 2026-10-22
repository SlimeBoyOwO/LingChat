//! 插件管理器：扫描目录、加载 manifest、启停插件、注册/注销工具、持久化状态。
//!
//! 这里只留编排：记录与状态文件在 `store`，工具授权在 `tools`，资源在
//! `resources`，信号派发在 `signals`，WS 连接在 `websocket`，启动入口与依赖在
//! `startup` / `deps`。

mod deps;
mod resources;
mod signals;
mod startup;
mod store;
mod tools;
mod websocket;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use ling_chat_main::ai_service::tools::registry::ToolRegistry;
use ling_chat_main::plugin_contract::PluginResourceEntry;

use super::python_backend;
use super::signal::SignalRegistry;
use super::types::{PluginInfo, PluginRunEnv, ResourceKind, WsState};
use crate::manager::deps::{dependency_error, unmet_dependencies};
use crate::manager::store::PluginStore;
use crate::manager::websocket::{WsHub, ws_open_input};
use crate::ws::WsCommand;

/// 插件管理器。
///
/// 持有一个 `Arc<ToolRegistry>` 引用，启用插件时把 `PluginTool` 注册进去，
/// 禁用时注销。`store` 保存扫描结果与持久化状态。
pub struct PluginManager {
    registry: Arc<ToolRegistry>,
    /// data 根目录（权限配置文件所在处）。
    data_dir: PathBuf,
    /// 插件记录与集中状态文件。
    store: PluginStore,
    /// 宿主信号注册表与插件订阅索引（随插件启停重建）。
    signals: RwLock<SignalRegistry>,
    /// id → 取消令牌。**令牌存在 ⟺ 插件处于启用态**。
    ///
    /// 用于让「被禁用」尽快生效：重试之间的等待可以立刻打断，准备新起解释器时
    /// 也会先查一次。注意它取消的是**等待**，不是正在执行的脚本——阻塞线程里的
    /// 解释器没有任何外部中断手段，只能等它自己返回并丢弃结果。
    cancels: Mutex<HashMap<String, CancellationToken>>,
    /// 插件声明的 WS 连接。
    ws: WsHub,
    /// 报告过「此刻播不了语音」的插件（女仆倒下，外部程序找不到播放目标）。
    ///
    /// 缺省视为能播：插件得在自己的脚本里明确报一次，才轮到它影响接管。
    voice_blocked: RwLock<HashSet<String>>,
}

impl PluginManager {
    /// 创建管理器并扫描目录加载所有插件（含启停状态）。
    /// `data_dir` 是 data 根目录，`root` 是 data/plugins。
    pub fn new(data_dir: PathBuf, registry: Arc<ToolRegistry>) -> Self {
        let manager = Self {
            registry,
            store: PluginStore::new(data_dir.join("plugins")),
            data_dir,
            signals: RwLock::new(SignalRegistry::new()),
            cancels: Mutex::new(HashMap::new()),
            ws: WsHub::new(),
            voice_blocked: RwLock::new(HashSet::new()),
        };
        manager.store.sync_state_file();
        manager.reload();
        manager
    }

    /// 重新扫描目录，重建记录；已启用插件的工具重新注册。
    /// 先同步集中状态文件（补存在、删不存在），再加载。
    pub fn reload(&self) {
        self.store.sync_state_file();
        let mut records = self.store.blocking_lock();
        // 先注销旧记录中已启用插件的工具，避免重扫注册时触发 DuplicateName
        for record in records.values() {
            if record.state.enabled {
                tools::unregister_tools(&self.registry, record);
            }
        }
        records.clear();
        let states = self.store.load_states();
        let Some(scanned) = self.store.scan(&states) else {
            return;
        };
        for (id, mut record) in scanned {
            if record.state.enabled && record.error.is_none() {
                match tools::register_tools(&self.registry, &record) {
                    Ok(()) => {},
                    Err(e) => record.error = Some(e),
                }
            }
            records.insert(id, record);
        }
        self.rebuild_signal_index(&records);
        // 重扫会重建记录：取消令牌对齐到新的启用集合。旧令牌一律作废，
        // 让重扫前还在跑的启动重试立刻停下来。
        let inputs: Vec<(String, websocket::WsOpenInput)> = {
            let mut cancels = self.cancels.blocking_lock();
            for token in cancels.values() {
                token.cancel();
            }
            cancels.clear();
            let mut inputs = Vec::new();
            for record in records.values() {
                if record.state.enabled && record.error.is_none() {
                    cancels.insert(record.manifest.id.clone(), CancellationToken::new());
                    if !record.manifest.ws.is_empty() {
                        inputs.push((record.manifest.id.clone(), ws_open_input(record)));
                    }
                }
            }
            inputs
        };
        // 记录已重建：WS 连接先全部断开，再按新的启用集合重建。
        drop(records);
        self.ws.close_all();
        for (id, input) in &inputs {
            if let Err(e) = self.ws.spawn(id, input, None, None) {
                tracing::warn!(plugin = %id, "重建 WS 连接失败: {e}");
            }
        }
    }

    /// 获取插件目录。
    ///
    /// 在插件脚本线程内调用，`blocking_lock` 等待锁安全。
    pub fn plugin_dir(&self, id: &str) -> Option<PathBuf> {
        let records = self.store.blocking_lock();
        records.get(id).map(|r| r.dir.clone())
    }

    /// 获取插件运行所需的 config、白名单环境变量与可读路径声明。
    ///
    /// 在插件脚本线程内调用，`blocking_lock` 等待锁安全。
    pub fn plugin_run_env(&self, id: &str) -> PluginRunEnv {
        let records = self.store.blocking_lock();
        let Some(record) = records.get(id) else {
            return PluginRunEnv::default();
        };
        PluginRunEnv {
            plugin_id: id.to_string(),
            config: record.state.config.clone(),
            env: python_backend::collect_env(&record.manifest),
            read: record.manifest.read.clone(),
            send_user_message: record.manifest.send_user_message,
            capture_screen: record.manifest.capture_screen,
            trigger_proactive: record.manifest.trigger_proactive,
        }
    }

    /// 插件报告外部程序此刻能不能播这句语音。播不了（女仆倒下）时 `voice_playback_owner`
    /// 跳过它，前端下一句就改回本机播放。
    pub fn set_voice_playback_available(&self, plugin_id: &str, available: bool) {
        let mut blocked = self
            .voice_blocked
            .write()
            .unwrap_or_else(|e| e.into_inner());
        let changed = if available {
            blocked.remove(plugin_id)
        } else {
            blocked.insert(plugin_id.to_string())
        };
        if changed {
            tracing::info!(plugin = %plugin_id, available, "插件报告游戏内语音可用性");
        }
    }

    /// 是否有插件正在接管台词语音的播放。
    ///
    /// 条件：已启用、加载无错、manifest 声明了 `[voice_playback]`、对应的布尔配置为真；
    /// 声明了 `ws_conn` 时该连接还必须处于 Connected（断线回落到本机播放）；
    /// 插件自己没报告过「游戏里播不了」（女仆死了）。多个插件声明时先到先得，返回插件 id。
    pub async fn voice_playback_owner(&self) -> Option<String> {
        let records = self.store.lock().await;
        for record in records.values() {
            if !record.state.enabled || record.error.is_some() {
                continue;
            }
            let Some(playback) = &record.manifest.voice_playback else {
                continue;
            };
            let enabled = record
                .state
                .config
                .get(&playback.config)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if !enabled {
                continue;
            }
            if self
                .voice_blocked
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .contains(&record.manifest.id)
            {
                continue;
            }
            if let Some(conn) = &playback.ws_conn {
                let connected = self
                    .ws
                    .state_of(&record.manifest.id, conn)
                    .is_some_and(|state| state == WsState::Connected);
                if !connected {
                    continue;
                }
            }
            return Some(record.manifest.id.clone());
        }
        None
    }

    /// 列表（供前端）。
    pub async fn list(&self) -> Vec<PluginInfo> {
        let records = self.store.lock().await;
        records
            .values()
            .map(|record| {
                let mut info = PluginInfo::from(record);
                // 用连接表的实时状态覆盖：表里没有该连接 = 未建立（Stopped）。
                for c in &mut info.ws {
                    if let Some(state) = self.ws.state_of(&record.manifest.id, &c.id) {
                        c.state = state;
                    }
                }
                info
            })
            .collect()
    }

    /// 启用/禁用插件：注册或注销其工具，保存状态，刷新权限。
    ///
    /// 启用前先查前置插件（`depends_on`）：缺一个就拒绝，错误码供前端弹窗。
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut records = self.store.lock().await;
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
            tools::register_tools(&self.registry, record).inspect_err(|_e| {
                record.state.enabled = false;
                self.store.persist_state(id, &record.state);
            })?;
        } else {
            tools::unregister_tools(&self.registry, record);
        }
        self.store.persist_state(id, &record.state);
        let _ = self.registry.save_permissions(&self.data_dir);
        self.rebuild_signal_index(&records);
        drop(records);

        if enabled {
            self.cancels
                .lock()
                .await
                .insert(id.to_string(), CancellationToken::new());
            // 建立该插件声明的 WS 连接（脚本仍可用 ws_open/ws_close 按需启停）。
            let input = {
                let records = self.store.lock().await;
                records.get(id).map(ws_open_input)
            };
            if let Some(input) = input {
                if let Err(e) = self.ws.spawn(id, &input, None, None) {
                    tracing::warn!(plugin = %id, "建立 WS 连接失败: {e}");
                }
            }
        } else {
            self.cancel_plugin(id).await;
            self.ws.close(id, None);
        }
        Ok(())
    }

    /// 保存插件配置（按 manifest 声明做类型归一化，无法转换的值忽略不写入）。
    pub async fn save_config(
        &self,
        id: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let mut records = self.store.lock().await;
        let record = records
            .get_mut(id)
            .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
        // 以旧值为底：secret 字段不回传前端、表单里也就没有，替换式保存会把它们抹掉。
        let mut next = record.state.config.clone();
        for field in &record.manifest.config {
            let Some(value) = config.get(&field.key) else {
                continue;
            };
            if let Some(v) = store::coerce_config_value(&field.kind, value) {
                next.insert(field.key.clone(), v);
            }
        }
        let declared: HashSet<&str> = record
            .manifest
            .config
            .iter()
            .map(|field| field.key.as_str())
            .collect();
        next.retain(|key, _| declared.contains(key.as_str()));
        record.state.config = next;
        self.store.persist_state(id, &record.state);
        Ok(())
    }

    /// 删除插件：注销其工具、移除集中状态记录、删除插件目录。
    pub async fn delete_plugin(&self, id: &str) -> Result<(), String> {
        let mut records = self.store.lock().await;
        let record = records
            .get(id)
            .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
        if record.state.enabled {
            tools::unregister_tools(&self.registry, record);
        }
        let dir = record.dir.clone();
        records.remove(id);
        self.rebuild_signal_index(&records);
        let mut states = self.store.load_states();
        states.remove(id);
        self.store.save_states(&states);
        drop(records);
        std::fs::remove_dir_all(&dir).map_err(|e| format!("删除插件目录失败: {e}"))?;
        let _ = self.registry.save_permissions(&self.data_dir);
        // 插件本体没了：让它的启动重试立刻停下、WS 连接断开。依赖它的插件由调用方级联禁用。
        self.cancel_plugin(id).await;
        self.ws.close(id, None);
        Ok(())
    }

    // ============================================================
    // 插件 WebSocket 连接（[[ws]]）
    // ============================================================

    /// 建立某插件声明的 WS 连接（脚本 `ws_open` 用）。`conn_id` 为 None 时处理全部。
    pub(crate) async fn ws_open(
        &self,
        id: &str,
        conn_id: Option<&str>,
        url_override: Option<&str>,
    ) -> Result<(), String> {
        let input = {
            let records = self.store.lock().await;
            let record = records
                .get(id)
                .ok_or_else(|| format!("插件 '{id}' 不存在"))?;
            ws_open_input(record)
        };
        self.ws.spawn(id, &input, conn_id, url_override)
    }

    /// 关闭某插件的 WS 连接（`conn_id` 为 None 时关闭全部）。
    pub(crate) fn ws_close(&self, id: &str, conn_id: Option<&str>) {
        self.ws.close(id, conn_id);
    }

    /// 向某连接发一帧（脚本 `ws_send` 用）。同步、不等待、不回调。
    pub(crate) fn ws_send(
        &self,
        plugin_id: &str,
        conn_id: &str,
        cmd: WsCommand,
    ) -> Result<(), String> {
        self.ws.send(plugin_id, conn_id, cmd)
    }

    /// 向某连接发一条请求并登记等待（脚本 `ws_request` 用），返回应答通道。
    ///
    /// 只登记不等待：等待与超时归调用方（宿主函数）管，连接任务这边必须保持能收帧，
    /// 否则响应回来时没人接。
    pub(crate) fn ws_request(
        &self,
        plugin_id: &str,
        conn_id: &str,
        request_id: String,
        payload: String,
    ) -> Result<tokio::sync::oneshot::Receiver<serde_json::Value>, String> {
        self.ws.request(plugin_id, conn_id, request_id, payload)
    }

    /// 查询某插件声明的 WS 连接及其当前状态（脚本 `ws_status` 用）。
    ///
    /// `conn_id` 指定时只返回该连接（未声明则报错）；否则返回全部声明。
    pub(crate) fn ws_status(
        &self,
        plugin_id: &str,
        conn_id: Option<&str>,
    ) -> Result<Vec<super::types::WsConnInfo>, String> {
        let records = self.store.blocking_lock();
        let record = records
            .get(plugin_id)
            .ok_or_else(|| format!("插件 '{plugin_id}' 不存在"))?;
        self.ws.status(plugin_id, &record.manifest.ws, conn_id)
    }
}

#[async_trait::async_trait]
impl ling_chat_main::plugin_contract::PluginResourceSource for PluginManager {
    async fn visible_file_entries(&self, kind: ResourceKind) -> Vec<PluginResourceEntry> {
        PluginManager::visible_file_entries(self, kind).await
    }
}
