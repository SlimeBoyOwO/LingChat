//! RustPython 嵌入执行器：加载插件脚本、注入 ctx、调用 `run(ctx)`。
//!
//! 每个工具调用创建一个全新的 `Interpreter`（`Interpreter` 非 `Send`，
//! 无法跨线程共享），脚本在 `spawn_blocking` 线程内执行，外层由调用方
//! 用 `tokio::time::timeout` 兜底，超时直接丢弃整个解释器。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use rustpython_vm::{
    AsObject, Interpreter, PyObjectRef, PyResult, VirtualMachine,
    builtins::{PyBaseExceptionRef, PyDictRef},
    compiler::Mode,
    py_serde,
};
use tauri::{AppHandle, Manager};

use serde_json::Value;

use ling_chat_main::AppState;
use ling_chat_main::ai_service::tools::executor::{ToolContext, ToolExecutor};
use ling_chat_main::ai_service::tools::registry::ToolRegistry;

use super::host_api;
use super::types::{PluginManifest, PluginRunEnv};

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
            rustpython_vm::Context::genesis(),
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
    run_env: &PluginRunEnv,
    app: AppHandle,
) -> PyResult<()> {
    ctx.set_item(
        vm.ctx.intern_str("config"),
        host_api::value_to_pyobject(
            vm,
            &serde_json::to_value(&run_env.config).unwrap_or(Value::Null),
        ),
        vm,
    )?;
    // ctx.env 是 dict：白名单环境变量查询，脚本用 ctx.env.get("KEY")
    let env_dict = vm.ctx.new_dict();
    for (k, v) in &run_env.env {
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
    run_env: &PluginRunEnv,
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
    inject_common(vm, &ctx, run_env, app)?;
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
    run_env: &PluginRunEnv,
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
    inject_common(vm, &ctx, run_env, app)?;
    Ok(ctx.into())
}

// 插件脚本可用工具白名单：由 `run_entry` 按调用来源设置，`call_tool` 读取。
//
// `Some(set)` = 本次执行处在某个 LLM 工具闭环里（插件工具被模型调用），
// 直接沿用外层 `ToolContext` 的权限集合；`None` = 信号 handler / 启动入口
// 这类没有场景上下文的调用，`call_tool` 按权限矩阵的无场景兜底策略计算。
thread_local! {
    static TOOL_ALLOW: RefCell<Option<HashSet<String>>> = const { RefCell::new(None) };
}

/// 设置当前线程的工具白名单；Drop 时还原（见 `TOOL_ALLOW`）。
struct ToolAllowGuard {
    prev: Option<HashSet<String>>,
}

impl Drop for ToolAllowGuard {
    fn drop(&mut self) {
        TOOL_ALLOW.with(|cell| *cell.borrow_mut() = self.prev.take());
    }
}

/// 设置当前线程的工具白名单，返回还原用 guard。
fn set_tool_allow(allowed: Option<HashSet<String>>) -> ToolAllowGuard {
    let prev = TOOL_ALLOW.with(|cell| cell.replace(allowed));
    ToolAllowGuard { prev }
}

/// 读取当前线程继承的工具白名单；`None` 表示没有外层工具上下文。
fn current_tool_allow() -> Option<HashSet<String>> {
    TOOL_ALLOW.with(|cell| cell.borrow().clone())
}

/// 无外层工具上下文时的兜底权限：当前角色在**所有已启用场景组**里允许的工具并集。
///
/// 与 LLM 路径共用同一份权限矩阵（`tool_permissions.toml`），区别只是不绑定场景；
/// 因此被设置页关掉的工具组、未归入任何角色组的角色，在这条路径上同样拿不到权限。
///
/// 角色身份用 `try_lock` 读取：兜底路径的调用方可能是启动入口/信号 handler，
/// 拿不到锁就退化为「无当前角色」（按 default 角色组计算），绝不因抢锁挂死插件。
fn fallback_tool_allow(registry: &ToolRegistry, app: &AppHandle) -> HashSet<String> {
    let state = app.state::<AppState>();
    // 整个「抢锁读当前角色」包成一个闭包：MutexGuard 临时量在闭包内释放，
    // 不会活过局部绑定（否则块尾表达式的临时量会触发 E0597）。
    let role_name = (|| {
        let service = state.ai_service.try_lock().ok()?;
        let game_status = service.game_status.clone();
        drop(service);
        let gs = game_status.try_lock().ok()?;
        let role_id = gs.current_role_id?;
        gs.role_manager
            .get_loaded(role_id)
            .and_then(|role| role.display_name.clone())
    })();
    registry.allowed_tools_unscoped(role_name.as_deref())
}

/// 构造 `call_tool(name, args)` 原生函数，注入到 ctx。
///
/// 通过 AppHandle 取 ToolRegistry，在独立 runtime 内走 `ToolExecutor`（与 LLM
/// 侧同一条路径）：参数按 schema 校验，未知工具 / 参数非法 / 超时统一编成
/// `{ ok: false, error: { code, message } }` JSON 信封返回给脚本，而不是抛异常。
///
/// **权限**：可调用集合由 `TOOL_ALLOW` 决定——插件工具执行时继承外层工具上下文，
/// 信号/启动入口按权限矩阵的无场景并集兜底（见 [`fallback_tool_allow`]），
/// 不再无条件放行。
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
            let app = app_for_fn.clone();
            let result_json = host_api::runtime().block_on(async move {
                let allowed = match current_tool_allow() {
                    Some(allowed) => allowed,
                    None => fallback_tool_allow(&registry, &app),
                };
                let context = ToolContext::new(allowed).with_app(app);
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
///
/// `allowed` 是本次执行 `call_tool` 的工具白名单：插件工具执行传外层
/// `ToolContext::allowed_tools`，信号/启动入口传 `None`（由 `call_tool` 走权限矩阵
/// 兜底）。执行期间通过线程局部 [`set_tool_allow`] 对脚本可见，Drop 自动还原。
fn run_entry(
    script_path: &Path,
    entry: &str,
    collect_result: bool,
    run_env: &PluginRunEnv,
    allowed: Option<HashSet<String>>,
    app: AppHandle,
    build_ctx: impl FnOnce(&VirtualMachine, AppHandle) -> PyResult<PyObjectRef>,
) -> Result<Option<Value>, String> {
    let script = std::fs::read_to_string(script_path).map_err(|e| format!("读取脚本失败: {e}"))?;
    let interpreter = build_interpreter();
    interpreter.enter(|vm| {
        // 本次执行期间，read_data_file 只认这个插件 manifest 的 read 声明
        let _read_guard = host_api::set_read_allow(&run_env.read);
        // call_tool 只认本次调用继承来的工具白名单（嵌套执行互不串味）
        let _tool_allow_guard = set_tool_allow(allowed);
        // send_user_message 只对声明了该能力的插件放行
        let _send_guard = host_api::set_send_allow(run_env.send_user_message);
        // ws_send / ws_open / ws_close 按当前插件定位连接
        let _plugin_guard = host_api::set_current_plugin(&run_env.plugin_id);
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
        py_serde::serialize(vm, &result, serde_json::value::Serializer)
            .map(Some)
            .map_err(|e| format!("结果序列化失败: {e}"))
    })
}

/// 执行工具脚本，调用 `run(ctx)` 并返回结果。
///
/// `allowed` = 外层工具上下文的白名单（模型调用插件工具时由 `ToolContext` 传入），
/// 插件脚本里的 `call_tool` 只能触达其中的工具。
/// 必须在 `spawn_blocking` 内调用（`Interpreter::enter` 需要线程局部状态）。
pub(crate) fn run_plugin_script(
    script_path: &Path,
    tool_name: &str,
    args: &Value,
    run_env: PluginRunEnv,
    allowed: Option<HashSet<String>>,
    app: AppHandle,
) -> Result<Value, String> {
    run_entry(
        script_path,
        "run",
        true,
        &run_env,
        allowed,
        app,
        |vm, app| build_tool_ctx(vm, tool_name, args, &run_env, app),
    )
    .map(|result| result.unwrap_or(Value::Null))
}

/// 执行插件的启动入口，调用 `handler(ctx)`。
///
/// ctx 只有 `config` / `env` / `call_tool`——启动没有信号名与载荷。返回值按约定丢弃。
/// 启动入口没有外层工具上下文，`allowed` 传 `None`（`call_tool` 走权限矩阵兜底）。
pub(crate) fn run_plugin_startup(
    script_path: &Path,
    handler: &str,
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(
        script_path,
        handler,
        false,
        &run_env,
        None,
        app,
        |vm, app| {
            let ctx = vm.ctx.new_dict();
            inject_common(vm, &ctx, &run_env, app)?;
            Ok(ctx.into())
        },
    )
    .map(|_| ())
}

/// 执行插件的信号 handler，调用 `handler(ctx)`。
///
/// 返回值按约定丢弃——信号没有下游消费者，结果只用于让宿主感知执行失败。
/// 信号 handler 同样没有外层工具上下文，`allowed` 恒为 `None`。
/// 必须在 `spawn_blocking` 内调用。
pub(crate) fn run_plugin_handler(
    script_path: &Path,
    handler: &str,
    signal: &str,
    payload: &Value,
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(
        script_path,
        handler,
        false,
        &run_env,
        None,
        app,
        |vm, app| build_signal_ctx(vm, signal, payload, &run_env, app),
    )
    .map(|_| ())
}
