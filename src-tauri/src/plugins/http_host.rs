//! 注入给插件 Python 脚本的 HTTP 宿主能力。
//!
//! 通过 `#[pymodule]` 暴露 `http_get` / `http_post` 原生函数，
//! 复用 `factory::build_http_client`（webpki-roots，Android 兼容）。
//! 返回 Python dict：`{ status, ok, body }`，body 为解析后的 JSON。

use std::collections::HashMap;
use std::sync::OnceLock;

use rustpython_derive::pymodule;
use rustpython_vm::{
    PyObjectRef, PyResult, VirtualMachine, builtins::PyListRef, function::KwArgs, py_serde,
};

use crate::ai_service::llm::factory;

/// 全局共享的 reqwest Client（连接池复用，进程内单例）。
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// 插件脚本在 spawn_blocking 线程执行，线程上无 tokio runtime 上下文；
/// reqwest 是异步 client，需要 tokio reactor，这里用独立多线程 runtime 驱动。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        factory::build_http_client(30)
            .expect("构建插件 HTTP client 失败（rustls/webpki 配置错误）")
    })
}

pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .expect("构建插件 HTTP runtime 失败")
    })
}

/// 把 Python 对象转成 serde_json::Value（用于解析 kwargs 里的 headers/body）。
fn py_to_value(vm: &VirtualMachine, obj: &PyObjectRef) -> serde_json::Value {
    py_serde::serialize(vm, &**obj, serde_json::value::Serializer)
        .unwrap_or(serde_json::Value::Null)
}

/// 递归把 serde_json::Value 转成 Python 对象（返回给插件脚本）。
pub(crate) fn value_to_pyobject(vm: &VirtualMachine, value: &serde_json::Value) -> PyObjectRef {
    match value {
        serde_json::Value::Null => vm.ctx.none(),
        serde_json::Value::Bool(b) => vm.ctx.new_bool(*b).into(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                vm.ctx.new_int(i).into()
            } else {
                vm.ctx.new_float(n.as_f64().unwrap_or(0.0)).into()
            }
        }
        serde_json::Value::String(s) => vm.ctx.new_str(s.clone()).into(),
        serde_json::Value::Array(items) => {
            let list: PyListRef =
                vm.ctx.new_list(items.iter().map(|i| value_to_pyobject(vm, i)).collect());
            list.into()
        }
        serde_json::Value::Object(map) => {
            let dict = vm.ctx.new_dict();
            for (k, v) in map {
                let _ = dict.set_item(vm.ctx.intern_str(k.as_str()), value_to_pyobject(vm, v), vm);
            }
            dict.into()
        }
    }
}

/// 把 kwargs 收进 HashMap 便于按键访问。
fn kwargs_map(kwargs: KwArgs<PyObjectRef>) -> HashMap<String, PyObjectRef> {
    kwargs.into_iter().collect()
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
/// 插件脚本在 `spawn_blocking` 线程内执行，线程上无 tokio runtime，
/// 用独立 runtime 的 `block_on` 阻塞等待，不会卡住 tokio runtime 主线程。
fn send_and_to_py(req: reqwest::RequestBuilder, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let json = runtime().block_on(async {
        let resp = req.send().await.map_err(|e| format!("HTTP 请求失败: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("读取响应失败: {e}"))?;
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

/// 插件宿主原生模块。
#[pymodule]
mod plugin_host {
    use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};

    /// 执行 HTTP GET。
    ///
    /// 用法：`ctx.http_get(url, query={...}, headers={...}, timeout_ms=30000)`
    #[pyfunction]
    fn http_get(
        url: String,
        kwargs: KwArgs<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        let kwargs = super::kwargs_map(kwargs);
        let timeout = super::kw_timeout(vm, &kwargs);
        let req = super::client()
            .get(&url)
            .timeout(std::time::Duration::from_millis(timeout));
        let req = super::apply_map_args(req, vm, &kwargs, "headers");
        let req = super::apply_map_args(req, vm, &kwargs, "query");
        super::send_and_to_py(req, vm)
    }

    /// 执行 HTTP POST（JSON body）。
    ///
    /// 用法：`ctx.http_post(url, headers={...}, body={...}, timeout_ms=30000)`
    #[pyfunction]
    fn http_post(
        url: String,
        kwargs: KwArgs<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        let kwargs = super::kwargs_map(kwargs);
        let timeout = super::kw_timeout(vm, &kwargs);
        let req = super::client()
            .post(&url)
            .timeout(std::time::Duration::from_millis(timeout));
        let req = super::apply_map_args(req, vm, &kwargs, "headers");
        let req = if let Some(body) = kwargs.get("body") {
            req.json(&super::py_to_value(vm, body))
        } else {
            req
        };
        super::send_and_to_py(req, vm)
    }
}

/// 获取插件宿主模块定义（供解释器注入）。
pub(crate) fn plugin_module_def(ctx: &rustpython_vm::Context) -> &'static rustpython_vm::builtins::PyModuleDef {
    plugin_host::module_def(ctx)
}
