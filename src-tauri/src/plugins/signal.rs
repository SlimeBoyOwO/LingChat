//! 插件信号订阅：宿主登记信号，插件声明订阅，宿主按需派发。
//!
//! 与「工具」的差别：工具由 LLM 主动调用、返回值给模型；信号由宿主在业务点发出，
//! handler 的返回值被丢弃。因此订阅索引按「信号名 → 订阅列表」预建，派发时先查
//! 注册表、再按 `match` 在宿主侧筛选——只有命中的订阅才会新建解释器执行脚本。
//!
//! 已登记的信号：
//!
//! - [`SIGNAL_AI_REPLY`]：每条助手回复（自由对话、剧本固定台词、主动消息都会触发）
//! - [`SIGNAL_WS_MESSAGE`]：插件声明的 WS 连接的建立 / 断开 / 收到帧 / 出错
//!
//! 接入新信号时：
//! 1. 在 [`SignalRegistry::new`] 里 `register` 一条 [`SignalSpec`]（名称 + payload 字段）；
//! 2. 在对应业务点调用 `PluginManager::dispatch_signal`。
//!
//! 未登记的信号不会被派发；插件对它的订阅只在加载时 warn，不算 manifest 错误，
//! 这样信号上线前后插件包都能正常安装。

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Semaphore;

use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::message_system::responses::ReplyResponse;
use crate::db::managers::role_repo::RoleRepo;

use super::types::{MatchValue, PluginRecord, SubscribeDecl};

/// 同时执行的 handler 上限。信号可能从任意业务点高频发出，不设上限会把
/// 阻塞线程打满。
const MAX_CONCURRENT_HANDLERS: usize = 4;

/// 每条助手回复。payload 与前端收到的 `ai:reply` 事件完全一致（camelCase）。
pub const SIGNAL_AI_REPLY: &str = "ai_reply";

/// WebSocket 连接事件：连接建立 / 断开、收到帧、出错都会发。
///
/// payload 顶层字段见 [`WS_MESSAGE_FIELDS`]。只有声明了该连接的插件会收到
/// （派发时按连接所属插件过滤，见 `PluginManager::dispatch_signal`）。
pub const SIGNAL_WS_MESSAGE: &str = "ws_message";

/// [`SIGNAL_WS_MESSAGE`] payload 的顶层字段（camelCase）。
pub const WS_MESSAGE_FIELDS: &[&str] = &["connId", "mode", "event", "data", "binary", "error"];

/// [`SIGNAL_AI_REPLY`] payload 的顶层字段（camelCase）。
///
/// 只用于校验插件 `match` 里写的键是否存在——写错了表现为「永不命中」，
/// 加载时给一条 warn 比让插件作者自己猜要省事。
pub const AI_REPLY_FIELDS: &[&str] = &[
    "type",
    "duration",
    "isFinal",
    "character",
    "roleId",
    "emotion",
    "originalTag",
    "message",
    "ttsText",
    "motionText",
    "audioFile",
    "originalMessage",
    "displayName",
    "displaySubtitle",
    "userMessageSeq",
    "thinking",
    "previewGen",
    "avatarDir",
];

/// 宿主登记的一条信号。
#[derive(Clone, Copy)]
pub struct SignalSpec {
    /// 信号名，派发与订阅都按它匹配。
    pub name: &'static str,
    /// 说明，供文档与插件页展示。暂无读取方，留给插件页。
    #[allow(dead_code)]
    pub description: &'static str,
    /// payload 的顶层字段名，用于校验插件 `match` 的键是否写错。
    pub fields: &'static [&'static str],
}

/// 一条已登记的订阅（由插件 manifest 构建）。
///
/// `dir` 在建索引时就解析好：派发是热路径，不该为了拿脚本路径去抢 records 锁
/// （那把锁只有 `blocking_lock` 一条取法，在 async 上下文里调用会 panic）。
#[derive(Clone, Debug)]
pub(crate) struct Subscription {
    pub plugin_id: String,
    pub dir: std::path::PathBuf,
    pub decl: SubscribeDecl,
}

impl Subscription {
    /// 按 `match` 条件筛选。条件为空时一律命中；payload 缺字段时不命中。
    pub(crate) fn accepts(&self, payload: &Value) -> bool {
        self.decl
            .matches
            .iter()
            .all(|(key, expected)| expected.hits(payload.get(key)))
    }
}

/// 宿主信号注册表与订阅索引。
pub struct SignalRegistry {
    specs: HashMap<&'static str, SignalSpec>,
    index: HashMap<String, Vec<Subscription>>,
    slots: Arc<Semaphore>,
}

impl Default for SignalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            specs: HashMap::new(),
            index: HashMap::new(),
            slots: Arc::new(Semaphore::new(MAX_CONCURRENT_HANDLERS)),
        };
        registry.register(SignalSpec {
            name: SIGNAL_AI_REPLY,
            description: "每条助手回复（自由对话、剧本固定台词、主动消息都会触发）",
            fields: AI_REPLY_FIELDS,
        });
        registry.register(SignalSpec {
            name: SIGNAL_WS_MESSAGE,
            description: "插件声明的 WS 连接：建立 / 断开 / 收到帧 / 出错",
            fields: WS_MESSAGE_FIELDS,
        });
        registry
    }

    /// 登记一个宿主信号。只应在 [`SignalRegistry::new`] 里调用。
    pub fn register(&mut self, spec: SignalSpec) {
        self.specs.insert(spec.name, spec);
    }

    /// handler 并发槽位。
    pub(crate) fn slots(&self) -> Arc<Semaphore> {
        self.slots.clone()
    }

    /// 按当前插件记录重建订阅索引。
    ///
    /// **只索引「启用且无加载错误」的插件**——被禁用插件的订阅不应再被派发。
    /// 这条不变量由本函数自己保证，不交给调用方过滤：调用方漏一次就会让禁用插件的
    /// handler 继续被触发。插件启停 / 重载 / 删除后都要调一次。
    pub(crate) fn rebuild<'a>(&mut self, records: impl Iterator<Item = &'a PluginRecord>) {
        self.index.clear();
        let live =
            records.filter(|r| r.state.enabled && r.error.is_none() && !r.manifest.id.is_empty());
        for record in live {
            let plugin_id = &record.manifest.id;
            for decl in &record.manifest.subscribe {
                match self.specs.get(decl.signal.as_str()) {
                    // 从宽处理：订阅了未登记的信号只 warn，不算 manifest 错误，
                    // 这样信号上线前后插件包都能正常安装（上线后订阅自动生效）。
                    None => tracing::warn!(
                        plugin = %plugin_id,
                        signal = %decl.signal,
                        "插件订阅了未注册的信号，暂不生效"
                    ),
                    Some(spec) => {
                        // match 键拼错会表现为「永不命中」的静默失败，这里提前提示。
                        for key in decl.matches.keys() {
                            if !spec.fields.contains(&key.as_str()) {
                                tracing::warn!(
                                    plugin = %plugin_id,
                                    signal = %decl.signal,
                                    field = %key,
                                    "match 字段不在该信号的 payload 中，该条件永不命中"
                                );
                            }
                        }
                    },
                }
                self.index
                    .entry(decl.signal.clone())
                    .or_default()
                    .push(Subscription {
                        plugin_id: plugin_id.clone(),
                        dir: record.dir.clone(),
                        decl: decl.clone(),
                    });
            }
            // [[ws]] 内联 handler：合成一条等价的 ws_message 订阅，match 本连接 id。
            // 派发路径不分叉——插件用内联 handler 还是 [[subscribe]] 效果一致。
            for ws in &record.manifest.ws {
                let (Some(script), Some(handler)) = (&ws.script, &ws.handler) else {
                    continue;
                };
                let mut matches = HashMap::new();
                matches.insert(
                    "connId".to_string(),
                    MatchValue::One(Value::String(ws.id.clone())),
                );
                self.index
                    .entry(SIGNAL_WS_MESSAGE.to_string())
                    .or_default()
                    .push(Subscription {
                        plugin_id: plugin_id.clone(),
                        dir: record.dir.clone(),
                        decl: SubscribeDecl {
                            signal: SIGNAL_WS_MESSAGE.to_string(),
                            script: script.clone(),
                            handler: handler.clone(),
                            timeout_ms: ws.timeout_ms,
                            matches,
                        },
                    });
            }
        }
    }

    /// 取该信号命中的订阅。未登记的信号一律返回空（不派发）。
    pub(crate) fn matching(&self, signal: &str, payload: &Value) -> Vec<Subscription> {
        if !self.specs.contains_key(signal) {
            return Vec::new();
        }
        self.index
            .get(signal)
            .map(|subs| {
                subs.iter()
                    .filter(|sub| sub.accepts(payload))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 是否有插件订阅了该信号（只看索引，不做 `match` 筛选，未登记的信号恒为 false）。
    ///
    /// 给发射点用：载荷准备（序列化、查库读文件）可能不便宜，先探一次有没有人订阅，
    /// 没有就免掉这份开销——零插件订阅是常态。
    pub(crate) fn has_subscribers(&self, signal: &str) -> bool {
        self.specs.contains_key(signal)
            && self.index.get(signal).is_some_and(|subs| !subs.is_empty())
    }
}

/// 把一条助手回复派发给订阅了 [`SIGNAL_AI_REPLY`] 的插件。
///
/// 先探一次有没有订阅者再准备载荷：`avatarDir` 要走一次「查库 + 读角色 YAML」，
/// 零插件订阅时不该为它买单。handler 本身在后台线程执行，不阻塞回复流水线。
pub async fn emit_ai_reply(app: &AppHandle, resp: &ReplyResponse) {
    let manager = app.state::<AppState>().data().plugin_manager.clone();
    if !manager.has_signal_subscribers(SIGNAL_AI_REPLY) {
        return;
    }

    let mut payload = match serde_json::to_value(resp) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::warn!("ai_reply 信号载荷序列化失败: {e}");
            return;
        },
    };

    // avatarDir 恒定存在：能解析出立绘目录就是相对 `data/` 的路径，解析不出
    // （没有 role_id，或剧本/插件角色本就没有立绘目录）就置 null。键位恒定，
    // 插件可以直接下标取值，与文档「该字段为 null」一致。
    payload["avatarDir"] = match resp.role_id {
        Some(role_id) => match avatar_dir(app, role_id).await {
            Some(dir) => serde_json::Value::String(dir),
            None => serde_json::Value::Null,
        },
        None => serde_json::Value::Null,
    };

    manager
        .dispatch_signal(app, SIGNAL_AI_REPLY, &payload, None)
        .await;
}

/// 角色立绘目录（相对 `data/`，URL 风格）。
///
/// 角色的**显示名和目录名不一定一样**（`resource_folder` 才是目录名），所以这里
/// 复用 [`RoleRepo::get_role_settings_by_id`]——它已经按角色类型（主角色 / 剧本 NPC /
/// 插件角色）解析好了目录，不用另写一套。
///
/// 拿不到就返回 `None`：插件少了 `avatarDir` 只是发不出表情包，不该影响文字本身。
async fn avatar_dir(app: &AppHandle, role_id: i32) -> Option<String> {
    let state = app.state::<AppState>();
    let data_dir = crate::api::data_dir();
    let settings = RoleRepo::get_role_settings_by_id(&state.db, &data_dir, role_id)
        .await
        .ok()
        .flatten()?;
    let dir = std::path::PathBuf::from(settings.resource_path?).join("avatar");
    let rel = dir.strip_prefix(&data_dir).ok()?;
    // 插件侧按 URL 风格拼路径，统一用 `/`
    Some(rel.to_string_lossy().replace('\\', "/"))
}
