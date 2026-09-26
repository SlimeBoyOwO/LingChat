//! 插件信号订阅：宿主登记信号，插件声明订阅，宿主按需派发。
//!
//! 与「工具」的差别：工具由 LLM 主动调用、返回值给模型；信号由宿主在业务点发出，
//! handler 的返回值被丢弃。因此订阅索引按「信号名 → 订阅列表」预建，派发时先查
//! 注册表、再按 `match` 在宿主侧筛选——只有命中的订阅才会新建解释器执行脚本。
//!
//! **宿主信号登记表当前为空**，即没有对插件承诺任何信号。接入新信号时：
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

/// 宿主登记的一条信号。
#[derive(Clone, Copy)]
pub struct SignalSpec {
    /// 信号名，派发与订阅都按它匹配。
    pub name: &'static str,
    /// 说明，供文档与插件页展示。当前注册表为空，暂无读取方。
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
        Self {
            specs: HashMap::new(),
            index: HashMap::new(),
            slots: Arc::new(Semaphore::new(MAX_CONCURRENT_HANDLERS)),
        }
    }

    /// 登记一个宿主信号。**当前没有任何调用**——信号登记表为空是刻意状态，
    /// 接入首个信号时在此登记（并删除这里的 allow）。
    #[allow(dead_code)]
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
                    // 从宽处理：注册表为空时（当前）所有订阅都走到这里。
                    // 信号上线后这些订阅自动生效，无需重装插件。
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::types::{MatchValue, PluginManifest, PluginState};
    use serde_json::json;

    const SCENE_SWITCH: SignalSpec = SignalSpec {
        name: "scene:switch",
        description: "切换场景后",
        fields: &["scene_id", "name"],
    };

    fn decl(signal: &str, matches: HashMap<String, MatchValue>) -> SubscribeDecl {
        SubscribeDecl {
            signal: signal.to_string(),
            script: "hook.py".to_string(),
            handler: "on_signal".to_string(),
            timeout_ms: 30_000,
            matches,
        }
    }

    fn record(id: &str, enabled: bool, subscribe: Vec<SubscribeDecl>) -> PluginRecord {
        PluginRecord {
            manifest: PluginManifest {
                id: id.to_string(),
                name: id.to_string(),
                subscribe,
                ..Default::default()
            },
            state: PluginState {
                enabled,
                ..Default::default()
            },
            dir: std::path::PathBuf::from(format!("/plugins/{id}")),
            error: None,
            startup_error: None,
        }
    }

    fn sub_with(matches: HashMap<String, MatchValue>) -> Subscription {
        Subscription {
            plugin_id: "p".into(),
            dir: std::path::PathBuf::from("/plugins/p"),
            decl: decl("scene:switch", matches),
        }
    }

    #[test]
    fn accepts_without_match_condition() {
        assert!(sub_with(HashMap::new()).accepts(&json!({})));
    }

    #[test]
    fn accepts_requires_present_and_equal_field() {
        let sub = sub_with(HashMap::from([(
            "scene_id".to_string(),
            MatchValue::One(json!("night")),
        )]));
        assert!(sub.accepts(&json!({ "scene_id": "night" })));
        assert!(!sub.accepts(&json!({ "scene_id": "day" })));
        // 字段缺失不命中，而不是当作通配
        assert!(!sub.accepts(&json!({})));
    }

    #[test]
    fn accepts_list_matches_any_member() {
        let sub = sub_with(HashMap::from([(
            "scene_id".to_string(),
            MatchValue::Many(vec![json!("night"), json!("rooftop")]),
        )]));
        assert!(sub.accepts(&json!({ "scene_id": "rooftop" })));
        assert!(!sub.accepts(&json!({ "scene_id": "cellar" })));
    }

    #[test]
    fn rebuild_indexes_only_enabled_plugins() {
        let mut registry = SignalRegistry::new();
        registry.register(SCENE_SWITCH);
        let records = vec![
            record("a", true, vec![decl("scene:switch", HashMap::new())]),
            record("b", false, vec![decl("scene:switch", HashMap::new())]),
        ];
        registry.rebuild(records.iter());
        let hits = registry.matching("scene:switch", &json!({}));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].plugin_id, "a");
    }

    #[test]
    fn unregistered_signal_never_dispatches() {
        let mut registry = SignalRegistry::new();
        let records = vec![record(
            "a",
            true,
            vec![decl("scene:switch", HashMap::new())],
        )];
        registry.rebuild(records.iter());
        // 订阅已建索引，但信号未登记 → 不派发
        assert!(registry.matching("scene:switch", &json!({})).is_empty());
    }

    #[test]
    fn rebuild_drops_subscription_after_disable() {
        let mut registry = SignalRegistry::new();
        registry.register(SCENE_SWITCH);
        let enabled = vec![record(
            "a",
            true,
            vec![decl("scene:switch", HashMap::new())],
        )];
        registry.rebuild(enabled.iter());
        assert_eq!(registry.matching("scene:switch", &json!({})).len(), 1);

        let disabled = vec![record(
            "a",
            false,
            vec![decl("scene:switch", HashMap::new())],
        )];
        registry.rebuild(disabled.iter());
        assert!(registry.matching("scene:switch", &json!({})).is_empty());
    }
}
