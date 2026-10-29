//! 插件宿主原生模块 `plugin_host`：注入给插件 Python 脚本的全部宿主能力。
//!
//! 按能力分文件：HTTP 在 `http`，角色/界面控制在 `character`，上下文在 `context`，
//! 读素材在 `assets`，送消息/抓屏/主动开口在 `conversation`，WS 控制在 `websocket`。
//! 本文件保留 `#[pymodule]` 导出面——20 个 `#[pyfunction]` 只做签名与转发，插件作者
//! 看到的仍是一个 `plugin_host` 模块。
//!
//! 新增宿主能力时：实现写进对应能力文件，在下面的 `plugin_host` 里挂一行转发。

mod assets;
mod character;
mod context;
mod conversation;
mod guard;
mod http;
mod websocket;

use std::collections::HashMap;
use std::sync::OnceLock;

use rustpython_derive::pymodule;
use rustpython_vm::{PyObjectRef, VirtualMachine, builtins::PyListRef, function::KwArgs, py_serde};

pub(crate) use assets::check_relative_data_path;
pub(crate) use guard::{
    set_capture_allow, set_current_plugin, set_proactive_allow, set_read_allow, set_send_allow,
};

/// 插件脚本在独立线程执行，线程上无 tokio runtime 上下文；
/// 需要异步能力（reqwest / 宿主 async 命令）时，用这个独立多线程 runtime 驱动。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

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
pub(super) fn py_to_value(vm: &VirtualMachine, obj: &PyObjectRef) -> serde_json::Value {
    py_serde::serialize(vm, obj, serde_json::value::Serializer).unwrap_or(serde_json::Value::Null)
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
pub(super) fn kwargs_map(kwargs: KwArgs<PyObjectRef>) -> HashMap<String, PyObjectRef> {
    kwargs.into_iter().collect()
}

/// 宿主句柄未初始化时的统一返回（插件在 setup 完成前被调用）。
pub(super) fn no_host(vm: &VirtualMachine) -> PyObjectRef {
    value_to_pyobject(
        vm,
        &serde_json::json!({ "ok": false, "error": "宿主句柄未初始化" }),
    )
}

/// 把 `Result<(), String>` 包成 `{ok, error}` 信封（插件好处理，不抛异常）。
pub(super) fn result_to_py(vm: &VirtualMachine, r: Result<(), String>) -> PyObjectRef {
    value_to_pyobject(
        vm,
        &match r {
            Ok(()) => serde_json::json!({ "ok": true }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        },
    )
}

/// `{ok: false, error}` 信封。
pub(super) fn err_json(vm: &VirtualMachine, msg: &str) -> PyObjectRef {
    value_to_pyobject(vm, &serde_json::json!({ "ok": false, "error": msg }))
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
        super::http::http_get(url, kwargs, vm)
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
        super::http::http_post(url, kwargs, vm)
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
        super::assets::read_data_file(path, vm)
    }

    /// 把一条用户消息送进对话（等同玩家在输入框里发了一句）。
    ///
    /// 用法：`send_user_message("在 QQ 里说的话")`
    ///
    /// 返回：成功 `{ "ok": true }`；失败 `{ "ok": false, "error": "..." }`（不抛异常）。
    ///
    /// **需要 manifest 顶层声明 `send_user_message = true`**，未声明的插件调用一律被拒。
    /// 调用等同玩家自己发了一句：会写进对话并触发回复生成（生成在后台跑，本调用
    /// 立刻返回）。注意两点：`/` 开头的内容会被当成调试指令，插件侧应自行过滤；
    /// 别把收到的 `ai_reply` 原样转回来，否则会和角色互相刷屏。
    #[pyfunction]
    fn send_user_message(text: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::conversation::send_user_message(text, vm)
    }

    /// 抓一张屏幕画面，返回 JPEG 的 data URL——工具把它转给模型看，模型就「看得到」了。
    ///
    /// 用法：`capture_screen()` / `capture_screen(window="Minecraft")`
    ///
    /// `window` 是要找的窗口标题（或进程名）关键词，命中就用它；找不到、抓失败、
    /// 抓回来的不像一张画面（最小化后只剩标题栏一条、独显全屏只给回黑图）都退回抓
    /// 整个桌面。图会先缩到长边 1024 再编 JPEG，约一千 token。
    ///
    /// 返回：成功 `{ "ok": true, "image": "data:image/jpeg;base64,..." }`；
    /// 失败 `{ "ok": false, "error": "..." }`（不抛异常）。
    ///
    /// **需要 manifest 顶层声明 `capture_screen = true`**（屏幕上是玩家的隐私），
    /// 且当前对话模型开着原生识图——看不到图的模型抓了也送不进对话。
    #[pyfunction]
    fn capture_screen(kwargs: KwArgs<PyObjectRef>, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::conversation::capture_screen(kwargs, vm)
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
        super::character::switch_character(role_id, vm)
    }

    /// 取当前角色名与玩家名：插件写旁白时用第三人称称呼，不再说「你 / 玩家」。
    ///
    /// 用法：`current_names()`
    ///
    /// 返回：`{ "ok": true, "role_id": 3, "ai_name": "...", "user_name": "..." }`；
    /// 没有加载角色时 `ai_name` 为空串。失败 `{ "ok": false, "error": "..." }`（不抛异常）。
    #[pyfunction]
    fn current_names(vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::character::current_names(vm)
    }

    /// 切换主界面的「自动模式」：开启后对话按设置里的延迟自动逐句推进，不用点。
    ///
    /// 用法：`set_auto_mode(True)` / `set_auto_mode(False)`
    ///
    /// 返回：成功 `{ "ok": true }`；失败 `{ "ok": false, "error": "..." }`（不抛异常）。
    #[pyfunction]
    fn set_auto_mode(enabled: bool, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::character::set_auto_mode(enabled, vm)
    }

    /// 切换回复台词数量档位：`short`（1~2 句）/ `normal`（3~5 句）/ `free`（不限制）。
    /// 对应设置页「对话长度」那一项。
    ///
    /// 用法：`set_reply_length_mode("short")`
    ///
    /// 要求已经进入对话；主菜单、剧本编辑器试玩中会被拒。档位没变化时不写任何台词，
    /// 重复调用是安全的。
    ///
    /// 返回：成功 `{ "ok": true, "mode": "short" }`；
    /// 失败 `{ "ok": false, "error": "..." }`（不抛异常）。
    #[pyfunction]
    fn set_reply_length_mode(mode: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::character::set_reply_length_mode(mode, vm)
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
        super::context::read_context(kwargs, vm)
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
        super::context::edit_context(replacement, kwargs, vm)
    }

    /// 往台词表末尾追加一条「提示」台词：旁白 / 系统提示 / 剧情提示。
    ///
    /// 用法：`add_prompt_line("narrator", "你已和玩家一起进入 Minecraft 世界")`
    ///
    /// 返回 `{ "ok": true, "index": N }`（N 是该行的位置，1 起、与 `read_context`
    /// 的下标口径一致）或 `{ "ok": false, "error": "..." }`（角色名不认识 / 内容为空）。
    ///
    /// 正在生成回复时不会直接写：这一行会先排队（`{ "ok": true, "queued": true }`），
    /// 等本轮收尾、下一轮开头按顺序补进台词表——中途插队会和流式写入抢行号。
    /// 连着的旁白在队列里会先用换行接成一行（上限 600 字），写入时仍是一条。
    ///
    /// 游戏事件传 `ttl_seconds=`（这条信息值多少秒）就一直留在队列里，等角色真要开口
    /// 时再判一次，过期的直接丢掉；`key=` + `severity=` 让同类事件只留最严重的一条。
    /// 带有效期的调用返回 `{ "ok": true, "queued": true, "pending": N }`。
    #[pyfunction]
    fn add_prompt_line(
        role: String,
        text: String,
        kwargs: KwArgs<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        super::context::add_prompt_line(role, text, kwargs, vm)
    }

    /// 请求宿主主动开一次口（主动对话系统的一次投放）：角色不等玩家说话，自己说一句。
    ///
    /// 用法：`trigger_proactive("")` —— 把要说的先记成旁白，再空提示请求开口；
    /// 传提示语则会被包成 `{旁白: （系统提示：…）}` 写进台词表（截图转述这类要用）。
    /// `urgent=True` 用于死亡这类必须马上开口的事：顶掉排队中的普通请求，
    /// 投放时也不吃冷却与上一轮的静默期（仍要等当前这轮生成结束）。
    ///
    /// 请求只会**排队**：等这一轮台词播完、前端空闲下来才投放；同时最多攒一条，
    /// 短时间连发多次会合并成一次（战斗里每挨一下来一次也不会叠出好几轮）。
    ///
    /// 返回：`{ "ok": true, "triggered": true }`（已受理/已合并）
    #[pyfunction]
    fn trigger_proactive(
        text: String,
        kwargs: KwArgs<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        super::conversation::trigger_proactive(text, kwargs, vm)
    }

    /// 报告「游戏里此刻能不能播这句语音」，用于 `[voice_playback]` 接管期间。
    ///
    /// 用法：`set_voice_playback_available(False)` —— 外部程序找不到播放目标
    ///（女仆倒下了、模组没连上）就播不出来；恢复或重新连上时再报 `True`。
    /// 宿主据此决定下一句是交给插件还是本机播放，玩家不用去关设置页的开关。
    ///
    /// 返回 `{ "ok": true }`。
    #[pyfunction]
    fn set_voice_playback_available(available: bool, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::conversation::set_voice_playback_available(available, vm)
    }

    /// 触发并等待一次永久记忆压缩；只压达到阈值的角色。
    ///
    /// 用法：`compress_context()`
    ///
    /// 返回 `{ "ok": true, "triggered": N }`（N = 实际触发压缩的角色数，0 = 无需压缩）。
    #[pyfunction]
    fn compress_context(vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::context::compress_context(vm)
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
        super::websocket::ws_open(conn_id, kwargs, vm)
    }

    /// 关闭一个 WebSocket 连接。
    ///
    /// 用法：`ws_close("gateway")`
    ///
    /// 关闭后可用 `ws_open` 重新连接。返回 `{ "ok": true }`。
    #[pyfunction]
    fn ws_close(conn_id: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        super::websocket::ws_close(conn_id, vm)
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
        super::websocket::ws_send(conn_id, data, vm)
    }

    /// 向已声明的 WS 连接发一条请求，并等它的响应帧。
    ///
    /// 用法：`ws_request("gateway", {"type": "get_status", "data": {}}, timeout_ms=5000)`
    ///
    /// `data` 必须是对象：宿主会在这层信封上补 `request_id` 再发出，对端原样带回后
    /// 按同一个 id 兑现。**会阻塞到响应或超时**，所以 `timeout_ms` 要明显小于本工具
    /// 自身的 `timeout_ms`，否则会被外层执行超时先杀掉。
    ///
    /// 返回 `{ "ok": true, "response": <响应帧完整对象> }`；失败一律
    /// `{ "ok": false, "error": "..." }`（未连接 / 超时 / 连接断开三种已归一化）。
    #[pyfunction]
    fn ws_request(
        conn_id: String,
        data: PyObjectRef,
        kwargs: KwArgs<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        super::websocket::ws_request(conn_id, data, kwargs, vm)
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
        super::websocket::ws_status(kwargs, vm)
    }
}

/// 获取插件宿主模块定义（供解释器注入）。
pub(crate) fn plugin_module_def(
    ctx: &rustpython_vm::Context,
) -> &'static rustpython_vm::builtins::PyModuleDef {
    plugin_host::module_def(ctx)
}
