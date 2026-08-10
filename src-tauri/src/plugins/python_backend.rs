//! RustPython 嵌入执行器：加载插件脚本、注入 ctx、调用 `run(ctx)`。
//!
//! 每个工具调用创建一个全新的 `Interpreter`（`Interpreter` 非 `Send`，
//! 无法跨线程共享），脚本在 `spawn_blocking` 线程内执行，外层由调用方
//! 用 `tokio::time::timeout` 兜底，超时直接丢弃整个解释器。

use std::collections::HashMap;
use std::path::Path;

use rustpython_vm::{
    AsObject, Interpreter, PyObjectRef, PyResult, VirtualMachine, compiler::Mode, py_serde,
    builtins::{PyBaseExceptionRef, PyDictRef},
};
use tauri::{AppHandle, Manager};

use serde_json::Value;

use crate::ai_service::tools::executor::ToolContext;
use crate::AppState;

use super::http_host;
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
        .add_native_module(http_host::plugin_module_def(&rustpython_vm::Context::genesis()))
        .build()
}

/// 把危险模块在 `sys.modules` 中置为 `None`，使后续 `import` 直接抛 ImportError。
fn block_dangerous_imports(vm: &VirtualMachine) -> PyResult<()> {
    let sys_modules = vm.sys_module.get_attr("modules", vm)?;
    let sys_modules: PyDictRef = sys_modules.downcast().map_err(|_| {
        vm.new_runtime_error("sys.modules 不是 dict")
    })?;
    for name in BLOCKED_MODULES {
        sys_modules.set_item(vm.ctx.intern_str(*name), vm.ctx.none(), vm)?;
    }
    Ok(())
}

/// 构造注入给脚本的 ctx 对象（Python dict）。
fn build_ctx(
    vm: &VirtualMachine,
    tool_name: &str,
    args: &Value,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
    app: AppHandle,
) -> PyResult<PyObjectRef> {
    let ctx = vm.ctx.new_dict();
    ctx.set_item(vm.ctx.intern_str("tool_name"), vm.ctx.new_str(tool_name).into(), vm)?;
    ctx.set_item(vm.ctx.intern_str("args"), http_host::value_to_pyobject(vm, args), vm)?;
    ctx.set_item(
        vm.ctx.intern_str("config"),
        http_host::value_to_pyobject(vm, &serde_json::to_value(config).unwrap_or(Value::Null)),
        vm,
    )?;
    // ctx.env 是 dict：白名单环境变量查询，脚本用 ctx.env.get("KEY")
    let env_dict = vm.ctx.new_dict();
    for (k, v) in env {
        env_dict.set_item(vm.ctx.intern_str(k.as_str()), vm.ctx.new_str(v.clone()).into(), vm)?;
    }
    ctx.set_item(vm.ctx.intern_str("env"), env_dict.into(), vm)?;
    // call_tool：让插件脚本调用任意已注册工具（内置或插件），返回其 JSON 结果
    ctx.set_item(vm.ctx.intern_str("call_tool"), make_call_tool(vm, app)?, vm)?;
    Ok(ctx.into())
}

/// 构造 `call_tool(name, args)` 原生函数，注入到 ctx。
///
/// 通过 AppHandle 取 ToolRegistry，在独立 runtime 内阻塞执行工具（脚本运行于
/// spawn_blocking 线程，无 tokio runtime 上下文），返回序列化后的 JSON dict。
fn make_call_tool(vm: &VirtualMachine, app: AppHandle) -> PyResult<PyObjectRef> {
    let app_for_fn = app.clone();
    let func = vm.new_function(
        "call_tool",
        move |name: String, args: PyObjectRef, vm: &VirtualMachine| -> PyResult<PyObjectRef> {
            let args_value = py_serde::serialize(vm, &args, serde_json::value::Serializer)
                .map_err(|e| vm.new_type_error(format!("call_tool 参数序列化失败: {e}")))?;
            let state = app_for_fn.state::<AppState>();
            let registry = state.data().tool_registry.clone();
            let tool = registry
                .get(&name)
                .ok_or_else(|| vm.new_value_error(format!("未知工具: {name}")))?;
            let allowed: std::collections::HashSet<String> =
                std::iter::once(name.clone()).collect();
            let context = ToolContext::new(allowed).with_app(app_for_fn.clone());
            let timeout = tool.timeout_hint().unwrap_or(std::time::Duration::from_secs(2));
            let result = http_host::runtime().block_on(async {
                tokio::time::timeout(timeout, tool.execute(&context, args_value)).await
            });
            match result {
                Ok(Ok(value)) => Ok(http_host::value_to_pyobject(vm, &value)),
                Ok(Err(e)) => Err(vm.new_value_error(format!("工具 {name} 执行失败: {e}"))),
                Err(_) => Err(vm.new_value_error(format!("工具 {name} 执行超时"))),
            }
        },
    );
    Ok(func.into())
}

/// 解析 manifest 声明的环境变量白名单，从宿主进程环境读取实际值。
pub(crate) fn collect_env(manifest: &PluginManifest) -> HashMap<String, String> {
    manifest
        .env
        .iter()
        .filter_map(|decl| {
            std::env::var(&decl.key).ok().map(|v| (decl.key.clone(), v))
        })
        .collect()
}

/// 执行插件脚本，调用 `run(ctx)` 并返回结果。
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
    let script = std::fs::read_to_string(script_path)
        .map_err(|e| format!("读取脚本失败: {e}"))?;
    let interpreter = build_interpreter();
    interpreter.enter(|vm| {
        let scope = vm.new_scope_with_builtins();
        let code = vm
            .compile(&script, Mode::Exec, script_path.display().to_string())
            .map_err(|e| format!("脚本编译失败: {e}"))?;
        vm.run_code_obj(code, scope.clone())
            .map_err(|e| format!("脚本执行失败: {}", exc_message(vm, &e)))?;

        // 顶层定义执行完毕后，拦截危险模块，再调用 run()
        block_dangerous_imports(vm).map_err(|e| exc_message(vm, &e))?;

        let ctx = build_ctx(vm, tool_name, args, config, env, app)
            .map_err(|e| exc_message(vm, &e))?;
        let run_func = scope
            .globals
            .get_item("run", vm)
            .map_err(|_| "脚本未定义 run(ctx) 函数".to_string())?;
        let result = run_func
            .call((ctx,), vm)
            .map_err(|e| format!("run() 调用失败: {}", exc_message(vm, &e)))?;
        py_serde::serialize(vm, &*result, serde_json::value::Serializer)
            .map_err(|e| format!("结果序列化失败: {e}"))
    })
}
