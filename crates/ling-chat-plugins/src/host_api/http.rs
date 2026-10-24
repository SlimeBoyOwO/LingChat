//! 插件 HTTP 能力：`http_get` / `http_post`。

use std::collections::HashMap;
use std::sync::OnceLock;

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};

use ling_chat_main::ai_service::llm::factory;

use super::{kwargs_map, py_to_value, runtime, value_to_pyobject};

/// 全局共享的 reqwest Client（连接池复用，进程内单例）。
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        factory::build_http_client(30).expect("构建插件 HTTP client 失败（rustls/webpki 配置错误）")
    })
}

/// 从 kwargs 提取 timeout_ms（毫秒），缺省 30s。
fn kw_timeout(vm: &VirtualMachine, kwargs: &HashMap<String, PyObjectRef>) -> u64 {
    kwargs
        .get("timeout_ms")
        .and_then(|v| py_to_value(vm, v).as_u64())
        .unwrap_or(30_000)
}

/// 组装 header / query 参数到请求。
fn apply_map_args(
    req: reqwest::RequestBuilder,
    vm: &VirtualMachine,
    kwargs: &HashMap<String, PyObjectRef>,
    key: &str,
) -> reqwest::RequestBuilder {
    let Some(value) = kwargs.get(key) else {
        return req;
    };
    let serde_json::Value::Object(map) = py_to_value(vm, value) else {
        return req;
    };
    map.iter().fold(req, |acc, (k, v)| {
        if let Some(s) = v.as_str() {
            if key == "headers" {
                acc.header(k.as_str(), s)
            } else {
                acc.query(&[(k.as_str(), s)])
            }
        } else {
            acc
        }
    })
}

/// 发送请求，把响应转成 Python dict 返回。
///
/// 插件脚本在独立线程内执行，线程上无 tokio runtime，
/// 用独立 runtime 的 `block_on` 阻塞等待，不会卡住 tokio runtime 主线程。
fn send_and_to_py(req: reqwest::RequestBuilder, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let json = runtime()
        .block_on(async {
            let resp = req
                .send()
                .await
                .map_err(|e| format!("HTTP 请求失败: {e}"))?;
            let status = resp.status();
            let text = resp
                .text()
                .await
                .map_err(|e| format!("读取响应失败: {e}"))?;
            let parsed = serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or_else(|_| serde_json::Value::String(text));
            Ok::<_, String>(serde_json::json!({
                "status": status.as_u16(),
                "ok": status.is_success(),
                "body": parsed,
            }))
        })
        .unwrap_or_else(|e| serde_json::json!({ "ok": false, "error": e }));
    Ok(value_to_pyobject(vm, &json))
}

/// `http_get(url, query={...}, headers={...}, timeout_ms=30000)`
pub(super) fn http_get(
    url: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let timeout = kw_timeout(vm, &kwargs);
    let req = client()
        .get(&url)
        .timeout(std::time::Duration::from_millis(timeout));
    let req = apply_map_args(req, vm, &kwargs, "headers");
    let req = apply_map_args(req, vm, &kwargs, "query");
    send_and_to_py(req, vm)
}

/// `http_post(url, headers={...}, body={...}, timeout_ms=30000)`
pub(super) fn http_post(
    url: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let timeout = kw_timeout(vm, &kwargs);
    let req = client()
        .post(&url)
        .timeout(std::time::Duration::from_millis(timeout));
    let req = apply_map_args(req, vm, &kwargs, "headers");
    let req = if let Some(body) = kwargs.get("body") {
        req.json(&py_to_value(vm, body))
    } else {
        req
    };
    send_and_to_py(req, vm)
}
