//! 开机自启动（角色桌宠 + TTS 联动）API 命令。
//!
//! 对应 `_docs/feature-issue-auto-startup-pet.md`：
//! - `autostart_status`：查询系统自启状态 + 自启动相关配置 + 本次是否由系统自启触发。
//! - `autostart_set_enabled`：切换「开机自启动」（写入系统注册项 / LaunchAgent / .desktop）。
//! - `autostart_boot_apply`：开机自启时按当前角色的 TTS 类型拉起外部语音服务，探测就绪后自动刷新 TTS。
//!
//! 说明：区分「系统自启」与「手动启动」——只有带 `--autostart` 参数（由 autostart 插件写入
//! 的自启命令带上）的这次启动才进入桌宠模式；用户手动双击 exe 不带该参数，走主菜单。
//!
//! 采用官方 `tauri-plugin-autostart`（桌面端），Windows 走注册表 Run 键、macOS 走 LaunchAgent、
//! Linux 走 .desktop。

use serde::Serialize;
use tauri::{AppHandle, Manager};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

use crate::AppState;
use crate::config::{self, keys};

/// 自启动当前状态（供设置面板展示与前端启动决策）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AutostartStatus {
    /// 系统级开机自启动是否已启用（真实写入注册表 / LaunchAgent / .desktop）。
    pub system_enabled: bool,
    /// 自启动后是否直接进入桌宠模式。
    pub boot_as_pet: bool,
    /// 开机自启动默认加载的角色 ID（为空则沿用上次角色）。
    pub pet_role_id: String,
    /// 用于拉起外部 TTS 服务的启动脚本（.bat）路径，可为空。
    pub tts_launcher_bat: String,
    /// 本次启动是否由「系统开机自启」触发（带 --autostart 参数）。
    /// 只有为 true 时，boot_as_pet 才会生效；手动启动一律进主菜单。
    pub launched_by_autostart: bool,
    /// 进入桌宠时是否默认开启自动对话（自动播放/推进对话）。
    pub auto_play: bool,
    /// 手动启动（非开机自启）时是否以桌宠模式进入。
    pub startup_pet_mode: bool,
    /// 进入桌宠时是否发出「入场问候」（角色主动问候；默认关闭）。
    pub startup_greeting: bool,
    /// 启动时是否自动拉起/刷新外部 TTS API 服务（全局：无论桌宠还是正常启动均生效）。
    pub auto_start_tts: bool,
}

/// `autostart_boot_apply` 的返回结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AutostartBootResult {
    /// 启动角色当前使用的 TTS 类型（对应角色的 `settings.tts_type`）。
    pub tts_type: String,
    /// 是否判定为「内置本地 TTS 引擎」（无需拉起脚本）。
    pub embedded: bool,
    /// 本次是否实际执行了启动脚本。
    pub launched: bool,
    /// 语音服务是否已就绪（内置引擎恒为 true；外部引擎为探测后的可达结果）。
    pub ready: bool,
    /// 若执行脚本失败 / 服务未能就绪，存放错误信息。
    pub error: Option<String>,
}

/// 从 settings store 读取布尔配置（兼容 Bool 与 "true"/"false" 字符串两种存储形态）。
fn read_bool(app: &AppHandle, key: &str) -> bool {
    config::settings_store(app)
        .ok()
        .and_then(|s| s.get(key))
        .and_then(|v| match v {
            serde_json::Value::Bool(b) => Some(b),
            serde_json::Value::String(s) => Some(s == "true"),
            _ => None,
        })
        .unwrap_or(false)
}

/// 从 settings store 读取字符串配置（兼容 String 与 Number —— 角色 ID 可能被存为数字）。
fn read_string(app: &AppHandle, key: &str) -> String {
    config::settings_store(app)
        .ok()
        .and_then(|s| s.get(key))
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

/// 读取自启动相关配置（从 settings store，不经系统注册表）。
fn read_config(app: &AppHandle) -> AutostartStatus {
    let boot_as_pet = read_bool(app, keys::AUTOSTART_BOOT_AS_PET);
    let pet_role_id = read_string(app, keys::AUTOSTART_PET_ROLE_ID);
    let tts_launcher_bat = read_string(app, keys::AUTOSTART_TTS_LAUNCHER_BAT);

    let auto_play = read_bool(app, keys::STARTUP_AUTO_PLAY);
    let startup_pet_mode = read_bool(app, keys::STARTUP_PET_MODE);
    let startup_greeting = read_bool(app, keys::STARTUP_GREETING);
    let auto_start_tts = read_bool(app, keys::STARTUP_AUTO_START_TTS);

    let system_enabled = system_autostart_enabled(app);
    let launched_by_autostart = std::env::args().any(|a| a == "--autostart");

    AutostartStatus {
        system_enabled,
        boot_as_pet,
        pet_role_id,
        tts_launcher_bat,
        launched_by_autostart,
        auto_play,
        startup_pet_mode,
        startup_greeting,
        auto_start_tts,
    }
}

// ========== 系统自启动（tauri-plugin-autostart） ==========

/// 查询系统级自动启动是否已开启。
#[cfg(desktop)]
fn system_autostart_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// 移动端没有开机自启，一律视为未开启。
#[cfg(not(desktop))]
fn system_autostart_enabled(_app: &AppHandle) -> bool {
    false
}

/// 开启 / 关闭系统自启动（供 `autostart_set_enabled` 与设置保存流程复用）。
#[cfg(desktop)]
pub fn set_system_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable().map_err(|e| format!("开启开机自启动失败: {e}"))
    } else {
        mgr.disable()
            .map_err(|e| format!("关闭开机自启动失败: {e}"))
    }
}

/// 移动端跳过系统自启动写入。
#[cfg(not(desktop))]
pub fn set_system_autostart(_app: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        Err("开机自启动目前仅支持桌面端".to_string())
    } else {
        Ok(())
    }
}

// ========== Tauri 命令 ==========

/// 查询自启动状态。
#[tauri::command]
pub fn autostart_status(app: AppHandle) -> Result<AutostartStatus, String> {
    Ok(read_config(&app))
}

/// 切换「开机自启动」开关。开启时写入系统自启动项，同时持久化配置。
#[tauri::command]
pub fn autostart_set_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    set_system_autostart(&app, enabled)?;

    let store = config::settings_store(&app).map_err(|e| e.to_string())?;
    store.set(
        keys::AUTOSTART_ENABLED.to_string(),
        serde_json::Value::Bool(enabled),
    );
    store.save().map_err(|e| e.to_string())?;

    Ok(())
}

/// 开机自启（带 `--autostart` 的启动）时调用：按当前角色的 TTS 类型拉起外部语音服务，
/// 并探测服务可达；就绪后自动刷新 TTS，让桌宠一开机就能出声。
///
/// - 内置本地 TTS 引擎（`localsbv2api`）或未配置 tts_type → 直接就绪，无需拉起。
/// - 其余（simple-vits / SBV2 / GPT-SoVITS / FISH / OpenTTS 等外部分离服务）→
///   执行启动脚本，轮询探测服务可达性，就绪后刷新 TTS。
#[tauri::command]
pub async fn autostart_boot_apply(
    app: AppHandle,
    role_id: Option<i32>,
) -> Result<AutostartBootResult, String> {
    let config = read_config(&app);

    let tts_type = resolve_role_tts_type(&app, role_id)
        .await
        .unwrap_or_default();
    let embedded = tts_type.is_empty() || tts_type == "localsbv2api";

    // 内置引擎无需拉起外部服务，直接视为就绪
    if embedded {
        return Ok(AutostartBootResult {
            tts_type,
            embedded: true,
            launched: false,
            ready: true,
            error: None,
        });
    }

    // 外部 TTS 模式
    let bat = config.tts_launcher_bat.trim().to_string();
    if bat.is_empty() {
        // 未配置启动脚本 → 不检测 API（避免无 TTS 配置时一直卡 loading），视为无需等待
        return Ok(AutostartBootResult {
            tts_type,
            embedded: false,
            launched: false,
            ready: true,
            error: None,
        });
    }

    let probe_url = resolve_probe_url(&app, &tts_type);

    // 先探测一次：若服务已在运行（用户可能事先手动打开过），则不重复拉起启动脚本
    let already_running = match &probe_url {
        Some(url) => wait_tts_ready(url, 1).await,
        None => false,
    };
    if already_running {
        refresh_tts(&app).await;
        return Ok(AutostartBootResult {
            tts_type,
            embedded: false,
            launched: false,
            ready: true,
            error: None,
        });
    }

    // 服务未运行，才执行启动脚本拉起
    spawn_launcher(&bat)?;

    // 轮询探测服务可达性（最多 5 次 × 2s ≈ 10s），超时也放行，避免 loading 长时间卡住
    let ready = match &probe_url {
        Some(url) => wait_tts_ready(url, 5).await,
        None => false,
    };

    if ready {
        refresh_tts(&app).await;
    }

    Ok(AutostartBootResult {
        tts_type,
        embedded: false,
        launched: true,
        ready,
        error: if ready {
            None
        } else {
            Some("语音服务可能仍在启动，稍后会自动重试".to_string())
        },
    })
}

/// 从数据库读取角色当前使用的 TTS 类型。
async fn resolve_role_tts_type(app: &AppHandle, role_id: Option<i32>) -> Option<String> {
    // 未传 roleId（如正常启动、尚未进入桌宠）时，用后端已加载的当前角色，
    // 这样也能判断是否需要拉起外部 TTS API 服务。
    let role_id = match role_id {
        Some(id) if id > 0 => id,
        _ => {
            let state = app.state::<AppState>();
            // 先把 game_status 的 Arc 单独取出来，避免临时锁 guard 被提前释放导致借用错误
            let gs = {
                let svc = state.ai_service.lock().await;
                svc.game_status.clone()
            };
            let current = {
                let gs = gs.lock().await;
                gs.current_role_id
            };
            current?
        },
    };
    let state = app.state::<AppState>();
    let db = &state.db;
    let data_dir = crate::api::data_dir();
    let settings =
        crate::db::managers::role_repo::RoleRepo::get_role_settings_by_id(db, &data_dir, role_id)
            .await
            .ok()?;
    settings?.tts_type
}

/// 根据 TTS 类型解析出对应的服务 URL（用于探测可达性）。
fn resolve_probe_url(app: &AppHandle, tts_type: &str) -> Option<String> {
    let cfg = crate::config::tts::TtsConfig::load(app);
    let url = match tts_type {
        "sva-vits" => &cfg.simple_vits_api_url,
        "sbv2" => &cfg.sbv2_api_url,
        "sbv2api" => &cfg.sbv2api_api_url,
        "sva-bv2" => &cfg.bv2_api_url,
        "gsv" => &cfg.gsv_api_url,
        "aivis" => &cfg.aivis_api_url,
        "indextts2" => &cfg.indextts_api_url,
        "opentts" => &cfg.opentts_api_url,
        "fishs2" => &cfg.fish_s2_api_url,
        _ => return None,
    };
    if url.trim().is_empty() {
        None
    } else {
        Some(url.clone())
    }
}

/// 用短超时 GET 反复探测目标 URL，直到连接成功或次数用尽。
async fn wait_tts_ready(url: &str, attempts: u32) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };
    for _ in 0..attempts {
        // 只要拿到 HTTP 响应（哪怕 4xx/5xx）即认为服务已监听
        if client.get(url).send().await.is_ok() {
            tracing::info!("[Autostart] TTS 服务已就绪: {url}");
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    false
}

/// 语音服务就绪后重新启用所有语音生成器（reactivate TTS）。
async fn refresh_tts(app: &AppHandle) {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    service
        .game_status
        .lock()
        .await
        .reactivate_all_voice_makers();
    tracing::info!("[Autostart] 已刷新 TTS 语音生成器");
}

/// 执行启动脚本。Windows 用 `cmd /C start "" <bat>` 避免带空格路径被拆分；
/// 其他桌面平台直接 spawn 可执行文件。
fn spawn_launcher(bat_path: &str) -> Result<(), String> {
    let path = std::path::PathBuf::from(bat_path);
    if !path.exists() {
        return Err(format!("启动脚本不存在: {bat_path}"));
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/C", "start", "", bat_path])
            .spawn()
            .map_err(|e| format!("启动脚本执行失败: {e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command;
        Command::new(&path)
            .spawn()
            .map_err(|e| format!("启动脚本执行失败: {e}"))?;
    }

    tracing::info!("已拉起外部 TTS 服务启动脚本: {bat_path}");
    Ok(())
}
