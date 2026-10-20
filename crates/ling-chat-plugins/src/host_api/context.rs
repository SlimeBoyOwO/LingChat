//! 台词历史（上下文源）的读取、编辑与压缩。

use std::collections::HashMap;

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};
use tauri::{AppHandle, Manager};

use ling_chat_main::AppState;
use ling_chat_main::ai_service::tools::game_status_handle;
use ling_chat_main::ai_service::types::GameLine;
use ling_chat_main::utils::prompt::PromptRole;

use super::{kwargs_map, no_host, py_to_value, runtime, value_to_pyobject};

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

/// 追加一条提示台词（旁白 / 系统提示 / 剧情提示）。
///
/// `ttl_seconds > 0` 表示这条只是「此刻发生的事」：先留在队列里，等真的轮到角色开口
/// 时再判一次有效期，过期的直接丢弃`key` + `severity` 让同类事件在队列里只保留最严重的一条。
async fn add_prompt_line_impl(
    app: AppHandle,
    role: String,
    text: String,
    ttl_seconds: Option<f64>,
    key: Option<String>,
    severity: i64,
) -> serde_json::Value {
    let role = match role.trim().to_ascii_lowercase().as_str() {
        "narrator" => PromptRole::Narrator,
        "system" => PromptRole::System,
        "plot" => PromptRole::Plot,
        other => {
            return serde_json::json!({
                "ok": false,
                "error": format!("未知的提示角色：{other}（可用 narrator / system / plot）"),
            });
        },
    };
    let text = text.trim();
    if text.is_empty() {
        return serde_json::json!({ "ok": false, "error": "提示内容不能为空" });
    }

    let state = app.state::<AppState>();
    let db = state.db.clone();
    let gen_lock = state.generation_lock.clone();
    let gs = game_status_handle(&app).await;
    let mut gs = gs.lock().await;

    // 有有效期的条目一律先排队：什么时候轮到角色开口不由插件决定，等那一刻再判
    // 「还值不值得说」。排队条数回给插件，它据此判「攒够几件了」。
    if let Some(ttl) = ttl_seconds.filter(|seconds| *seconds > 0.0) {
        let pending = gs.queue_prompt_line(
            role,
            text,
            Some(std::time::Duration::from_secs_f64(ttl)),
            key,
            severity,
        );
        return serde_json::json!({ "ok": true, "queued": true, "pending": pending });
    }

    // 生成进行中先排队：直接写会往流式任务算好的行号后面插队，事件一多就会和
    // 正在生成的这一轮搅在一起。下一轮开头（拿到生成锁之后）再按顺序补写。
    let idle = gen_lock.try_lock();
    if idle.is_err() {
        gs.queue_prompt_line(role, text, None, None, 0);
        return serde_json::json!({ "ok": true, "queued": true });
    }
    match gs.add_prompt_line(&db, role, text).await {
        Ok(index) => serde_json::json!({ "ok": true, "index": index }),
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

pub(super) fn read_context(
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let (start, end) = kw_range(vm, &kwargs);
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = runtime().block_on(read_context_impl(app, start, end));
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn edit_context(
    replacement: PyObjectRef,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let (start, end) = kw_range(vm, &kwargs);
    let lines: Vec<GameLine> = match serde_json::from_value(py_to_value(vm, &replacement)) {
        Ok(lines) => lines,
        Err(e) => {
            return Ok(value_to_pyobject(
                vm,
                &serde_json::json!({
                    "ok": false,
                    "error": format!("replacement 解析为台词行失败: {e}"),
                }),
            ));
        },
    };
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = runtime().block_on(edit_context_impl(app, start, end, lines));
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn add_prompt_line(
    role: String,
    text: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let kwargs = kwargs_map(kwargs);
    let ttl_seconds = kwargs
        .get("ttl_seconds")
        .and_then(|v| py_to_value(vm, v).as_f64());
    let key = kwargs
        .get("key")
        .and_then(|v| py_to_value(vm, v).as_str().map(str::to_string));
    let severity = kwargs
        .get("severity")
        .and_then(|v| py_to_value(vm, v).as_i64())
        .unwrap_or(0);
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = runtime().block_on(add_prompt_line_impl(
        app,
        role,
        text,
        ttl_seconds,
        key,
        severity,
    ));
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn compress_context(vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = runtime().block_on(compress_context_impl(app));
    Ok(value_to_pyobject(vm, &result))
}
