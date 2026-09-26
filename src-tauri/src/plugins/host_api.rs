//! 插件宿主原生模块 `plugin_host`：注入给插件 Python 脚本的全部宿主能力。
//!
//! 两类能力：
//! - **HTTP**：`http_get` / `http_post`，复用 `factory::build_http_client`
//!   （webpki-roots，Android 兼容），返回 `{ status, ok, body }`。
//! - **宿主控制**：`switch_character`，完整切换当前角色。
//!
//! 这是**插件专属**面——LLM 拿不到，与 `ctx["call_tool"]` 那条共用通道相对。
//! 新增宿主能力时，`#[pyfunction]` 直接加在下面的 `plugin_host` 模块里，
//! 实现放在本文件上方，不要另开文件——插件作者看到的是一个模块。

use std::collections::HashMap;
use std::sync::OnceLock;

use rustpython_derive::pymodule;
use rustpython_vm::{
    PyObjectRef, PyResult, VirtualMachine, builtins::PyListRef, function::KwArgs, py_serde,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::llm::factory;
use crate::db::managers::role_repo::RoleRepo;

/// 全局共享的 reqwest Client（连接池复用，进程内单例）。
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// 插件脚本在 spawn_blocking 线程执行，线程上无 tokio runtime 上下文；
/// 需要异步能力（reqwest / 宿主 async 命令）时，用这个独立多线程 runtime 驱动。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        factory::build_http_client(30).expect("构建插件 HTTP client 失败（rustls/webpki 配置错误）")
    })
}

pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .expect("构建插件 runtime 失败")
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
        },
        serde_json::Value::String(s) => vm.ctx.new_str(s.clone()).into(),
        serde_json::Value::Array(items) => {
            let list: PyListRef = vm
                .ctx
                .new_list(items.iter().map(|i| value_to_pyobject(vm, i)).collect());
            list.into()
        },
        serde_json::Value::Object(map) => {
            let dict = vm.ctx.new_dict();
            for (k, v) in map {
                let _ = dict.set_item(vm.ctx.intern_str(k.as_str()), value_to_pyobject(vm, v), vm);
            }
            dict.into()
        },
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

/// 取已加载角色的展示名（`settings.ai_name`，即对话里显示的名字）。
///
/// 锁顺序必须 `ai_service` → `game_status`（见 `ai_service::tools` 的约定）。
async fn loaded_display_name(app: &AppHandle, role_id: i32) -> Option<String> {
    let state = app.state::<AppState>();
    let gs = {
        let service = state.ai_service.lock().await;
        service.game_status.clone()
    };
    let gs = gs.lock().await;
    gs.role_manager
        .get_loaded(role_id)
        .and_then(|role| role.display_name.clone())
}

/// `plugin_host.switch_character` 的实现：完整切换当前角色。
///
/// 直接复用玩家侧的 `select_character`——它就是「正常切换」的全部语义：清空对话
/// 上下文、重置角色内存、递增会话边界代号丢弃旧流，最后产出整份 `WebInitData`。
/// 与 LLM 工具 `character_switch`（只换角色、保留历史）是两回事。
///
/// 插件调用没有前端调用方，所以额外把 `WebInitData` 广播给前端；不广播的话后端
/// 已经换了角色，前端还停在旧角色上。
async fn switch_character_impl(app: AppHandle, role_id: i32) -> serde_json::Value {
    // 先确认角色存在再动手：select_character 是先 clear_game_status() 再加载角色，
    // 传一个不存在的 id 会**先把对话清空**、然后才报错。玩家路径只会传合法 id，
    // 插件传的是脚本里的任意整数，所以这道校验必须前置。
    // 顺带取出角色标题（DB 的 role.name，即 settings.yml 的 title）。
    let title = {
        let state = app.state::<AppState>();
        match RoleRepo::get_role_by_id(&state.db, role_id).await {
            Ok(Some(role)) => role.name,
            Ok(None) => {
                return serde_json::json!({ "ok": false, "error": format!("角色 id {role_id} 不存在") });
            },
            Err(e) => {
                return serde_json::json!({ "ok": false, "error": format!("查询角色失败: {e}") });
            },
        }
    };

    match crate::api::game::select_character(app.clone(), role_id).await {
        Ok(init) => {
            // name 回对话里显示的 AI 名称（settings.yml 的 ai_name），与 LLM 工具
            // `character_switch` 一致；title 是角色标题（角色卡列表页那行大字）。
            // 界面上两处分别用它们，所以都给出来，插件不用再去读 settings.yml。
            let name = loaded_display_name(&app, role_id)
                .await
                .unwrap_or_else(|| title.clone());
            if let Err(e) = app.emit("character:full-switch", &init) {
                tracing::warn!("emit character:full-switch 失败: {e}");
            }
            serde_json::json!({ "ok": true, "role_id": role_id, "name": name, "title": title })
        },
        Err(e) => serde_json::json!({ "ok": false, "error": format!("切换角色失败: {e}") }),
    }
}

/// 插件宿主原生模块。插件脚本里用 `from plugin_host import ...` 取用。
#[pymodule]
mod plugin_host {
    use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};

    /// 执行 HTTP GET。
    ///
    /// 用法：`http_get(url, query={...}, headers={...}, timeout_ms=30000)`
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
    /// 用法：`http_post(url, headers={...}, body={...}, timeout_ms=30000)`
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

    /// 完整切换当前角色：清空对话上下文、重置角色内存、刷新前端。
    ///
    /// 用法：`switch_character(role_id)`
    ///
    /// 这是「正常切换」，**会丢弃当前对话历史**。只想换角色、保留历史请改用
    /// `ctx["call_tool"]("character_switch", {"id": ...})`。
    ///
    /// 返回：成功 `{ "ok": true, "role_id": 3, "name": "..." }`；
    /// 失败 `{ "ok": false, "error": "..." }`（不抛异常）。
    #[pyfunction]
    fn switch_character(role_id: i32, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::value_to_pyobject(
                vm,
                &serde_json::json!({ "ok": false, "error": "宿主句柄未初始化" }),
            ));
        };
        let result = super::runtime().block_on(super::switch_character_impl(app, role_id));
        Ok(super::value_to_pyobject(vm, &result))
    }
}

/// 获取插件宿主模块定义（供解释器注入）。
pub(crate) fn plugin_module_def(
    ctx: &rustpython_vm::Context,
) -> &'static rustpython_vm::builtins::PyModuleDef {
    plugin_host::module_def(ctx)
}
