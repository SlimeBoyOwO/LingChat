//! 前置插件判定：纯函数，不碰管理器状态。

use std::collections::{HashMap, HashSet};

use crate::types::PluginRecord;

/// 把前置插件分成「未安装的」与「已安装但不可用的」。
///
/// `installed` / `unavailable` 都以插件目录名（= 插件 id）为准
pub(super) fn classify_deps(
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
pub(super) fn unmet_dependencies(
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
pub(super) fn unrunnable_in_order(
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
pub(super) fn dependency_error(missing: &[String], inactive: &[String]) -> String {
    if !missing.is_empty() {
        format!("PLUGIN_MISSING_DEPENDENCY|{}", missing.join(", "))
    } else {
        format!("PLUGIN_INACTIVE_DEPENDENCY|{}", inactive.join(", "))
    }
}
