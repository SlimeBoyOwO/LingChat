//! RustPython 嵌入执行器：加载插件脚本、注入 ctx、调用 `run(ctx)`。
//!
//! 每个工具调用创建一个全新的 `Interpreter`（`Interpreter` 非 `Send`，
//! 无法跨线程共享），脚本在自建的 `plugin-script` 线程内执行（见
//! [`run_on_script_thread`]），外层由调用方用 `tokio::time::timeout` 兜底，
//! 超时直接丢弃整个解释器。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// 插件目录的导入钩子（一段 Python，装进解释器后让脚本能 import 插件内的模块与包）。
///
/// 为什么读文件的是宿主而不是插件：这个嵌入式解释器**没有任何文件系统能力**——
/// 冻结标准库里没有 `os` / `nt` / `posix` / `_os`，`_io.open` 在调用时就报
/// 「Couldn't get FileIO」，常规的文件查找器（`PathFinder`）既没装上、装上也读不了文件。
/// 所以这里注入 `_read_plugin_module(name)`（宿主用 `std::fs` 读，只认插件目录内的
/// `<a>/<b>.py` 或 `<a>/<b>/__init__.py`），插件只多拿到「执行插件内某个模块」这一件事，
/// 仍然碰不到文件系统。
///
/// 模块名可以点分（`events.world`）；每一段必须是合法标识符，路径分隔符与 `..`
/// 一律挡掉（见 `is_module_name`）。包必须有 `__init__.py`，相对导入由 RustPython
/// 的导入系统按 `__package__` 解析。
const PLUGIN_IMPORT_BOOTSTRAP: &str = r#"
import sys

_bootstrap = sys.modules['_frozen_importlib']


class _PluginSourceLoader:
    """把一个模块（或包的 __init__.py）的源码当模块执行。源码是宿主读好递进来的。"""

    def __init__(self, name, source, path, is_package):
        self.name = name
        self.source = source
        self.path = path
        self.is_package = is_package

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        # __file__ 自己填：spec.has_location 会让导入系统去取 spec.cached，
        # 那条路 RustPython 没实现（NotImplementedError）。
        module.__file__ = self.path
        exec(compile(self.source, self.path, 'exec'), module.__dict__)


class _PluginFinder:
    def find_spec(self, name, path=None, target=None):
        if not name:
            return None
        info = _read_plugin_module(name)
        if info is None:
            return None
        is_package = info['is_package']
        location = info['location']
        spec = _bootstrap.ModuleSpec(
            name,
            _PluginSourceLoader(name, info['source'], location, is_package),
            origin=location,
            is_package=is_package,
        )
        if is_package:
            spec.submodule_search_locations = [location.rsplit('/', 1)[0]]
        return spec


sys.meta_path.append(_PluginFinder())
"#;

/// 模块名：点分，每一段都是合法 Python 标识符。
///
/// `a.b` 是合法模块名；`a..b`、`../x`、`C:\x`、`sub/tables` 这类含空段或路径分隔符的
/// 名字一律挡掉——它们本可以诱导宿主读到插件目录之外去。
fn is_module_name(name: &str) -> bool {
    !name.is_empty() && name.split('.').all(is_module_segment)
}

fn is_module_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {},
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

struct ModuleSource {
    path: PathBuf,
    is_package: bool,
}

/// 把点分模块名解析成插件目录内的源码文件。
///
/// 先找包目录 `<name>/__init__.py`，再找同名 `<name>.py`；两者都要在 canonicalize
/// 之后仍位于插件目录内（挡住符号链接逃逸）。`root` 必须是已 canonicalize 的路径。
fn resolve_module(root: &Path, name: &str) -> Option<ModuleSource> {
    if !is_module_name(name) {
        return None;
    }
    let mut base = root.to_path_buf();
    for segment in name.split('.') {
        base.push(segment);
    }
    let package_init = base.join("__init__.py");
    if let Some(path) = safe_file(root, &package_init) {
        return Some(ModuleSource {
            path,
            is_package: true,
        });
    }
    let module_file = base.with_extension("py");
    if let Some(path) = safe_file(root, &module_file) {
        return Some(ModuleSource {
            path,
            is_package: false,
        });
    }
    None
}

fn safe_file(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let path = candidate.canonicalize().ok()?;
    (path.starts_with(root) && path.is_file()).then_some(path)
}

/// 读插件内模块源码：只认插件目录里的 `<a>/<b>.py` 或 `<a>/<b>/__init__.py`。
fn make_module_reader(vm: &VirtualMachine, plugin_dir: PathBuf) -> PyObjectRef {
    let root = plugin_dir.canonicalize().unwrap_or(plugin_dir);
    vm.new_function(
        "_read_plugin_module",
        move |name: String, vm: &VirtualMachine| -> PyObjectRef {
            let Some(module) = resolve_module(&root, &name) else {
                return vm.ctx.none();
            };
            let Ok(source) = std::fs::read_to_string(&module.path) else {
                return vm.ctx.none();
            };
            let location = module.path.to_string_lossy().replace('\\', "/");
            let info = vm.ctx.new_dict();
            let set =
                |key: &str, value: PyObjectRef| info.set_item(vm.ctx.intern_str(key), value, vm);
            if set("source", vm.ctx.new_str(source).into()).is_err()
                || set("is_package", vm.ctx.new_bool(module.is_package).into()).is_err()
                || set("location", vm.ctx.new_str(location).into()).is_err()
            {
                return vm.ctx.none();
            }
            info.into()
        },
    )
    .into()
}

/// 装上插件目录的导入钩子。
///
/// 必须在跑脚本顶层之前调用：兄弟模块与包都是在顶层 `import` 的。
fn install_plugin_importer(vm: &VirtualMachine, plugin_dir: &Path) -> PyResult<()> {
    let scope = vm.new_scope_with_builtins();
    scope.globals.set_item(
        vm.ctx.intern_str("_read_plugin_module"),
        make_module_reader(vm, plugin_dir.to_path_buf()),
        vm,
    )?;
    let code = vm
        .compile(
            PLUGIN_IMPORT_BOOTSTRAP,
            Mode::Exec,
            "<plugin_import>".to_owned(),
        )
        .map_err(|e| vm.new_syntax_error(&e, Some(PLUGIN_IMPORT_BOOTSTRAP)))?;
    vm.run_code_obj(code, scope).map(|_| ())
}

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

/// 插件脚本线程的栈大小。钦灵：这是个技术坑，靠 AI 解决的，我就把注释全留着好了
///
/// RustPython 的模块导入是递归执行的：脚本 → 兄弟模块 → 标准库模块 → 它自己的依赖。
/// tokio 阻塞线程只有 2MB 栈，插件多引一层就会整进程爆栈（STATUS_STACK_OVERFLOW）。
/// 这里给足余量：栈是虚拟地址预留，只有真用到才提交。
const SCRIPT_STACK_SIZE: usize = 16 * 1024 * 1024;

/// 在独立线程上执行一段阻塞的脚本工作，返回它的结果。
///
/// 代替 `tokio::task::spawn_blocking`：那边固定 2MB 栈，给不了 [`SCRIPT_STACK_SIZE`]。
/// 线程名固定为 `plugin-script`，真出问题时（包括栈溢出）能从日志一眼认出是谁。
pub(crate) async fn run_on_script_thread<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("plugin-script".into())
        .stack_size(SCRIPT_STACK_SIZE)
        .spawn(move || {
            let _ = tx.send(work());
        })
        .map_err(|e| format!("创建插件脚本线程失败: {e}"))?;
    rx.await.map_err(|_| "插件脚本线程异常退出".to_string())
}

/// 公共执行骨架：建解释器 → 跑脚本顶层 → 拦截危险模块 → 调用入口函数。
///
/// `collect_result` 为 false 时不序列化返回值（信号 handler 的返回值按约定丢弃，
/// 不该因为返回了不可序列化的对象而报错）。
fn run_entry(
    plugin_root: &Path,
    script_path: &Path,
    entry: &str,
    collect_result: bool,
    run_env: &PluginRunEnv,
    app: AppHandle,
    build_ctx: impl FnOnce(&VirtualMachine, AppHandle) -> PyResult<PyObjectRef>,
) -> Result<Option<Value>, String> {
    let root = plugin_root
        .canonicalize()
        .map_err(|e| format!("插件目录无效: {e}"))?;
    let script_path = script_path
        .canonicalize()
        .map_err(|e| format!("读取脚本失败: {e}"))?;
    if !script_path.starts_with(&root) {
        return Err("脚本路径越出插件目录".to_string());
    }
    let script = std::fs::read_to_string(&script_path).map_err(|e| format!("读取脚本失败: {e}"))?;
    let interpreter = build_interpreter();
    interpreter.enter(|vm| {
        // 本次执行期间，read_data_file 只认这个插件 manifest 的 read 声明
        let _read_guard = host_api::set_read_allow(&run_env.read);
        // send_user_message 只对声明了该能力的插件放行
        let _send_guard = host_api::set_send_allow(run_env.send_user_message);
        // 顺带抓屏同理：屏幕内容属于玩家隐私，单独声明
        let _capture_guard = host_api::set_capture_allow(run_env.capture_screen);
        // trigger_proactive 同上
        let _proactive_guard = host_api::set_proactive_allow(run_env.trigger_proactive);
        // ws_send / ws_open / ws_close 按当前插件定位连接
        let _plugin_guard = host_api::set_current_plugin(&run_env.plugin_id);
        // 兄弟模块与包要在脚本顶层 import，所以导入钩子必须先装好
        install_plugin_importer(vm, &root)
            .map_err(|e| format!("插件导入钩子初始化失败: {}", exc_message(vm, &e)))?;
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
/// 必须在 [`run_on_script_thread`] 里调用（`Interpreter::enter` 需要线程局部状态）。
pub(crate) fn run_plugin_script(
    plugin_root: &Path,
    script_path: &Path,
    tool_name: &str,
    args: &Value,
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<Value, String> {
    run_entry(
        plugin_root,
        script_path,
        "run",
        true,
        &run_env,
        app,
        |vm, app| build_tool_ctx(vm, tool_name, args, &run_env, app),
    )
    .map(|result| result.unwrap_or(Value::Null))
}

/// 执行插件的启动入口，调用 `handler(ctx)`。
///
/// ctx 只有 `config` / `env` / `call_tool`——启动没有信号名与载荷。返回值按约定丢弃。
pub(crate) fn run_plugin_startup(
    plugin_root: &Path,
    script_path: &Path,
    handler: &str,
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(
        plugin_root,
        script_path,
        handler,
        false,
        &run_env,
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
/// 同样必须在 [`run_on_script_thread`] 里调用。
pub(crate) fn run_plugin_handler(
    plugin_root: &Path,
    script_path: &Path,
    handler: &str,
    signal: &str,
    payload: &Value,
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(
        plugin_root,
        script_path,
        handler,
        false,
        &run_env,
        app,
        |vm, app| build_signal_ctx(vm, signal, payload, &run_env, app),
    )
    .map(|_| ())
}
