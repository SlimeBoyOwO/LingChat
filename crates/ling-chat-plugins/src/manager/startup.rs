//! 启动入口调度、重试与依赖级联禁用。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use super::PluginManager;
use super::deps::unrunnable_in_order;
use crate::python_backend;
use crate::types::StartupDecl;

impl PluginManager {
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
            let records = self.store.lock().await;
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
                let records = self.store.lock().await;
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
            let records = self.store.lock().await;
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
            let mut records = self.store.lock().await;
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
        let plugin_dir = dir.to_path_buf();
        let script_path = plugin_dir.join(&decl.script);
        let handler = decl.handler.clone();
        let timeout = Duration::from_millis(decl.timeout_ms);
        let plugin_id = id.to_string();
        let app_handle = app.clone();
        let token = token.clone();

        let joined = tokio::time::timeout(
            timeout,
            python_backend::run_on_script_thread(move || {
                // 起解释器前最后一道闸：刚被停用的插件不再白跑一次。
                // 注意已经进到脚本里的执行无法中断，只能等它返回后丢弃结果。
                if token.is_cancelled() {
                    return Err("插件已被停用".to_string());
                }
                let manager = app_handle.state::<Arc<PluginManager>>().inner().clone();
                let run_env = manager.plugin_run_env(&plugin_id);
                python_backend::run_plugin_startup(
                    &plugin_dir,
                    &script_path,
                    &handler,
                    run_env,
                    app_handle,
                )
            }),
        )
        .await;

        match joined {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(e),
            Err(_) => Err(format!("执行超时（{}ms）", timeout.as_millis())),
        }
    }

    /// 取出当前待调度插件里「前置未就绪」的那批（含传递依赖），返回 (id, 错误码)。
    ///
    /// 判定本身在 `deps::unrunnable_in_order` 里（纯函数，内部跑到不动点）；这里只负责
    /// 把 records 拍成它的三个输入。
    async fn unrunnable_pending(
        &self,
        pending: &HashMap<String, (PathBuf, Option<StartupDecl>)>,
    ) -> Vec<(String, String)> {
        // 以 map 的键（= 插件目录名）为准，不用 `manifest.id`：manifest 解析失败的
        // 记录里 `manifest.id` 是空的，拿它比对会把「装坏了的前置」误报成「没安装」。
        let (installed, unavailable, depends) = {
            let records = self.store.lock().await;
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
            let mut records = self.store.lock().await;
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
                let records = self.store.lock().await;
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
    pub(super) async fn cancel_plugin(&self, id: &str) {
        if let Some(token) = self.cancels.lock().await.remove(id) {
            token.cancel();
        }
    }

    /// 插件是否已停用（令牌不存在，或已被取消）。
    pub(super) async fn is_stopped(&self, id: &str) -> bool {
        self.live_token(id).await.is_none()
    }
}
