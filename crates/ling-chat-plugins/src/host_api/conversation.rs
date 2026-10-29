//! 送消息、抓屏与请求主动开口。

use std::sync::Arc;

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};
use tauri::{AppHandle, Manager};

use ling_chat_main::AppState;
use ling_chat_main::ai_service::proactive_system::ProactiveSystem;

use super::guard::{
    current_capture_allow, current_plugin, current_proactive_allow, current_send_allow,
};
use super::{err_json, kwargs_map, no_host, py_to_value, runtime, value_to_pyobject};
use crate::PluginManager;

/// 请求宿主主动开一次口（主动对话系统的一次投放）。
///
/// `text` 为空表示只开一次口、不写任何提示行：要说的内容由插件自己记成旁白，
/// 再补一句「该注意什么、该不该开口」等于替角色拿主意。
/// `urgent` 用于死亡这类必须马上开口的事：不吃冷却与上一轮的静默期。
///
/// 每次都往宿主日志留一行：拒绝原因只回给插件，而插件的 print 不进日志文件，
/// 于是「什么都没发生」时这里是唯一的线索。
async fn trigger_proactive_impl(app: AppHandle, text: String, urgent: bool) -> serde_json::Value {
    let text = text.trim().to_string();
    let plugin = current_plugin().unwrap_or_else(|| "未知插件".to_string());
    let head = if text.is_empty() {
        "（不带提示语）".to_string()
    } else {
        text.chars().take(24).collect()
    };
    let Some(proactive) = app.state::<AppState>().proactive_system.clone() else {
        tracing::warn!(plugin = %plugin, "插件请求主动开口被拒：主动对话系统未初始化");
        return serde_json::json!({ "ok": false, "error": "主动对话系统未初始化" });
    };
    match ProactiveSystem::request_deliver(proactive, text, urgent).await {
        Ok(()) => {
            tracing::info!(plugin = %plugin, urgent, "插件请求主动开口已受理：{head}");
            serde_json::json!({ "ok": true, "triggered": true })
        },
        Err(reason) => {
            tracing::info!(plugin = %plugin, reason, urgent, "插件请求主动开口被拒：{head}");
            serde_json::json!({ "ok": true, "triggered": false, "reason": reason })
        },
    }
}

/// 抓一张屏幕画面，返回 JPEG 的 data URL。
///
/// 抓哪个窗口由插件给的关键词决定，找不到（或抓回来的不像一张画）就退回整个桌面。
/// 具体实现与压缩档位在 `screen_analyzer::capture_window_as_jpeg`，那边已经处理了
/// 「最小化后只剩标题栏一条」「独显全屏只给回黑图」这类坏帧。
///
/// 当前模型没开原生识图时直接失败：抓了也送不进对话，不如把原因给插件，让它如实回话。
fn capture_screen_impl(app: &AppHandle, window: &str) -> serde_json::Value {
    use base64::Engine as _;

    if !ling_chat_main::ai_service::llm::provider_config::native_vision_enabled(app) {
        return serde_json::json!({
            "ok": false,
            "error": "当前对话模型没有开启原生识图，看不到图片",
        });
    }
    match ling_chat_main::ai_service::screen_analyzer::capture_window_as_jpeg(window) {
        Some(bytes) => serde_json::json!({
            "ok": true,
            "image": format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            ),
        }),
        None => serde_json::json!({ "ok": false, "error": "抓不到画面（窗口与桌面都没取到）" }),
    }
}

/// 把一条用户消息送进对话（等同玩家在输入框里发了一句）。
///
/// 复用玩家那条入口 `send_chat_message`，行为完全一致：写进对话、走当前配置
/// 生成回复，该有的记忆与工具都会生效。生成在后台跑，这里不等它——插件 handler
/// 不该被一轮回复卡住。
async fn send_user_message_impl(app: AppHandle, text: String) -> serde_json::Value {
    match ling_chat_main::api::chat::send_chat_message(app, text, None).await {
        Ok(()) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    }
}

pub(super) fn send_user_message(text: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    if !current_send_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明 send_user_message 能力（需在 manifest.toml 顶层写 send_user_message = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let result = runtime().block_on(send_user_message_impl(app, text));
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn capture_screen(
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_capture_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明 capture_screen 能力（需在 manifest.toml 顶层写 capture_screen = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let kwargs = kwargs_map(kwargs);
    let window = kwargs
        .get("window")
        .and_then(|v| py_to_value(vm, v).as_str().map(str::to_string))
        .unwrap_or_default();
    let result = capture_screen_impl(&app, &window);
    tracing::info!(
        "[Plugin] capture_screen 窗口关键词={window:?} 结果={}",
        if result["ok"].as_bool().unwrap_or(false) {
            "ok"
        } else {
            "失败"
        }
    );
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn trigger_proactive(
    text: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_proactive_allow() {
        tracing::warn!(
            plugin = %current_plugin().unwrap_or_default(),
            "插件调用 trigger_proactive 被拒：未在 manifest 顶层声明该能力"
        );
        return Ok(err_json(
            vm,
            "本插件未声明 trigger_proactive 能力（需在 manifest.toml 顶层写 trigger_proactive = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let urgent = kwargs_map(kwargs)
        .get("urgent")
        .map(|v| py_to_value(vm, v).as_bool().unwrap_or(false))
        .unwrap_or(false);
    let result = runtime().block_on(trigger_proactive_impl(app, text, urgent));
    Ok(value_to_pyobject(vm, &result))
}

/// 插件报告外部程序此刻能不能播这句语音：女仆死了，模组找不到她就播不出来，
/// 宿主下一句改回本机播放，不必玩家去关开关。
pub(super) fn set_voice_playback_available(
    available: bool,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin) = current_plugin() else {
        return Ok(err_json(vm, "宿主不知道当前是哪个插件"));
    };
    let manager = app.state::<Arc<PluginManager>>().inner().clone();
    manager.set_voice_playback_available(&plugin, available);
    Ok(value_to_pyobject(vm, &serde_json::json!({ "ok": true })))
}
