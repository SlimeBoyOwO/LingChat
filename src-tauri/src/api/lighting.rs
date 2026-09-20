//! 光影控制命令与共享应用逻辑。
//!
//! 三条入口（设置面板 / 剧本 `lighting` 事件 / `lighting_apply` 工具）最终都走
//! [`apply_lighting`]，保证「后端状态 + 前端画面」一次广播同步到位。
//!
//! 渲染优先级在前端 `useLighting` 里落地：
//! 总开关关 → 无光影；否则 运行时覆盖 → 全局预设 → 场景自带。

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::ai_service::game_system::lighting_store::{self, LightingPreset};
use crate::ai_service::game_system::scene_store::{LightingOverride, LightingParams};
use crate::ai_service::tools::game_status_handle;

/// 前端监听的事件名。剧本事件走 `script:lighting`（进事件队列、受 duration 约束），
/// 手动面板与工具走这个即时事件。
pub const EVENT_LIGHTING_CHANGE: &str = "lighting:change";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingChangePayload {
    /// 生效的预设 id；自定义参数或清空覆盖时为 None
    pub preset: Option<String>,
    /// 已解析好的完整参数；None 表示回到「跟随场景」
    pub params: Option<LightingParams>,
    /// 谁改的：`panel` / `script` / `tool`
    pub source: String,
    /// 剧本事件的间隔秒数，仅 `script:lighting` 用；面板与工具不带
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyLightingRequest {
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub params: Option<LightingParams>,
}

/// 校验并解析预设名 → 完整参数 + 规范化 id。纯函数，不加锁不做 IO。
///
/// 剧本事件也要用它解析，但事件跑在 `ctx.game_status` 上、不能碰 `ai_service`
/// 锁，所以解析与落库必须分开。
pub fn resolve_request(
    requested: &LightingOverride,
) -> Result<(Option<String>, LightingParams), String> {
    let params = requested.resolve().ok_or_else(|| match &requested.preset {
        Some(preset) => {
            format!("未知光影预设: {preset}；可用值见 lighting_list_presets 工具返回值")
        },
        None => "lighting_apply 需要 preset 或 params".to_string(),
    })?;
    // 大小写纠正后再存，前端高亮与状态回读都拿规范 id。
    let preset = requested
        .preset
        .as_deref()
        .map(|raw| lighting_store::normalize_id(raw).unwrap_or(raw).to_string());
    Ok((preset, params))
}

/// 校验并解析预设名，写入运行时覆盖，然后广播。
///
/// `override_` 为 `None` 表示清空覆盖、回到「跟随场景」。未知预设直接报错——
/// 静默回退会让「灯光没变化」变成查不出原因的事。
pub async fn apply_lighting(
    app: &AppHandle,
    override_: Option<LightingOverride>,
    source: &str,
) -> Result<LightingChangePayload, String> {
    let Some(requested) = override_ else {
        return broadcast(app, None, source).await;
    };

    let (preset, params) = resolve_request(&requested)?;

    broadcast(
        app,
        Some(LightingChangePayload {
            preset,
            params: Some(params),
            source: source.to_string(),
            duration: None,
        }),
        source,
    )
    .await
}

/// 广播「已清掉覆盖、回到跟随场景」。
///
/// 剧本收尾单独用它：那时 `game_status` 已经改过，不必再走一遍带锁的
/// [`apply_lighting`]，只要把结果告诉主窗口和投屏窗口。
pub fn emit_cleared(app: &AppHandle, source: &str) {
    let payload = LightingChangePayload {
        preset: None,
        params: None,
        source: source.to_string(),
        duration: None,
    };
    let _ = crate::ai_service::message_system::events::emit(app, EVENT_LIGHTING_CHANGE, &payload);
}

/// 落库 + 广播。`payload` 为 None 即清空覆盖。
async fn broadcast(
    app: &AppHandle,
    payload: Option<LightingChangePayload>,
    source: &str,
) -> Result<LightingChangePayload, String> {
    let game_status = game_status_handle(app).await;
    {
        let mut gs = game_status.lock().await;
        match &payload {
            Some(p) => gs.set_lighting_override(
                LightingOverride {
                    preset: p.preset.clone(),
                    params: p.params.clone(),
                },
                source,
            ),
            None => {
                gs.clear_lighting_override();
            },
        }
    }

    let payload = payload.unwrap_or(LightingChangePayload {
        preset: None,
        params: None,
        source: source.to_string(),
        duration: None,
    });
    let _ = crate::ai_service::message_system::events::emit(app, EVENT_LIGHTING_CHANGE, &payload);
    Ok(payload)
}

/// 清空运行时覆盖，回到「跟随场景」。
pub async fn clear_lighting(app: &AppHandle, source: &str) -> Result<(), String> {
    broadcast(app, None, source).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingState {
    /// 当前运行时覆盖（已解析为完整参数）
    pub override_params: Option<LightingParams>,
    pub override_preset: Option<String>,
    pub override_source: String,
    /// 前端上报的「屏幕上真正在渲染的」预设 id
    pub active_preset: Option<String>,
    /// 生效来源：`tool` / `script` / `global` / `scene` / `off`
    pub active_source: String,
    /// 当前场景 id，前端据此取场景自带光影
    pub current_scene_id: Option<String>,
}

#[tauri::command]
pub async fn lighting_list_presets() -> Result<Vec<LightingPreset>, String> {
    Ok(lighting_store::presets())
}

#[tauri::command]
pub async fn lighting_apply(
    app: AppHandle,
    req: ApplyLightingRequest,
) -> Result<LightingChangePayload, String> {
    let override_ = match (req.preset.filter(|s| !s.is_empty()), req.params) {
        (None, None) => None,
        (preset, params) => Some(LightingOverride { preset, params }),
    };
    apply_lighting(&app, override_, "panel").await
}

#[tauri::command]
pub async fn lighting_clear(app: AppHandle) -> Result<(), String> {
    clear_lighting(&app, "panel").await
}

#[tauri::command]
pub async fn lighting_get(app: AppHandle) -> Result<LightingState, String> {
    let game_status = game_status_handle(&app).await;
    let gs = game_status.lock().await;
    let current = gs.lighting_override.clone();
    Ok(LightingState {
        override_params: current.as_ref().and_then(LightingOverride::resolve),
        override_preset: current.as_ref().and_then(|c| c.preset.clone()),
        override_source: gs.lighting_override_source.clone(),
        active_preset: gs.lighting_active_preset.clone(),
        active_source: gs.lighting_active_source.clone(),
        current_scene_id: gs.current_scene_id.clone(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportLightingActiveRequest {
    /// 前端解析出的生效预设 id；「跟随场景」或自定义参数时为 None
    #[serde(default)]
    pub preset: Option<String>,
    /// 生效来源：`tool` / `script` / `global` / `scene` / `off`
    pub source: String,
}

/// 前端上报「屏幕上真正在渲染的光影」。
///
/// 只回读、不再广播：这条状态就是前端自己算出来的，再发回去会让它重算一遍并
/// 再次上报，形成事件环。
#[tauri::command]
pub async fn lighting_report_active(
    app: AppHandle,
    req: ReportLightingActiveRequest,
) -> Result<(), String> {
    let game_status = game_status_handle(&app).await;
    let mut gs = game_status.lock().await;
    gs.set_lighting_active(req.preset.filter(|s| !s.is_empty()), &req.source);
    Ok(())
}
