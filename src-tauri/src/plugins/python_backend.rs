//! RustPython 嵌入执行器：加载插件脚本、注入 ctx、调用 `run(ctx)`。
//!
//! 每个工具调用创建一个全新的 `Interpreter`（`Interpreter` 非 `Send`，
//! 无法跨线程共享），脚本在 `spawn_blocking` 线程内执行，外层由调用方
//! 用 `tokio::time::timeout` 兜底，超时直接丢弃整个解释器。

use std::collections::HashMap;
use std::path::Path;

use rustpython_vm::{
    AsObject, Interpreter, PyObjectRef, PyResult, VirtualMachine,
    builtins::{PyBaseExceptionRef, PyDictRef},
    compiler::Mode,
    py_serde,
};
use tauri::{AppHandle, Manager};

use serde_json::Value;

use crate::AppState;
use crate::ai_service::tools::executor::{ToolContext, ToolExecutor};

use super::host_api;
use super::types::PluginManifest;

/// 沙箱拦截的顶层模块名：碰文件系统、跑命令、调底层 C 的一律禁止导入。
const BLOCKED_MODULES: &[&str] = &[
    "os",
    "subprocess",
    "shutil",
    "pathlib",
    "ctypes",
    "sysconfig",
];

/// 取 Python 异常的文本信息。
fn exc_message(vm: &VirtualMachine, e: &PyBaseExceptionRef) -> String {
    match e.as_object().str(vm) {
        Ok(s) => s.as_wtf8().to_string(),
        Err(_) => e.as_object().class().name().to_string(),
    }
}

/// 构造受限解释器。
///
/// 冻结标准库 + 注入宿主 `plugin_host` 模块。危险模块不在此拦截，
/// 而是等脚本顶层定义执行完、调用 `run()` 前再拦（见 `block_dangerous_imports`），
/// 避免影响脚本顶层对标准库内部依赖的正常导入。
fn build_interpreter() -> Interpreter {
    rustpython_vm::Interpreter::builder(rustpython_vm::Settings::default())
        .add_frozen_modules(rustpython_pylib::FROZEN_STDLIB)
        .add_native_module(host_api::plugin_module_def(
            &rustpython_vm::Context::genesis(),
        ))
        .build()
}

/// 把危险模块在 `sys.modules` 中置为 `None`，使后续 `import` 直接抛 ImportError。
fn block_dangerous_imports(vm: &VirtualMachine) -> PyResult<()> {
    let sys_modules = vm.sys_module.get_attr("modules", vm)?;
    let sys_modules: PyDictRef = sys_modules
        .downcast()
        .map_err(|_| vm.new_runtime_error("sys.modules 不是 dict"))?;
    for name in BLOCKED_MODULES {
        sys_modules.set_item(vm.ctx.intern_str(*name), vm.ctx.none(), vm)?;
    }
    Ok(())
}

/// 注入所有脚本共用的字段：`config` / `env` / `call_tool`。
fn inject_common(
    vm: &VirtualMachine,
    ctx: &PyDictRef,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
    app: AppHandle,
) -> PyResult<()> {
    ctx.set_item(
        vm.ctx.intern_str("config"),
        host_api::value_to_pyobject(vm, &serde_json::to_value(config).unwrap_or(Value::Null)),
        vm,
    )?;
    // ctx.env 是 dict：白名单环境变量查询，脚本用 ctx.env.get("KEY")
    let env_dict = vm.ctx.new_dict();
    for (k, v) in env {
        env_dict.set_item(
            vm.ctx.intern_str(k.as_str()),
            vm.ctx.new_str(v.clone()).into(),
            vm,
        )?;
    }
    ctx.set_item(vm.ctx.intern_str("env"), env_dict.into(), vm)?;
    // call_tool：让插件脚本调用任意已注册工具（内置或插件），返回其 JSON 结果
    ctx.set_item(vm.ctx.intern_str("call_tool"), make_call_tool(vm, app)?, vm)?;
    Ok(())
}

/// 构造工具调用注入给脚本的 ctx 对象（Python dict）。
fn build_tool_ctx(
    vm: &VirtualMachine,
    tool_name: &str,
    args: &Value,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
    app: AppHandle,
) -> PyResult<PyObjectRef> {
    let ctx = vm.ctx.new_dict();
    ctx.set_item(
        vm.ctx.intern_str("tool_name"),
        vm.ctx.new_str(tool_name).into(),
        vm,
    )?;
    ctx.set_item(
        vm.ctx.intern_str("args"),
        host_api::value_to_pyobject(vm, args),
        vm,
    )?;
    inject_common(vm, &ctx, config, env, app)?;
    Ok(ctx.into())
}

/// 构造信号 handler 注入给脚本的 ctx 对象（Python dict）。
///
/// 与工具 ctx 形状一致，只把 `tool_name` / `args` 换成 `signal` / `payload`，
/// 插件作者只需要学一套注入字段。
fn build_signal_ctx(
    vm: &VirtualMachine,
    signal: &str,
    payload: &Value,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
    app: AppHandle,
) -> PyResult<PyObjectRef> {
    let ctx = vm.ctx.new_dict();
    ctx.set_item(
        vm.ctx.intern_str("signal"),
        vm.ctx.new_str(signal).into(),
        vm,
    )?;
    ctx.set_item(
        vm.ctx.intern_str("payload"),
        host_api::value_to_pyobject(vm, payload),
        vm,
    )?;
    inject_common(vm, &ctx, config, env, app)?;
    Ok(ctx.into())
}

/// 构造 `call_tool(name, args)` 原生函数，注入到 ctx。
///
/// 通过 AppHandle 取 ToolRegistry，在独立 runtime 内走 `ToolExecutor`（与 LLM
/// 侧同一条路径）：参数按 schema 校验，未知工具 / 参数非法 / 超时统一编成
/// `{ ok: false, error: { code, message } }` JSON 信封返回给脚本，而不是抛异常。
fn make_call_tool(vm: &VirtualMachine, app: AppHandle) -> PyResult<PyObjectRef> {
    let app_for_fn = app.clone();
    let func = vm.new_function(
        "call_tool",
        move |name: String, args: PyObjectRef, vm: &VirtualMachine| -> PyResult<PyObjectRef> {
            let args_value = py_serde::serialize(vm, &args, serde_json::value::Serializer)
                .map_err(|e| vm.new_type_error(format!("call_tool 参数序列化失败: {e}")))?;
            let args_json = args_value.to_string();
            let state = app_for_fn.state::<AppState>();
            let registry = state.data().tool_registry.clone();
            let allowed: std::collections::HashSet<String> =
                std::iter::once(name.clone()).collect();
            let context = ToolContext::new(allowed).with_app(app_for_fn.clone());
            let result_json = host_api::runtime().block_on(async move {
                ToolExecutor::new(&registry)
                    .execute(&name, &args_json, &context)
                    .await
            });
            let parsed: serde_json::Value = serde_json::from_str(&result_json)
                .unwrap_or_else(|_| serde_json::Value::String(result_json));
            Ok(host_api::value_to_pyobject(vm, &parsed))
        },
    );
    Ok(func.into())
}

/// 解析 manifest 声明的环境变量白名单，从宿主进程环境读取实际值。
pub(crate) fn collect_env(manifest: &PluginManifest) -> HashMap<String, String> {
    manifest
        .env
        .iter()
        .filter_map(|decl| std::env::var(&decl.key).ok().map(|v| (decl.key.clone(), v)))
        .collect()
}

/// 公共执行骨架：建解释器 → 跑脚本顶层 → 拦截危险模块 → 调用入口函数。
///
/// `collect_result` 为 false 时不序列化返回值（信号 handler 的返回值按约定丢弃，
/// 不该因为返回了不可序列化的对象而报错）。
fn run_entry(
    script_path: &Path,
    entry: &str,
    collect_result: bool,
    app: AppHandle,
    build_ctx: impl FnOnce(&VirtualMachine, AppHandle) -> PyResult<PyObjectRef>,
) -> Result<Option<Value>, String> {
    let script = std::fs::read_to_string(script_path).map_err(|e| format!("读取脚本失败: {e}"))?;
    let interpreter = build_interpreter();
    interpreter.enter(|vm| {
        let scope = vm.new_scope_with_builtins();
        let code = vm
            .compile(&script, Mode::Exec, script_path.display().to_string())
            .map_err(|e| format!("脚本编译失败: {e}"))?;
        vm.run_code_obj(code, scope.clone())
            .map_err(|e| format!("脚本执行失败: {}", exc_message(vm, &e)))?;

        // 顶层定义执行完毕后，拦截危险模块，再调用入口函数
        block_dangerous_imports(vm).map_err(|e| exc_message(vm, &e))?;

        let ctx = build_ctx(vm, app).map_err(|e| exc_message(vm, &e))?;
        let func = scope
            .globals
            .get_item(entry, vm)
            .map_err(|_| format!("脚本未定义 {entry}(ctx) 函数"))?;
        let result = func
            .call((ctx,), vm)
            .map_err(|e| format!("{entry}() 调用失败: {}", exc_message(vm, &e)))?;
        if !collect_result {
            return Ok(None);
        }
        py_serde::serialize(vm, &*result, serde_json::value::Serializer)
            .map(Some)
            .map_err(|e| format!("结果序列化失败: {e}"))
    })
}

/// 执行工具脚本，调用 `run(ctx)` 并返回结果。
///
/// 必须在 `spawn_blocking` 内调用（`Interpreter::enter` 需要线程局部状态）。
pub(crate) fn run_plugin_script(
    script_path: &Path,
    tool_name: &str,
    args: &Value,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
    app: AppHandle,
) -> Result<Value, String> {
    run_entry(script_path, "run", true, app, |vm, app| {
        build_tool_ctx(vm, tool_name, args, config, env, app)
    })
    .map(|result| result.unwrap_or(Value::Null))
}

/// 执行插件的启动入口，调用 `handler(ctx)`。
///
/// ctx 只有 `config` / `env` / `call_tool`——启动没有信号名与载荷。返回值按约定丢弃。
pub(crate) fn run_plugin_startup(
    script_path: &Path,
    handler: &str,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(script_path, handler, false, app, |vm, app| {
        let ctx = vm.ctx.new_dict();
        inject_common(vm, &ctx, config, env, app)?;
        Ok(ctx.into())
    })
    .map(|_| ())
}

/// 执行插件的信号 handler，调用 `handler(ctx)`。
///
/// 返回值按约定丢弃——信号没有下游消费者，结果只用于让宿主感知执行失败。
/// 同样必须在 `spawn_blocking` 内调用。
pub(crate) fn run_plugin_handler(
    script_path: &Path,
    handler: &str,
    signal: &str,
    payload: &Value,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(script_path, handler, false, app, |vm, app| {
        build_signal_ctx(vm, signal, payload, config, env, app)
    })
    .map(|_| ())
}
