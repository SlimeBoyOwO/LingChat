//! 插件信号订阅：宿主登记信号，插件声明订阅，宿主按需派发。
//!
//! 与「工具」的差别：工具由 LLM 主动调用、返回值给模型；信号由宿主在业务点发出，
//! handler 的返回值被丢弃。因此订阅索引按「信号名 → 订阅列表」预建，派发时先查
//! 注册表、再按 `match` 在宿主侧筛选——只有命中的订阅才会新建解释器执行脚本。
//!
//! 已登记的信号：
//!
//! - [`SIGNAL_AI_REPLY`]：每条助手回复（自由对话、剧本固定台词、主动消息都会触发）
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

use super::types::{PluginRecord, SubscribeDecl};

/// 同时执行的 handler 上限。信号可能从任意业务点高频发出，不设上限会把
/// 阻塞线程打满。
const MAX_CONCURRENT_HANDLERS: usize = 4;

/// 每条助手回复。payload 与前端收到的 `ai:reply` 事件完全一致（camelCase）。
pub const SIGNAL_AI_REPLY: &str = "ai_reply";

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
}
