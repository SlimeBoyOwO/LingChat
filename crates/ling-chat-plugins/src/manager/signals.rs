//! 宿主信号：订阅索引重建与派发。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::PluginManager;
use crate::python_backend;
use crate::types::PluginRecord;

impl PluginManager {
    /// 按当前记录重建信号订阅索引。任何影响「哪些插件在跑」的操作之后都要调一次
    /// （重载 / 启停 / 删除），否则被禁用插件的 handler 仍会被派发。
    pub(super) fn rebuild_signal_index(&self, records: &HashMap<String, PluginRecord>) {
        let mut signals = self.signals.write().unwrap_or_else(|e| e.into_inner());
        // 「哪些插件在跑」的判定在 SignalRegistry::rebuild 内部，不在这里重复。
        signals.rebuild(records.values());
    }

    /// 是否有插件订阅了该信号。发射点用它先短路：没有订阅者时连载荷都不用准备。
    pub fn has_signal_subscribers(&self, signal: &str) -> bool {
        let signals = self.signals.read().unwrap_or_else(|e| e.into_inner());
        signals.has_subscribers(signal)
    }

    /// 把信号派发给命中的插件订阅。
    ///
    /// 筛选全在宿主侧完成：未登记的信号、`match` 不中的订阅都不会产生执行，
    /// 免得为不关心的插件白新建一个解释器。handler 的返回值按约定丢弃，执行失败
    /// 只记日志——信号是宿主业务的旁路，不能反过来影响发出方。
    ///
    /// `only_plugin` 给出时只派发给该插件的订阅：`ws_message` 用它把事件限定在
    /// 连接所属插件，避免别的订阅了同名信号的插件收到不相关的帧。
    pub async fn dispatch_signal(
        &self,
        app: &AppHandle,
        signal: &str,
        payload: &Value,
        only_plugin: Option<&str>,
    ) {
        let (subscriptions, slots) = {
            let signals = self.signals.read().unwrap_or_else(|e| e.into_inner());
            (signals.matching(signal, payload), signals.slots())
        };

        for sub in subscriptions {
            if only_plugin.is_some_and(|p| p != sub.plugin_id) {
                continue;
            }
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
            let plugin_dir = sub.dir.clone();
            let script_path = plugin_dir.join(&sub.decl.script);
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
                    python_backend::run_on_script_thread(move || {
                        let _permit = permit;
                        let manager = app.state::<Arc<PluginManager>>().inner().clone();
                        let run_env = manager.plugin_run_env(&plugin_id);
                        python_backend::run_plugin_handler(
                            &plugin_dir,
                            &script_path,
                            &handler,
                            &signal_name,
                            &payload,
                            run_env,
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
}
