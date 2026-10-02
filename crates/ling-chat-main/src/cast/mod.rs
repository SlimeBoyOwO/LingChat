//! 投屏（Screen Cast）模块。
//!
//! 把主聊天场景通过一个独立的「投屏窗口」串流给局域网内的物联网设备：
//! - 前端复用现有组件（GameBackground / GameRolesStage / GameDialog）渲染投屏窗口
//!   （Live2D、背景、打字机天然支持——就是真实 webview 里的真实组件）；
//! - 本模块用 `tauri_plugin_screenshots` 直接捕获该窗口画面，以 MJPEG 串流出去
//!   （`capture.rs` / `server.rs`）。
//!
//! 预留接口（v1 仅协议 + 桩，见 [mic]）：
//! - 远程音频同步播放（投屏客户端播 BGM / 角色语音）
//! - 麦克风 → 语音转文字（ASR）
//!
//! 前端命令：
//! - `cast_open_window` / `cast_close_window` — 开关投屏窗口
//! - `cast_start` / `cast_stop` — 启停 MJPEG 服务
//! - `cast_get_status` — 查询状态与局域网地址
//! - `cast_get_snapshot` — 读取当前场景快照（供投屏窗口打开时播种）

pub mod capture;
pub mod mic;
pub mod server;

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config;

/// 投屏窗口的 Tauri 窗口 label。
pub const CAST_WINDOW_LABEL: &str = "cast";

/// 投屏运行时状态（Tauri managed state）。
#[derive(Default)]
pub struct CastManager {
    /// MJPEG 服务是否在运行。
    pub server_running: Mutex<bool>,
    /// 实际绑定的端口。
    pub server_port: Mutex<Option<u16>>,
    /// 最近一次投屏镜像（主窗口台词/标题/情绪/背景/场景/角色），
    /// 由 `cast_emit_mirror` 存储并经 `cast:mirror` 广播；投屏窗口打开时用
    /// `cast_get_mirror` 读取播种，保证「打开晚于当前对话」也能同步。
    pub mirror: Mutex<Option<serde_json::Value>>,
}

/// `cast_get_status` 的返回结构。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CastStatusInfo {
    /// 设置里是否开启投屏（cast.enabled）
    pub enabled: bool,
    /// 设置里配置的端口
    pub port: u16,
    /// 设置里配置的帧率（1–30）
    pub fps: u8,
    /// 设置里配置的 JPEG 质量（1–100）
    pub quality: u8,
    /// 设置里配置的串流默认输出宽度（0 = 跟随投屏窗口）
    pub width: u32,
    /// 设置里配置的串流默认输出高度（0 = 跟随投屏窗口）
    pub height: u32,
    /// 是否启用 vivid 色彩增强（饱和度/对比度预设，串流编码时生效）
    pub vivid: bool,
    /// 投屏角色缩放倍率（1.0 = 原始大小）
    pub char_scale: f64,
    /// 投屏角色水平偏移（像素，正值右移）
    pub char_offset_x: f64,
    /// 投屏角色垂直偏移（像素，正值下移）
    pub char_offset_y: f64,
    /// 投屏对话框宽度百分比（100 = 无两侧留白）
    pub dialog_width: f64,
    /// 投屏对话框整体高度（占窗口高度百分比）
    pub dialog_height: f64,
    /// 投屏对话框字体大小（px）
    pub dialog_font_size: f64,
    /// 投屏对话框背景色透明度（0–100）
    pub dialog_bg_opacity: f64,
    /// 投屏隐藏对话框（true 时对话层不渲染，只留背景与角色舞台）
    pub dialog_hidden: bool,
    /// MJPEG 服务是否在运行
    pub running: bool,
    /// 投屏窗口是否已打开
    pub cast_window_open: bool,
    /// 本机局域网地址列表
    pub lan_urls: Vec<String>,
    /// 预览页地址（http://ip:port/）
    pub page_url: String,
    /// MJPEG 流地址（http://ip:port/stream）
    pub stream_url: String,
}

// ─── 设置读取 ──────────────────────────────────────────────

pub(crate) fn read_bool(app: &AppHandle, key: &str, default: bool) -> bool {
    config::settings_store(app)
        .ok()
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

pub(crate) fn read_u64(app: &AppHandle, key: &str, default: u64) -> u64 {
    config::settings_store(app)
        .ok()
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_u64())
        .unwrap_or(default)
}

pub(crate) fn read_f64(app: &AppHandle, key: &str, default: f64) -> f64 {
    config::settings_store(app)
        .ok()
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
}

// ─── 内部逻辑（命令与 lib.rs 启动自启共用） ────────────────

/// 打开投屏窗口（幂等：已存在则聚焦）。
pub fn open_cast_window(app: &AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder};

        if let Some(win) = app.get_webview_window(CAST_WINDOW_LABEL) {
            let _ = win.unminimize();
            win.set_focus()
                .map_err(|e| format!("聚焦投屏窗口失败: {e}"))?;
            return Ok(());
        }

        let win = WebviewWindowBuilder::new(
            app,
            CAST_WINDOW_LABEL,
            WebviewUrl::App("index.html?window=cast".into()),
        )
        .title("LingChat 投屏")
        .inner_size(800.0, 450.0)
        .min_inner_size(320.0, 180.0)
        // 去掉原生标题栏：串流画面直接抓窗口整个 HWND，
        // 留着标题栏会被一起抓进画面（用户反馈的「包含标题等窗口其他部分」）。
        // 窗口仍可拖拽（前端 data-tauri-drag-region 拖拽条）与边缘缩放。
        .decorations(false)
        // 关掉系统阴影：无边框窗口默认会带 DWM 投影，被 capture_own_window 整窗
        // 抓进 MJPEG 就成了画面四周的黑色边框（主窗/桌宠 tauri.conf.json 也是 shadow: false）。
        .shadow(false)
        .build()
        .map_err(|e| format!("创建投屏窗口失败: {e}"))?;

        // 广播窗口关闭状态（设置页 / 投屏页可监听）
        let app_for_event = app.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Destroyed = event {
                let _ = app_for_event.emit("cast-window:state", false);
            }
        });
        let _ = app.emit("cast-window:state", true);

        tracing::info!("[Cast] 投屏窗口已创建");
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
        return Err("投屏窗口仅支持桌面端".to_string());
    }
    Ok(())
}

/// 启动投屏服务：确保投屏窗口打开 + 启动 MJPEG 服务。返回实际端口。
pub async fn start_cast_server(app: &AppHandle, state: &CastManager) -> Result<u16, String> {
    {
        let running = state
            .server_running
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        if *running {
            return Err("投屏服务已在运行中".to_string());
        }
    }

    // 串流的是投屏窗口的画面，先确保窗口存在
    if let Err(e) = open_cast_window(app) {
        tracing::warn!("[Cast] 启动时打开投屏窗口失败（继续启动服务）: {e}");
    }

    let port = read_u64(app, config::keys::CAST_PORT, server::DEFAULT_PORT as u64) as u16;
    let fps = read_u64(app, config::keys::CAST_FPS, server::DEFAULT_FPS as u64).clamp(1, 30) as u8;
    let quality = read_u64(
        app,
        config::keys::CAST_QUALITY,
        server::DEFAULT_QUALITY as u64,
    )
    .clamp(1, 100) as u8;

    let actual = server::start_server(app.clone(), port, fps, quality).await?;

    {
        let mut running = state
            .server_running
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *running = true;
    }
    {
        let mut bound = state
            .server_port
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *bound = Some(actual);
    }
    tracing::info!("[Cast] 投屏服务已启动（端口 {actual}，{fps}fps，quality={quality}）");
    Ok(actual)
}

/// 停止投屏服务。
pub async fn stop_cast_server(state: &CastManager) -> Result<(), String> {
    {
        let running = state
            .server_running
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        if !*running {
            return Err("投屏服务未在运行".to_string());
        }
    }
    server::stop_server().await?;
    {
        let mut running = state
            .server_running
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *running = false;
    }
    {
        let mut bound = state
            .server_port
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *bound = None;
    }
    tracing::info!("[Cast] 投屏服务已停止");
    Ok(())
}

/// 构建状态信息。
fn build_status(app: &AppHandle, state: &CastManager) -> CastStatusInfo {
    let enabled = read_bool(app, config::keys::CAST_ENABLED, false);
    let port = read_u64(app, config::keys::CAST_PORT, server::DEFAULT_PORT as u64) as u16;
    let fps = read_u64(app, config::keys::CAST_FPS, server::DEFAULT_FPS as u64).clamp(1, 30) as u8;
    let quality = read_u64(
        app,
        config::keys::CAST_QUALITY,
        server::DEFAULT_QUALITY as u64,
    )
    .clamp(1, 100) as u8;
    // 串流默认输出分辨率（0 = 跟随投屏窗口当前尺寸）
    let width = read_u64(app, config::keys::CAST_WIDTH, 0) as u32;
    let height = read_u64(app, config::keys::CAST_HEIGHT, 0) as u32;
    // vivid 色彩增强开关
    let vivid = read_bool(app, config::keys::CAST_VIVID, false);
    // 角色缩放 / 偏移与对话框尺寸（投屏窗口渲染调参）
    let char_scale = read_f64(app, config::keys::CAST_CHAR_SCALE, 1.0);
    let char_offset_x = read_f64(app, config::keys::CAST_CHAR_OFFSET_X, 0.0);
    let char_offset_y = read_f64(app, config::keys::CAST_CHAR_OFFSET_Y, 0.0);
    let dialog_width = read_f64(app, config::keys::CAST_DIALOG_WIDTH, 70.0);
    let dialog_height = read_f64(app, config::keys::CAST_DIALOG_HEIGHT, 40.0);
    let dialog_font_size = read_f64(app, config::keys::CAST_DIALOG_FONT_SIZE, 20.0);
    let dialog_bg_opacity = read_f64(app, config::keys::CAST_DIALOG_BG_OPACITY, 70.0);
    let dialog_hidden = read_bool(app, config::keys::CAST_DIALOG_HIDDEN, false);
    let running = state.server_running.lock().map(|g| *g).unwrap_or(false);
    let cast_window_open = app.get_webview_window(CAST_WINDOW_LABEL).is_some();

    let ips = crate::lan_sync::discovery::get_local_ips().unwrap_or_default();
    let host = ips.first().map(String::as_str).unwrap_or("127.0.0.1");
    // 设置了输出分辨率时把 ?w=&h= 直接拼进流地址，客户端照抄即可生效
    let mut stream_url = format!("http://{host}:{port}/stream");
    if width > 0 && height > 0 {
        stream_url.push_str(&format!("?w={width}&h={height}"));
    }
    let page_url = format!("http://{host}:{port}/");

    CastStatusInfo {
        enabled,
        port,
        fps,
        quality,
        width,
        height,
        vivid,
        char_scale,
        char_offset_x,
        char_offset_y,
        dialog_width,
        dialog_height,
        dialog_font_size,
        dialog_bg_opacity,
        dialog_hidden,
        running,
        cast_window_open,
        lan_urls: ips,
        page_url,
        stream_url,
    }
}

// ─── Tauri 命令 ────────────────────────────────────────────

/// 打开投屏窗口（已存在则聚焦）。
#[tauri::command]
pub async fn cast_open_window(app: AppHandle) -> Result<(), String> {
    open_cast_window(&app)
}

/// 关闭投屏窗口。
#[tauri::command]
pub async fn cast_close_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(CAST_WINDOW_LABEL) {
        win.close().map_err(|e| format!("关闭投屏窗口失败: {e}"))?;
    }
    Ok(())
}

/// 启动投屏服务（MJPEG），返回实际端口。
#[tauri::command]
pub async fn cast_start(app: AppHandle, state: State<'_, CastManager>) -> Result<u16, String> {
    start_cast_server(&app, &state).await
}

/// 停止投屏服务。
#[tauri::command]
pub async fn cast_stop(state: State<'_, CastManager>) -> Result<(), String> {
    stop_cast_server(&state).await
}

/// 查询投屏状态（服务运行 / 窗口状态 / 局域网地址）。
#[tauri::command]
pub async fn cast_get_status(
    app: AppHandle,
    state: State<'_, CastManager>,
) -> Result<CastStatusInfo, String> {
    Ok(build_status(&app, &state))
}

/// 读取当前场景快照，供投屏窗口打开时播种初始状态。
#[tauri::command]
pub async fn cast_get_snapshot(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::AppState>();
    let service = state.ai_service.lock().await;
    let gs = service.game_status.lock().await;

    // 当前对话台词：最近一条角色（assistant）台词
    let line = gs
        .line_list
        .iter()
        .rev()
        .find(|gl| {
            gl.base.sender_role_id.is_some()
                && matches!(
                    gl.base.attribute.inner(),
                    crate::db::entities::line::LineAttribute::Assistant
                )
        })
        .map(|gl| {
            serde_json::json!({
                "speaker": gl.base.display_name.clone()
                    .unwrap_or_else(|| gl.base.sender_role_id.map(|id| id.to_string()).unwrap_or_default()),
                "text": gl.base.content.clone(),
                "emotion": gl.base.predicted_emotion.clone()
                    .or_else(|| gl.base.original_emotion.clone())
                    .unwrap_or_default(),
                "motionText": gl.base.action_content.clone().unwrap_or_default(),
            })
        });

    // 在场角色顺序：onstage_role_ids（有序）优先；为空退回 present_role_ids
    let role_ids: Vec<i32> = if !gs.onstage_role_ids.is_empty() {
        gs.onstage_role_ids.clone()
    } else {
        gs.present_role_ids.iter().cloned().collect()
    };

    Ok(serde_json::json!({
        "background": gs.background,
        "backgroundEffect": gs.background_effect,
        "presentPic": gs.present_pic,
        "currentSceneId": gs.current_scene_id,
        "presentRoleIds": role_ids,
        "currentRoleId": gs.current_role_id,
        "line": line,
    }))
}

/// 投屏镜像上报：主窗口台词/标题/情绪/背景/场景/角色发生变化时调用。
/// 把主窗口「当前正在显示的台词」经本中继存储并广播给投屏窗口——
/// 投屏窗口与主窗口是各自独立的 webview/事件队列，若投屏自己消费 ai:reply
/// 就会「按消息到达时间显示」而非与主界面逐句同步。经镜像后投屏完全跟随主窗口。
#[tauri::command]
pub async fn cast_emit_mirror(
    app: AppHandle,
    state: State<'_, CastManager>,
    mirror: serde_json::Value,
) -> Result<(), String> {
    {
        let mut stored = state.mirror.lock().map_err(|e| format!("锁失败: {e}"))?;
        *stored = Some(mirror.clone());
    }
    app.emit("cast:mirror", mirror)
        .map_err(|e| format!("广播投屏镜像失败: {e}"))?;
    Ok(())
}

/// 读取最近一次投屏镜像（投屏窗口打开时播种初始状态）。
#[tauri::command]
pub async fn cast_get_mirror(
    state: State<'_, CastManager>,
) -> Result<Option<serde_json::Value>, String> {
    state
        .mirror
        .lock()
        .map(|g| g.clone())
        .map_err(|e| format!("锁失败: {e}"))
}

/// 前端触发对话/播放回复音频时调用：把该句语音（`cast:audio` play 帧）
/// 广播给所有投屏 WS 客户端，供远端设备同步播放。
/// 投屏服务未运行时安全 no-op。
#[tauri::command]
pub async fn cast_play_voice(audio_file: String) -> Result<(), String> {
    server::broadcast_voice_play(&audio_file).await
}
