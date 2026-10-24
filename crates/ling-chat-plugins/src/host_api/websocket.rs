//! 脚本侧 WebSocket 控制：打开、关闭、发送、请求、查询。

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, builtins::PyBytes, function::KwArgs};
use tauri::Manager;

use super::guard::current_plugin;
use super::{err_json, kwargs_map, no_host, py_to_value, result_to_py, runtime, value_to_pyobject};

/// `ws_request` 默认等待时长。
const WS_REQUEST_TIMEOUT_MS: u64 = 5000;
/// 下限：再短本机回环也可能来不及。
const WS_REQUEST_TIMEOUT_MIN_MS: u64 = 100;
/// 上限：工具自身 `timeout_ms` 最高 120000，留出余量免得被外层超时先杀掉。
const WS_REQUEST_TIMEOUT_MAX_MS: u64 = 115_000;

pub(super) fn ws_open(
    conn_id: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let url = kwargs
        .get("url")
        .and_then(|v| py_to_value(vm, v).as_str().map(str::to_string));
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "无法确定当前插件"));
    };
    let manager = app
        .state::<std::sync::Arc<crate::PluginManager>>()
        .inner()
        .clone();
    let result = runtime().block_on(manager.ws_open(&plugin_id, Some(&conn_id), url.as_deref()));
    Ok(result_to_py(vm, result))
}

pub(super) fn ws_close(conn_id: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "无法确定当前插件"));
    };
    let manager = app
        .state::<std::sync::Arc<crate::PluginManager>>()
        .inner()
        .clone();
    manager.ws_close(&plugin_id, Some(&conn_id));
    Ok(value_to_pyobject(vm, &serde_json::json!({ "ok": true })))
}

pub(super) fn ws_send(
    conn_id: String,
    data: PyObjectRef,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "无法确定当前插件"));
    };
    // clone 廉价（引用计数）；downcast 会消耗 receiver，之后还要读 data。
    let cmd = match data.clone().downcast::<PyBytes>() {
        Ok(bytes) => crate::ws::WsCommand::Binary(bytes.as_bytes().to_vec()),
        // 其余（含 str）：转成 JSON 值——str 得到文本原样，dict/数字得到 JSON 文本。
        Err(_) => match py_to_value(vm, &data) {
            serde_json::Value::String(s) => crate::ws::WsCommand::Text(s),
            other => crate::ws::WsCommand::Text(other.to_string()),
        },
    };
    let manager = app
        .state::<std::sync::Arc<crate::PluginManager>>()
        .inner()
        .clone();
    let result = manager.ws_send(&plugin_id, &conn_id, cmd);
    Ok(result_to_py(vm, result))
}

pub(super) fn ws_request(
    conn_id: String,
    data: PyObjectRef,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let timeout_ms = kwargs
        .get("timeout_ms")
        .and_then(|v| py_to_value(vm, v).as_u64())
        .unwrap_or(WS_REQUEST_TIMEOUT_MS)
        .clamp(WS_REQUEST_TIMEOUT_MIN_MS, WS_REQUEST_TIMEOUT_MAX_MS);

    let mut payload = py_to_value(vm, &data);
    let Some(map) = payload.as_object_mut() else {
        return Ok(err_json(vm, "ws_request 的 data 必须是对象"));
    };
    let request_id = uuid::Uuid::new_v4().to_string();
    map.insert(
        "request_id".to_string(),
        serde_json::Value::String(request_id.clone()),
    );

    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "无法确定当前插件"));
    };
    let manager = app
        .state::<std::sync::Arc<crate::PluginManager>>()
        .inner()
        .clone();
    let rx = match manager.ws_request(&plugin_id, &conn_id, request_id, payload.to_string()) {
        Ok(rx) => rx,
        Err(e) => return Ok(err_json(vm, &e)),
    };

    let waited = runtime().block_on(async move {
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), rx).await
    });
    let out = match waited {
        Ok(Ok(response)) => serde_json::json!({ "ok": true, "response": response }),
        // 通道被丢弃 = 连接断了（连接任务退出时清空等待表）。
        Ok(Err(_)) => serde_json::json!({ "ok": false, "error": "连接已断开，请求未得到响应" }),
        // 单人游戏按 Esc 暂停时游戏 tick 停止，模组不处理任何请求，表现就是全部超时。
        Err(_) => serde_json::json!({
            "ok": false,
            "error": "请求超时，游戏可能处于暂停或未响应状态",
        }),
    };
    Ok(value_to_pyobject(vm, &out))
}

pub(super) fn ws_status(kwargs: KwArgs<PyObjectRef>, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let conn_id = kwargs
        .get("conn_id")
        .and_then(|v| py_to_value(vm, v).as_str().map(str::to_string));
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "无法确定当前插件"));
    };
    let manager = app
        .state::<std::sync::Arc<crate::PluginManager>>()
        .inner()
        .clone();
    match manager.ws_status(&plugin_id, conn_id.as_deref()) {
        // 指定 conn_id 时 manager 保证非空，取首条。
        Ok(list) if conn_id.is_some() => {
            let c = &list[0];
            Ok(value_to_pyobject(
                vm,
                &serde_json::json!({ "ok": true, "id": c.id, "mode": c.mode, "state": c.state }),
            ))
        },
        Ok(list) => Ok(value_to_pyobject(
            vm,
            &serde_json::json!({ "ok": true, "connections": list }),
        )),
        Err(e) => Ok(err_json(vm, &e)),
    }
}
