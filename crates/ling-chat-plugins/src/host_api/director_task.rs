//! 插件向导演 Agent 投递后台任务：投递、查进度、取结果。

use std::collections::HashMap;
use std::sync::Arc;

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use ling_chat_main::AppState;
use ling_chat_main::ai_service::god_agent::GodAgentCore;
use ling_chat_main::ai_service::god_agent::task::{self, TaskStatus};

use super::guard::{current_director_task_allow, current_plugin};
use super::{err_json, kwargs_map, no_host, py_to_value, runtime, value_to_pyobject};
use crate::PluginManager;
use crate::signal::SIGNAL_DIRECTOR_TASK;

/// 插件交给导演的请求体积上限。现场快照也会塞在里面，留得比普通工具参数宽。
const MAX_REQUEST_BYTES: usize = 64 * 1024;

fn task_id_arg(kwargs: &HashMap<String, PyObjectRef>, vm: &VirtualMachine) -> Option<String> {
    kwargs
        .get("task_id")
        .and_then(|value| {
            py_to_value(vm, value)
                .as_str()
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|value| !value.is_empty())
}

pub(super) fn director_task_start(
    kind: String,
    request: PyObjectRef,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_director_task_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明 director_task 能力（需在 manifest.toml 顶层写 director_task = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin) = current_plugin() else {
        return Ok(err_json(vm, "宿主不知道当前是哪个插件"));
    };
    let request = py_to_value(vm, &request);
    if !request.is_object() {
        return Ok(err_json(vm, "request 必须是 dict"));
    }
    let size = serde_json::to_vec(&request)
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    if size > MAX_REQUEST_BYTES {
        return Ok(err_json(
            vm,
            &format!("request 过大（{size} 字节，上限 {MAX_REQUEST_BYTES} 字节）"),
        ));
    }

    let result = runtime().block_on(start_impl(app, plugin, kind, request));
    Ok(value_to_pyobject(vm, &result))
}

async fn start_impl(app: AppHandle, plugin: String, kind: String, request: Value) -> Value {
    let Some(core) = app.state::<AppState>().god_agent.clone() else {
        return json!({ "ok": false, "error": "上帝Agent 不可用：未配置可用的模型" });
    };
    let core_for_wait = core.clone();
    match task::start(core, app.clone(), plugin.clone(), kind, request).await {
        Ok(task_id) => {
            spawn_waiter(app, core_for_wait, plugin, task_id.clone());
            json!({ "ok": true, "task_id": task_id })
        },
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

/// 任务进入终态后把结果派回发起它的插件。信号只是通知，蓝图仍由插件按 id 取。
fn spawn_waiter(app: AppHandle, core: Arc<GodAgentCore>, plugin: String, task_id: String) {
    tauri::async_runtime::spawn(async move {
        core.tasks.wait_done(&task_id).await;
        let Some(snapshot) = core.tasks.snapshot(&plugin, Some(&task_id)).await else {
            return;
        };
        let manager = app.state::<Arc<PluginManager>>().inner().clone();
        if !manager.has_signal_subscribers(SIGNAL_DIRECTOR_TASK) {
            return;
        }
        let payload = json!({
            "taskId": snapshot.task_id,
            "kind": snapshot.kind,
            "status": snapshot.status,
            "summary": snapshot.summary,
            "hasResult": snapshot.has_result,
            "error": snapshot.error,
        });
        manager
            .dispatch_signal(&app, SIGNAL_DIRECTOR_TASK, &payload, Some(&plugin))
            .await;
    });
}

pub(super) fn director_task_status(
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_director_task_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明 director_task 能力（需在 manifest.toml 顶层写 director_task = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin) = current_plugin() else {
        return Ok(err_json(vm, "宿主不知道当前是哪个插件"));
    };
    let kwargs = kwargs_map(kwargs);
    let task_id = task_id_arg(&kwargs, vm);
    let result = runtime().block_on(status_impl(app, plugin, task_id));
    Ok(value_to_pyobject(vm, &result))
}

async fn status_impl(app: AppHandle, plugin: String, task_id: Option<String>) -> Value {
    let Some(core) = app.state::<AppState>().god_agent.clone() else {
        return json!({ "ok": false, "error": "上帝Agent 不可用：未配置可用的模型" });
    };
    match core.tasks.snapshot(&plugin, task_id.as_deref()).await {
        Some(snapshot) => json!({
            "ok": true,
            "task": serde_json::to_value(snapshot).unwrap_or(Value::Null),
        }),
        None => json!({ "ok": true, "task": null }),
    }
}

pub(super) fn director_task_take(
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_director_task_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明 director_task 能力（需在 manifest.toml 顶层写 director_task = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin) = current_plugin() else {
        return Ok(err_json(vm, "宿主不知道当前是哪个插件"));
    };
    let kwargs = kwargs_map(kwargs);
    let task_id = task_id_arg(&kwargs, vm);
    let result = runtime().block_on(take_impl(app, plugin, task_id));
    Ok(value_to_pyobject(vm, &result))
}

async fn take_impl(app: AppHandle, plugin: String, task_id: Option<String>) -> Value {
    let Some(core) = app.state::<AppState>().god_agent.clone() else {
        return json!({ "ok": false, "error": "上帝Agent 不可用：未配置可用的模型" });
    };
    let Some(snapshot) = core.tasks.snapshot(&plugin, task_id.as_deref()).await else {
        return json!({ "ok": false, "error": "没有这个任务" });
    };
    if snapshot.status != TaskStatus::Done {
        return json!({ "ok": false, "error": "任务还没有完成" });
    }
    match core.tasks.take_result(&plugin, task_id.as_deref()).await {
        Some(result) => {
            let bytes = serde_json::to_vec(&result.result)
                .map(|value| value.len())
                .unwrap_or(0);
            tracing::info!(task = %result.task_id, bytes, "插件取走导演任务结果");
            json!({
                "ok": true,
                "taskId": result.task_id,
                "kind": result.kind,
                "summary": result.summary,
                "result": result.result,
            })
        },
        None => json!({ "ok": false, "error": "蓝图已经被取走或没有结果" }),
    }
}
