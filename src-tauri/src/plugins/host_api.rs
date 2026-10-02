//! 插件宿主原生模块 `plugin_host`：注入给插件 Python 脚本的全部宿主能力。
//!
//! 三类能力：
//! - **HTTP**：`http_get` / `http_post`，复用 `factory::build_http_client`
//!   （webpki-roots，Android 兼容），返回 `{ status, ok, body }`。
//! - **宿主控制**：`switch_character`（完整切换当前角色）、以及台词历史（上下文
//!   源）的 `read_context` / `edit_context` / `compress_context`。
//! - **读素材**：`read_data_file`，把 `data/` 下的文件（角色立绘、TTS 语音等）
//!   读成 base64 交给插件；只允许 `data/` 内的相对路径，且必须落在该插件
//!   manifest `read` 声明的前缀之下（未声明一律拒绝）。
//!
//! 这是**插件专属**面——LLM 拿不到，与 `ctx["call_tool"]` 那条共用通道相对。
//! 新增宿主能力时，`#[pyfunction]` 直接加在下面的 `plugin_host` 模块里，
//! 实现放在本文件上方，不要另开文件——插件作者看到的是一个模块。

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use rustpython_derive::pymodule;
use rustpython_vm::{
    PyObjectRef, PyResult, VirtualMachine, builtins::PyListRef, function::KwArgs, py_serde,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::llm::factory;
use crate::ai_service::tools::game_status_handle;
use crate::ai_service::types::GameLine;
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

// ── 台词历史（上下文源）的读取 / 编辑 / 压缩 ──

/// 宿主句柄未初始化时的统一返回（插件在 setup 完成前被调用）。
fn no_host(vm: &VirtualMachine) -> PyObjectRef {
    value_to_pyobject(
        vm,
        &serde_json::json!({ "ok": false, "error": "宿主句柄未初始化" }),
    )
}

/// 解析 kwargs 里的 `start` / `end`（1 起、闭区间；缺省 = 整段，由 impl 补）。
fn kw_range(
    vm: &VirtualMachine,
    kwargs: &HashMap<String, PyObjectRef>,
) -> (Option<usize>, Option<usize>) {
    let get = |key: &str| {
        kwargs
            .get(key)
            .and_then(|v| py_to_value(vm, v).as_u64())
            .map(|n| n as usize)
    };
    (get("start"), get("end"))
}

/// 读取第 `start`~`end` 条台词，缺省整段。
async fn read_context_impl(
    app: AppHandle,
    start: Option<usize>,
    end: Option<usize>,
) -> serde_json::Value {
    let gs = game_status_handle(&app).await;
    let gs = gs.lock().await;
    let total = gs.line_list.len();
    let lines = gs.read_context_lines(start.unwrap_or(1), end.unwrap_or(total));
    serde_json::json!({ "ok": true, "total": total, "lines": lines })
}

/// 用 `replacement` 替换第 `start`~`end` 条台词，缺省整段。
async fn edit_context_impl(
    app: AppHandle,
    start: Option<usize>,
    end: Option<usize>,
    replacement: Vec<GameLine>,
) -> serde_json::Value {
    let state = app.state::<AppState>();
    let db = state.db.clone();
    let generation_lock = state.generation_lock.clone();
    let gs = game_status_handle(&app).await;
    let mut gs = gs.lock().await;
    let total = gs.line_list.len();
    match gs
        .edit_context_lines(
            &db,
            &generation_lock,
            start.unwrap_or(1),
            end.unwrap_or(total),
            replacement,
        )
        .await
    {
        Ok(removed) => {
            serde_json::json!({ "ok": true, "removed": removed, "total": gs.line_list.len() })
        },
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    }
}

/// 触发并等待一次永久记忆压缩；只压达到阈值的角色。
async fn compress_context_impl(app: AppHandle) -> serde_json::Value {
    // 短锁：只取句柄 + 台词快照，随后在锁外 await 压缩，
    // 避免压缩（若干次 LLM 调用）期间一直冻住 GameStatus。
    let (handles, lines) = {
        let gs = game_status_handle(&app).await;
        let gs = gs.lock().await;
        (gs.role_manager.memory_bank_handles(), gs.line_list.clone())
    };
    let mut triggered = 0usize;
    for handle in handles {
        if handle.compress_if_needed(&lines).await {
            triggered += 1;
        }
    }
    serde_json::json!({ "ok": true, "triggered": triggered })
}

// ── 读素材 ──

/// 单次读取上限。插件读素材基本是为了转 base64 发给对端（视频、长语音这类也要能过），
/// 给宽一点；注意峰值内存约是这个数的 2～3 倍（原字节 + base64 串 + Python str 拷贝），
/// 再往上就该换流式/分片通道了。
const MAX_READ_BYTES: u64 = 64 * 1024 * 1024;

thread_local! {
    /// 当前线程正在执行的插件所声明的可读前缀（由 `python_backend::run_entry` 设置）。
    ///
    /// 脚本在单个线程内同步跑完，`call_tool` 会切到插件 runtime 的线程再切回来，
    /// 所以「当前插件」按线程区分是成立的；guard 在 Drop 时还原上一个值，嵌套执行
    /// （脚本里 call_tool 触发另一个插件）也不会串味。
    static READ_ALLOW: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// 设置当前线程的可读前缀；Drop 时还原（见 `READ_ALLOW`）。
pub(crate) struct ReadAllowGuard {
    prev: Vec<String>,
}

impl Drop for ReadAllowGuard {
    fn drop(&mut self) {
        READ_ALLOW.with(|cell| *cell.borrow_mut() = std::mem::take(&mut self.prev));
    }
}

/// 设置当前线程的可读前缀，返回还原用 guard。
pub(crate) fn set_read_allow(allow: &[String]) -> ReadAllowGuard {
    let prev = READ_ALLOW.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), allow.to_vec()));
    ReadAllowGuard { prev }
}

fn current_read_allow() -> Vec<String> {
    READ_ALLOW.with(|cell| cell.borrow().clone())
}

thread_local! {
    /// 当前线程正在执行的插件 id（由 `python_backend::run_entry` 设置）。
    /// `ws_send` / `ws_open` / `ws_close` 按插件定位连接时用它，与 `READ_ALLOW` 同款。
    static CURRENT_PLUGIN: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// 设置当前线程的插件 id；Drop 时还原（见 `CURRENT_PLUGIN`）。
pub(crate) struct CurrentPluginGuard {
    prev: Option<String>,
}

impl Drop for CurrentPluginGuard {
    fn drop(&mut self) {
        CURRENT_PLUGIN.with(|cell| *cell.borrow_mut() = std::mem::take(&mut self.prev));
    }
}

/// 设置当前线程的插件 id，返回还原用 guard。
pub(crate) fn set_current_plugin(id: &str) -> CurrentPluginGuard {
    let prev = CURRENT_PLUGIN.with(|cell| cell.borrow_mut().replace(id.to_string()));
    CurrentPluginGuard { prev }
}

fn current_plugin() -> Option<String> {
    CURRENT_PLUGIN.with(|cell| cell.borrow().clone())
}

/// 把 `Result<(), String>` 包成 `{ok, error}` 信封（插件好处理，不抛异常）。
fn result_to_py(vm: &VirtualMachine, r: Result<(), String>) -> PyObjectRef {
    value_to_pyobject(
        vm,
        &match r {
            Ok(()) => serde_json::json!({ "ok": true }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        },
    )
}

/// `{ok: false, error}` 信封。
fn err_json(vm: &VirtualMachine, msg: &str) -> PyObjectRef {
    value_to_pyobject(vm, &serde_json::json!({ "ok": false, "error": msg }))
}

/// 校验「相对 `data/` 的路径」字面是否合法：非空、相对、不含 `..` 与盘符。
///
/// manifest 的 `read` 声明与插件传入的请求路径共用这一份校验，别各写一套。
pub(crate) fn check_relative_data_path(rel: &str) -> Result<(), String> {
    use std::path::Component;

    if rel.is_empty() {
        return Err("路径为空".to_string());
    }
    let candidate = std::path::Path::new(rel);
    if candidate.is_absolute() {
        return Err("只接受相对 data/ 的路径".to_string());
    }
    for comp in candidate.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {},
            _ => return Err("路径里不能有 .. 或盘符".to_string()),
        }
    }
    Ok(())
}

/// 拼 IO 错误的文案。**不带真实路径**：`io::Error` 的 Display 里含宿主绝对路径，
/// 原文回给插件等于把 data 目录的位置交代出去，所以这里只用插件给的相对路径。
fn io_error_text(rel: &str, e: &std::io::Error) -> String {
    let reason = match e.kind() {
        std::io::ErrorKind::NotFound => "文件不存在",
        std::io::ErrorKind::PermissionDenied => "没有权限",
        _ => "IO 错误",
    };
    format!("读取失败：{rel}（{reason}）")
}

/// 把插件给的相对路径解析成 `base` 下的真实路径。
///
/// 规则（这是沙箱的一部分，别放宽）：
/// - 只接受相对路径：绝对路径、盘符、`..` 一律拒绝；
/// - 解析后必须仍在 `base` 内：软链接指到外面也会被这一步拦下；
/// - 还必须落在 `allow`（本插件 manifest 的 `read` 声明）里某个前缀之下。
///
/// 不复用 `utils::path::validate_path_in_base`：这里要拿回 canonical 路径供读取，
/// 且错误文案不能回带宿主绝对路径（那个辅助函数的报错里全是路径）。
///
/// 显式传 base 是为了能测：全局 `data_dir()` 只在 App 启动时初始化，单测里拿不到。
fn resolve_data_path_in(
    base: &std::path::Path,
    allow: &[String],
    rel: &str,
) -> Result<std::path::PathBuf, String> {
    let rel = rel.trim();
    check_relative_data_path(rel)?;

    let real = base
        .join(rel)
        .canonicalize()
        .map_err(|e| io_error_text(rel, &e))?;
    let base_real = base
        .canonicalize()
        .map_err(|_| "data 目录不可用".to_string())?;
    if !real.starts_with(&base_real) {
        return Err("越界：只能读 data/ 下的文件".to_string());
    }
    // 声明的前缀也要 canonicalize 后再比，免得软链接把声明目录指向 data/ 别处
    let declared = allow.iter().any(|prefix| {
        base.join(prefix.trim())
            .canonicalize()
            .is_ok_and(|p| real.starts_with(&p))
    });
    if !declared {
        return Err(format!(
            "未声明读取该路径：{rel}（需在本插件 manifest 的 read 中声明其所在目录）"
        ));
    }
    Ok(real)
}

/// 读 `data/` 下的文件并转 base64；失败返回 `{ok: false, error}`，不抛异常。
fn read_data_file_impl(rel: &str) -> serde_json::Value {
    read_data_file_impl_in(&crate::api::data_dir(), &current_read_allow(), rel)
}

/// `read_data_file_impl` 的显式 base / 声明版本（测试用临时目录）。
fn read_data_file_impl_in(
    base: &std::path::Path,
    allow: &[String],
    rel: &str,
) -> serde_json::Value {
    use base64::Engine as _;

    let rel = rel.trim();
    let path = match resolve_data_path_in(base, allow, rel) {
        Ok(path) => path,
        Err(e) => return serde_json::json!({ "ok": false, "error": e }),
    };
    let meta = match std::fs::metadata(&path) {
        Ok(meta) => meta,
        Err(e) => return serde_json::json!({ "ok": false, "error": io_error_text(rel, &e) }),
    };
    if !meta.is_file() {
        return serde_json::json!({ "ok": false, "error": "不是普通文件" });
    }
    if meta.len() > MAX_READ_BYTES {
        return serde_json::json!({
            "ok": false,
            "error": format!("文件太大（{} 字节，上限 {MAX_READ_BYTES}）", meta.len()),
        });
    }
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::json!({
            "ok": true,
            "size": bytes.len(),
            "base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
        }),
        Err(e) => serde_json::json!({ "ok": false, "error": io_error_text(rel, &e) }),
    }
}

/// 插件宿主原生模块。插件脚本里用 `from plugin_host import ...` 取用。
#[pymodule]
mod plugin_host {
    use rustpython_vm::{
        PyObjectRef, PyResult, VirtualMachine, builtins::PyBytes, function::KwArgs,
    };
    use tauri::Manager;

    use crate::ai_service::types::GameLine;

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

    /// 读取 `data/` 下的文件，返回 base64。
    ///
    /// 用法：`read_data_file("game_data/characters/风雪/avatar/高兴.webp")`
    ///
    /// 返回：成功 `{ "ok": true, "size": 12345, "base64": "..." }`；
    /// 失败 `{ "ok": false, "error": "..." }`（不抛异常，插件好处理）。
    ///
    /// 必须是相对 `data/` 的路径，`..`、绝对路径、指向外部的软链接都会被拒；
    /// 且该路径得落在本插件 manifest `read` 声明的前缀之下，未声明一律拒绝——
    /// 插件拿不到游戏目录之外的东西，也拿不到没声明过的素材。单次读取上限 64MB。
    /// 失败文案里只有插件自己给的相对路径，不回带宿主绝对路径。
    #[pyfunction]
    fn read_data_file(path: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        Ok(super::value_to_pyobject(
            vm,
            &super::read_data_file_impl(&path),
        ))
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

    /// 读取台词历史（上下文源）第 start~end 条，缺省整段。
    ///
    /// 用法：`read_context()`（整段）/ `read_context(start=1, end=3)`
    ///
    /// 返回 `{ "ok": true, "total": N, "lines": [ {全字段行} ] }`。
    /// 行 dict 携带全部字段（内容、情绪、动作、TTS、音频、thinking、tool_call、
    /// 感知集合等），可直接改后交给 `edit_context`。
    #[pyfunction]
    fn read_context(kwargs: KwArgs<PyObjectRef>, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let kwargs = super::kwargs_map(kwargs);
        let (start, end) = super::kw_range(vm, &kwargs);
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::no_host(vm));
        };
        let result = super::runtime().block_on(super::read_context_impl(app, start, end));
        Ok(super::value_to_pyobject(vm, &result))
    }

    /// 用 `replacement` 替换第 start~end 条台词，缺省整段。
    ///
    /// 用法：`edit_context(new_lines)` / `edit_context(new_lines, start=1, end=3)`
    ///
    /// `replacement` 是行 dict 列表，**每个 dict 需含全部字段**（最省事是拿
    /// `read_context` 的输出改）。条数不限：空列表 = 删除该区间，多条 = 展开。
    /// 返回 `{ "ok": true, "removed": N, "total": M }` 或 `{ "ok": false, "error": ... }`。
    #[pyfunction]
    fn edit_context(
        replacement: PyObjectRef,
        kwargs: KwArgs<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        let kwargs = super::kwargs_map(kwargs);
        let (start, end) = super::kw_range(vm, &kwargs);
        let lines: Vec<GameLine> =
            match serde_json::from_value(super::py_to_value(vm, &replacement)) {
                Ok(lines) => lines,
                Err(e) => {
                    return Ok(super::value_to_pyobject(
                        vm,
                        &serde_json::json!({
                            "ok": false,
                            "error": format!("replacement 解析为台词行失败: {e}"),
                        }),
                    ));
                },
            };
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::no_host(vm));
        };
        let result = super::runtime().block_on(super::edit_context_impl(app, start, end, lines));
        Ok(super::value_to_pyobject(vm, &result))
    }

    /// 触发并等待一次永久记忆压缩；只压达到阈值的角色。
    ///
    /// 用法：`compress_context()`
    ///
    /// 返回 `{ "ok": true, "triggered": N }`（N = 实际触发压缩的角色数，0 = 无需压缩）。
    #[pyfunction]
    fn compress_context(vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::no_host(vm));
        };
        let result = super::runtime().block_on(super::compress_context_impl(app));
        Ok(super::value_to_pyobject(vm, &result))
    }

    /// 启用一个已在 manifest 声明的 WebSocket 连接。
    ///
    /// 用法：`ws_open("gateway")` / `ws_open("gateway", url="wss://...")`
    ///
    /// 插件启用时宿主已自动建立声明的连接，`ws_open` 用于关闭后重连，或用脚本给出的
    /// url 覆盖声明值（仍受 manifest `ws_allow` 白名单约束）。返回 `{ "ok": true }`
    /// 或 `{ "ok": false, "error": "..." }`。
    #[pyfunction]
    fn ws_open(
        conn_id: String,
        kwargs: KwArgs<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        let kwargs = super::kwargs_map(kwargs);
        let url = kwargs
            .get("url")
            .and_then(|v| super::py_to_value(vm, v).as_str().map(str::to_string));
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::no_host(vm));
        };
        let Some(plugin_id) = super::current_plugin() else {
            return Ok(super::err_json(vm, "无法确定当前插件"));
        };
        let manager = app.state::<crate::AppState>().data().plugin_manager.clone();
        let result =
            super::runtime().block_on(manager.ws_open(&plugin_id, Some(&conn_id), url.as_deref()));
        Ok(super::result_to_py(vm, result))
    }

    /// 关闭一个 WebSocket 连接。
    ///
    /// 用法：`ws_close("gateway")`
    ///
    /// 关闭后可用 `ws_open` 重新连接。返回 `{ "ok": true }`。
    #[pyfunction]
    fn ws_close(conn_id: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::no_host(vm));
        };
        let Some(plugin_id) = super::current_plugin() else {
            return Ok(super::err_json(vm, "无法确定当前插件"));
        };
        let manager = app.state::<crate::AppState>().data().plugin_manager.clone();
        manager.ws_close(&plugin_id, Some(&conn_id));
        Ok(super::value_to_pyobject(
            vm,
            &serde_json::json!({ "ok": true }),
        ))
    }

    /// 向已建立的 WebSocket 连接发一帧。
    ///
    /// 用法：`ws_send("gateway", {"op": 1})` / `ws_send("gateway", "hello")` /
    /// `ws_send("gateway", b"\x01")`
    ///
    /// `str` → 文本帧；`bytes` → 二进制帧；dict / list / 数字 → JSON 文本帧。
    /// **同步发出、不等回执**；要拿对端响应就等下一次 `ws_message` 事件。
    /// 返回 `{ "ok": true }` 或 `{ "ok": false, "error": "..." }`。
    #[pyfunction]
    fn ws_send(conn_id: String, data: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::no_host(vm));
        };
        let Some(plugin_id) = super::current_plugin() else {
            return Ok(super::err_json(vm, "无法确定当前插件"));
        };
        // clone 廉价（引用计数）；downcast 会消耗 receiver，之后还要读 data。
        let cmd = match data.clone().downcast::<PyBytes>() {
            Ok(bytes) => crate::plugins::ws::WsCommand::Binary(bytes.as_bytes().to_vec()),
            // 其余（含 str）：转成 JSON 值——str 得到文本原样，dict/数字得到 JSON 文本。
            Err(_) => match super::py_to_value(vm, &data) {
                serde_json::Value::String(s) => crate::plugins::ws::WsCommand::Text(s),
                other => crate::plugins::ws::WsCommand::Text(other.to_string()),
            },
        };
        let manager = app.state::<crate::AppState>().data().plugin_manager.clone();
        let result = manager.ws_send(&plugin_id, &conn_id, cmd);
        Ok(super::result_to_py(vm, result))
    }

    /// 查询本插件 WS 连接的当前状态。
    ///
    /// 用法：`ws_status()` 返回全部声明；`ws_status(conn_id="gateway")` 返回单条。
    ///
    /// 返回：全部 `{ "ok": true, "connections": [ { "id", "mode", "state" } ] }`；
    /// 单条 `{ "ok": true, "id", "mode", "state" }`。
    /// `state` 取值：`"connected"` / `"connecting"` / `"stopped"` / `"error"`。
    #[pyfunction]
    fn ws_status(kwargs: KwArgs<PyObjectRef>, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let kwargs = super::kwargs_map(kwargs);
        let conn_id = kwargs
            .get("conn_id")
            .and_then(|v| super::py_to_value(vm, v).as_str().map(str::to_string));
        let Some(app) = crate::plugins::app_handle() else {
            return Ok(super::no_host(vm));
        };
        let Some(plugin_id) = super::current_plugin() else {
            return Ok(super::err_json(vm, "无法确定当前插件"));
        };
        let manager = app.state::<crate::AppState>().data().plugin_manager.clone();
        match manager.ws_status(&plugin_id, conn_id.as_deref()) {
            // 指定 conn_id 时 manager 保证非空，取首条。
            Ok(list) if conn_id.is_some() => {
                let c = &list[0];
                Ok(super::value_to_pyobject(
                    vm,
                    &serde_json::json!({ "ok": true, "id": c.id, "mode": c.mode, "state": c.state }),
                ))
            },
            Ok(list) => Ok(super::value_to_pyobject(
                vm,
                &serde_json::json!({ "ok": true, "connections": list }),
            )),
            Err(e) => Ok(super::err_json(vm, &e)),
        }
    }
}

/// 获取插件宿主模块定义（供解释器注入）。
pub(crate) fn plugin_module_def(
    ctx: &rustpython_vm::Context,
) -> &'static rustpython_vm::builtins::PyModuleDef {
    plugin_host::module_def(ctx)
}
