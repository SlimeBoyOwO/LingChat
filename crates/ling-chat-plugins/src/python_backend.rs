//! RustPython 嵌入执行器：加载插件脚本、注入 ctx、调用 `run(ctx)`。
//!
//! 每个工具调用创建一个全新的 `Interpreter`（`Interpreter` 非 `Send`，
//! 无法跨线程共享），脚本在 `spawn_blocking` 线程内执行，外层由调用方
//! 用 `tokio::time::timeout` 兜底，超时直接丢弃整个解释器。

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

/// 插件目录的导入钩子（一段 Python，装进解释器后让脚本能 import 同目录的兄弟模块）。
///
/// 为什么读文件的是宿主而不是插件：这个嵌入式解释器**没有任何文件系统能力**——
/// 冻结标准库里没有 `os` / `nt` / `posix` / `_os`，`_io.open` 在调用时就报
/// 「Couldn't get FileIO」，常规的文件查找器（`PathFinder`）既没装上、装上也读不了文件。
/// 所以这里注入 `_read_plugin_source(name)`（宿主用 `std::fs` 读，只认插件目录下
/// `<名字>.py`），插件只多拿到「执行同目录某个文件」这一件事，仍然碰不到文件系统。
///
/// 只认顶层单段模块名：`a.b`、带路径分隔符的名字一律不算模块名（见 `is_module_name`）。
const PLUGIN_IMPORT_BOOTSTRAP: &str = r#"
import sys

_bootstrap = sys.modules['_frozen_importlib']


class _PluginSourceLoader:
    """把一个兄弟模块的源码当模块执行。源码是宿主读好递进来的。"""

    def __init__(self, name, source, path):
        self.name = name
        self.source = source
        self.path = path

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        # __file__ 自己填：spec.has_location 会让导入系统去取 spec.cached，
        # 那条路 RustPython 没实现（NotImplementedError）。
        module.__file__ = self.path
        exec(compile(self.source, self.path, 'exec'), module.__dict__)


class _PluginFinder:
    def find_spec(self, name, path=None, target=None):
        if not name or '.' in name:
            return None
        source = _read_plugin_source(name)
        if source is None:
            return None
        location = _plugin_dir + '/' + name + '.py'
        spec = _bootstrap.ModuleSpec(name, _PluginSourceLoader(name, source, location))
        spec.origin = location
        return spec


sys.meta_path.append(_PluginFinder())
"#;

/// 模块名：首字符是字母或下划线，其余是字母、数字、下划线。
///
/// `a.b`、`../x`、`C:\x` 这类是路径不是模块名，一律挡掉——它们本可以诱导宿主读到
/// 插件目录之外去。
fn is_module_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {},
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 读兄弟模块源码：只认插件目录下的 `<模块名>.py`，读不到就给 None。
fn make_source_reader(vm: &VirtualMachine, plugin_dir: PathBuf) -> PyObjectRef {
    vm.new_function(
        "_read_plugin_source",
        move |name: String| -> Option<String> {
            if !is_module_name(&name) {
                return None;
            }
            std::fs::read_to_string(plugin_dir.join(format!("{name}.py"))).ok()
        },
    )
    .into()
}

/// 装上插件目录的导入钩子。
///
/// 必须在跑脚本顶层之前调用：兄弟模块就是在顶层 `import` 的。
fn install_plugin_importer(vm: &VirtualMachine, plugin_dir: &Path) -> PyResult<()> {
    let scope = vm.new_scope_with_builtins();
    let location = plugin_dir.to_string_lossy().into_owned();
    scope.globals.set_item(
        vm.ctx.intern_str("_plugin_dir"),
        vm.ctx.new_str(location).into(),
        vm,
    )?;
    scope.globals.set_item(
        vm.ctx.intern_str("_read_plugin_source"),
        make_source_reader(vm, plugin_dir.to_path_buf()),
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

/// 公共执行骨架：建解释器 → 跑脚本顶层 → 拦截危险模块 → 调用入口函数。
///
/// `collect_result` 为 false 时不序列化返回值（信号 handler 的返回值按约定丢弃，
/// 不该因为返回了不可序列化的对象而报错）。
fn run_entry(
    script_path: &Path,
    entry: &str,
    collect_result: bool,
    run_env: &PluginRunEnv,
    app: AppHandle,
    build_ctx: impl FnOnce(&VirtualMachine, AppHandle) -> PyResult<PyObjectRef>,
) -> Result<Option<Value>, String> {
    let script = std::fs::read_to_string(script_path).map_err(|e| format!("读取脚本失败: {e}"))?;
    let plugin_dir = script_path.parent().unwrap_or(Path::new(""));
    let interpreter = build_interpreter();
    interpreter.enter(|vm| {
        // 本次执行期间，read_data_file 只认这个插件 manifest 的 read 声明
        let _read_guard = host_api::set_read_allow(&run_env.read);
        // send_user_message 只对声明了该能力的插件放行
        let _send_guard = host_api::set_send_allow(run_env.send_user_message);
        // trigger_proactive 同上
        let _proactive_guard = host_api::set_proactive_allow(run_env.trigger_proactive);
        // ws_send / ws_open / ws_close 按当前插件定位连接
        let _plugin_guard = host_api::set_current_plugin(&run_env.plugin_id);
        // 兄弟模块要在脚本顶层 import，所以导入钩子必须先装好
        if !plugin_dir.as_os_str().is_empty() {
            install_plugin_importer(vm, plugin_dir)
                .map_err(|e| format!("插件导入钩子初始化失败: {}", exc_message(vm, &e)))?;
        }
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
/// 必须在 `spawn_blocking` 内调用（`Interpreter::enter` 需要线程局部状态）。
pub(crate) fn run_plugin_script(
    script_path: &Path,
    tool_name: &str,
    args: &Value,
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<Value, String> {
    run_entry(script_path, "run", true, &run_env, app, |vm, app| {
        build_tool_ctx(vm, tool_name, args, &run_env, app)
    })
    .map(|result| result.unwrap_or(Value::Null))
}

/// 执行插件的启动入口，调用 `handler(ctx)`。
///
/// ctx 只有 `config` / `env` / `call_tool`——启动没有信号名与载荷。返回值按约定丢弃。
pub(crate) fn run_plugin_startup(
    script_path: &Path,
    handler: &str,
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(script_path, handler, false, &run_env, app, |vm, app| {
        let ctx = vm.ctx.new_dict();
        inject_common(vm, &ctx, &run_env, app)?;
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
    run_env: PluginRunEnv,
    app: AppHandle,
) -> Result<(), String> {
    run_entry(script_path, handler, false, &run_env, app, |vm, app| {
        build_signal_ctx(vm, signal, payload, &run_env, app)
    })
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 起一个和线上一样的解释器、装上导入钩子、跑一段脚本。
    ///
    /// 不调 `build_interpreter`：它会拉进宿主那一侧（`host_api` 的 tauri 依赖），
    /// 测试 exe 在 Windows 上会以 0xC0000139 起不来。导入钩子本身与宿主 API 无关，
    /// 这里少的就是注入的 `plugin_host` 模块。
    fn run_in_plugin_dir(dir: &Path, script: &str) -> Result<(), String> {
        let interpreter = Interpreter::builder(rustpython_vm::Settings::default())
            .add_frozen_modules(rustpython_pylib::FROZEN_STDLIB)
            .build();
        interpreter.enter(|vm| {
            install_plugin_importer(vm, dir).map_err(|e| exc_message(vm, &e))?;
            let scope = vm.new_scope_with_builtins();
            let code = vm
                .compile(script, Mode::Exec, "<test>".to_owned())
                .map_err(|e| format!("脚本编译失败: {e}"))?;
            vm.run_code_obj(code, scope)
                .map(|_| ())
                .map_err(|e| format!("[{}] {}", e.as_object().class().name(), exc_message(vm, &e)))
        })
    }

    /// 建一个空的插件目录。每个用例用不同名字，免得互相看见对方的文件。
    fn plugin_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn plugin_can_import_a_sibling_module() {
        let dir = plugin_dir("lingchat_plugin_import_ok");
        std::fs::write(dir.join("shared_table.py"), "VALUE = 42\n").unwrap();
        // 兄弟模块之间也能互相 import
        std::fs::write(
            dir.join("helper.py"),
            "from shared_table import VALUE\nDOUBLED = VALUE * 2\n",
        )
        .unwrap();

        run_in_plugin_dir(
            &dir,
            "from shared_table import VALUE\n\
             import helper\n\
             assert VALUE == 42, VALUE\n\
             assert helper.DOUBLED == 84, helper.DOUBLED\n\
             import shared_table\n\
             assert shared_table.__file__.endswith('shared_table.py'), shared_table.__file__\n",
        )
        .expect("插件目录里的兄弟模块应该 import 得到");
    }

    /// 带点或路径分隔符的名字不算模块名：那是路径，能把宿主引到插件目录之外去。
    #[test]
    fn module_names_reject_paths() {
        assert!(is_module_name("tables"));
        assert!(is_module_name("_helpers2"));
        assert!(!is_module_name(""));
        assert!(!is_module_name("a.b"));
        assert!(!is_module_name("../tables"));
        assert!(!is_module_name("..\\tables"));
        assert!(!is_module_name("sub/tables"));
        assert!(!is_module_name("C:\\tables"));
        assert!(!is_module_name("2tables"));
    }

    /// 只能 import 自己目录里的模块：隔壁插件的文件、带点的名字、读源码的原生函数都不给。
    #[test]
    fn sibling_import_does_not_leave_the_plugin_dir() {
        let mine = plugin_dir("lingchat_plugin_import_mine");
        let other = plugin_dir("lingchat_plugin_import_other");
        std::fs::write(mine.join("sub.py"), "VALUE = 1\n").unwrap();
        std::fs::write(other.join("secret.py"), "VALUE = 'leaked'\n").unwrap();

        run_in_plugin_dir(&mine, "import secret\n")
            .expect_err("不该 import 到别的插件目录里的模块");
        run_in_plugin_dir(&mine, "import sub.tables\n").expect_err("带点的名字不该被认成模块名");
        run_in_plugin_dir(&other, "from sub import VALUE\n")
            .expect_err("不同插件目录的文件不该互相可见");

        // 读源码这件事是宿主做的：脚本只能请求「执行某个模块」，拿不到读文件的入口
        run_in_plugin_dir(&mine, "assert '_read_plugin_source' not in dir()\n")
            .expect("读源码的原生函数不该出现在脚本的全局里");

        // 装上钩子不等于开了文件系统：该禁的模块还是禁的
        run_in_plugin_dir(&mine, "import os\n").expect_err("沙箱禁的模块不该因为这个钩子变得可用");
        run_in_plugin_dir(&mine, "import _io\n_io.open('x')\n")
            .expect_err("解释器里本来就没有文件读写，别指望从这里绕出去");
    }

    /// 兄弟模块里的语法错误要指到那个文件，而不是含糊地说一句脚本执行失败。
    #[test]
    fn sibling_syntax_error_points_at_the_file() {
        let dir = plugin_dir("lingchat_plugin_import_bad");
        std::fs::write(dir.join("broken.py"), "def :\n").unwrap();

        let err = run_in_plugin_dir(&dir, "import broken\n").expect_err("坏模块应该报错");
        assert!(err.contains("broken.py"), "错误里应该点出文件：{err}");
    }
}
