//! 世界动作：改变舞台状态的后端唯一入口。
//!
//! 切场景与切粒子特效此前在命令、聊天工具、天气同步里各有一份实现，
//! 状态写入、持久化、事件广播抄了多遍。这里只收这三件事；输入解析、
//! 手动优先锁、特效名校验等策略仍留在调用方。

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::game_system::script_engine::events::known_effects::is_weather_effect;
use crate::api::data_dir;

/// 世界动作广播的事件名。前端仍使用同名字面量，改动需两边同步。
pub const EVENT_SCENE_SWITCH: &str = "scene:switch";
pub const EVENT_AMBIENT_EFFECT: &str = "ambient:effect";
pub const EVENT_CHARACTER_SWITCH: &str = "character:switch";
pub const EVENT_CLOTHES_CHANGED: &str = "character:clothes-changed";

/// 取出共享的 `GameStatus` 句柄。锁顺序与全仓一致：先 `ai_service`，克隆后释放。
async fn game_status_handle(app: &AppHandle) -> Arc<Mutex<GameStatus>> {
    let state = app.state::<crate::AppState>();
    let service = state.ai_service.lock().await;
    service.game_status.clone()
}

/// 切换当前场景；`scene_id` 为 `None` 表示取消场景。
pub async fn switch_scene(app: &AppHandle, scene_id: Option<String>) {
    // 读盘放在锁外
    let scene_info = scene_id
        .as_deref()
        .and_then(|id| SceneStore::new(&data_dir()).find_by_id(id).ok().flatten())
        .map(|scene| crate::api::scene::model_to_info(&scene));
    let background = scene_info.as_ref().and_then(|info| info.background.clone());

    {
        let gs = game_status_handle(app).await;
        let mut gs = gs.lock().await;
        gs.current_scene_id = scene_id.clone();
        // 换了场景，剧本临时背景的记录就此作废
        gs.clear_background_override();
        gs.sync_background_with_scene(background);
    }

    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        let value = match &scene_id {
            Some(id) => serde_json::Value::String(id.clone()),
            None => serde_json::Value::Null,
        };
        store.set(crate::config::session::LAST_SCENE_ID.to_string(), value);
        let _ = store.save();
    }

    let Some(scene) = scene_info else {
        return;
    };
    let payload = serde_json::json!({
        "type": "scene_switch",
        "scene": scene,
    });
    if let Err(e) = app.emit(EVENT_SCENE_SWITCH, &payload) {
        tracing::warn!("emit scene:switch 失败: {e}");
    }
}

/// 粒子特效来源：决定写哪个状态字段、发哪种 payload。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOrigin {
    /// 用户在设置页手动选择氛围层
    Manual,
    /// 用户在设置页手动选择天气层
    ManualWeather,
    /// 聊天模型调用工具
    Ai,
    /// 天气自动同步
    Weather,
}

/// 清空特效的约定值。
fn is_clearing_effect(effect: &str) -> bool {
    matches!(effect, "None" | "none" | "")
}

/// 应用粒子特效。
///
/// 落在哪一层由特效名决定：天气名进 `weather_effect`，其余进 `background_effect`，
/// 与前端按注册表分流的规则一致。清空按来源清对应的层：AI 清两层，设置页与天气同步
/// 只清自己那一层。payload 字段保持各来源原样，前端按字段分流。
pub async fn apply_effect(
    app: &AppHandle,
    origin: EffectOrigin,
    effect: &str,
    duration: Option<f64>,
) {
    {
        let gs = game_status_handle(app).await;
        let mut gs = gs.lock().await;
        if is_weather_effect(effect) {
            gs.weather_effect = effect.to_string();
        } else if is_clearing_effect(effect) {
            match origin {
                // 前端收到 AI 的清空会同时清两层，后端保持一致
                EffectOrigin::Ai => {
                    gs.background_effect = "none".to_string();
                    gs.weather_effect = "None".to_string();
                },
                // 设置页的「无特效」只作用于各自的层
                EffectOrigin::Manual => gs.background_effect = effect.to_string(),
                EffectOrigin::ManualWeather | EffectOrigin::Weather => {
                    gs.weather_effect = effect.to_string()
                },
            }
        } else {
            gs.background_effect = effect.to_string();
        }
    }

    let payload = match origin {
        EffectOrigin::Manual => serde_json::json!({
            "type": "ambient_effect",
            "effect": effect,
            "source": "manual",
        }),
        EffectOrigin::Ai => serde_json::json!({
            "type": "ambient_effect",
            "effect": effect,
            "duration": duration,
        }),
        // 带 layer 的广播只改天气层，不动氛围层
        EffectOrigin::ManualWeather => serde_json::json!({
            "type": "ambient_effect",
            "effect": effect,
            "layer": "weather",
        }),
        EffectOrigin::Weather => serde_json::json!({
            "type": "ambient_effect",
            "effect": effect,
            "duration": serde_json::Value::Null,
            "layer": "weather",
        }),
    };
    if let Err(e) = app.emit(EVENT_AMBIENT_EFFECT, &payload) {
        tracing::warn!("emit ambient effect 失败: {e}");
    }
}
