use crate::bridge::{PetBridge, PetBridgeExt};
#[cfg(not(target_os = "android"))]
use crate::error::PetError;
use crate::state::CachedPetState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};

/// 悬浮叠加层权限状态。
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPermissionStatus {
    Granted,
    Denied,
    Unknown,
    /// 当前平台不支持悬浮桌宠（桌面 / iOS）。
    Unsupported,
}

/// WebView -> 桌宠 的完整状态负载。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetStatePayload {
    pub character: Option<CharacterInfo>,
    pub dialogue: Option<DialogueInfo>,
    pub scale: Option<f64>,
    pub volume: Option<u32>,
    pub background_effect: Option<String>,
    pub visible: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterInfo {
    pub id: String,
    pub name: String,
    pub avatar_url: String,
    pub expression: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogueInfo {
    pub text: String,
    pub is_typing: bool,
    pub audio_playing: bool,
}

impl From<&PetStatePayload> for CachedPetState {
    fn from(p: &PetStatePayload) -> Self {
        Self {
            character_id: p.character.as_ref().map(|c| c.id.clone()),
            avatar_url: p.character.as_ref().map(|c| c.avatar_url.clone()),
            expression: p.character.as_ref().map(|c| c.expression.clone()),
            dialogue_text: p.dialogue.as_ref().map(|d| d.text.clone()),
            dialogue_typing: p.dialogue.as_ref().map(|d| d.is_typing),
            audio_playing: p.dialogue.as_ref().map(|d| d.audio_playing),
            scale: p.scale,
            volume: p.volume,
            visible: p.visible,
        }
    }
}

#[tauri::command]
pub async fn check_overlay_permission<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
) -> std::result::Result<OverlayPermissionStatus, String> {
    #[cfg(target_os = "android")]
    {
        let _ = bridge; // 仅签名占位
        Ok(OverlayPermissionStatus::Unknown)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        Ok(OverlayPermissionStatus::Unsupported)
    }
}

#[tauri::command]
pub async fn request_overlay_permission<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
) -> std::result::Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        bridge
            .invoke_android("requestOverlayPermission", serde_json::json!({}))
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn show_floating_pet<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
    scale: Option<f64>,
) -> std::result::Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        let payload = serde_json::json!({ "scale": scale });
        bridge
            .invoke_android("showFloatingPet", payload)
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn hide_floating_pet<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
) -> std::result::Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        bridge
            .invoke_android("hideFloatingPet", serde_json::json!({}))
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn stop_floating_pet_service<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
) -> std::result::Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        bridge
            .invoke_android("stopFloatingPetService", serde_json::json!({}))
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn stop_floating_pet_service_with_confirmation<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
) -> std::result::Result<bool, String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        #[derive(Deserialize)]
        struct StopConfirmationResponse {
            stopped: bool,
        }

        let response: StopConfirmationResponse = bridge
            .invoke_android("stopFloatingPetServiceWithConfirmation", serde_json::json!({}))
            .map_err(|e| e.to_string())?;
        Ok(response.stopped)
    }
}

#[tauri::command]
pub async fn update_pet_state<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
    payload: PetStatePayload,
) -> std::result::Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = (bridge, payload);
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        let payload_json = serde_json::to_value(&payload).map_err(|e| e.to_string())?;
        bridge
            .invoke_android("updatePetState", payload_json)
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn start_permission_explanation<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
) -> std::result::Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        bridge
            .invoke_android("startPermissionExplanation", serde_json::json!({}))
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn mark_permission_explanation_shown<R: Runtime>(
    _app: AppHandle<R>,
    bridge: State<'_, PetBridge<R>>,
) -> std::result::Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let _ = bridge;
        return Err(PetError::UnsupportedPlatform.to_string());
    }
    #[cfg(target_os = "android")]
    {
        bridge
            .invoke_android(
                "markPermissionExplanationShown",
                serde_json::json!({}),
            )
            .map_err(|e| e.to_string())
    }
}
