//! OpenAI Codex（ChatGPT 订阅）相关 Tauri commands。
//!
//! 前端大模型管理页用这些命令完成设备码登录（展示用户码 → 轮询授权）、
//! 查看登录状态、退出登录、查询额度；具体协议见 `llm::codex_auth`。

use serde::Serialize;

use crate::ai_service::llm::codex_auth;
use crate::utils::proxy::build_proxied_client;

#[derive(Debug, Serialize)]
pub struct CodexAuthStatus {
    pub logged_in: bool,
    pub account_id: Option<String>,
    /// access_token 过期时刻（Unix 毫秒）
    pub expires: Option<i64>,
}

#[tauri::command]
pub async fn codex_auth_status() -> Result<CodexAuthStatus, String> {
    Ok(match codex_auth::load_credential() {
        Some(cred) => CodexAuthStatus {
            logged_in: true,
            account_id: Some(cred.account_id),
            expires: Some(cred.expires),
        },
        None => CodexAuthStatus {
            logged_in: false,
            account_id: None,
            expires: None,
        },
    })
}

#[tauri::command]
pub async fn codex_start_login() -> Result<codex_auth::DeviceLoginStart, String> {
    let http = build_proxied_client(15).await.map_err(|e| e.to_string())?;
    codex_auth::start_device_login(&http)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct CodexPollStatus {
    /// "pending"（等待授权）/ "slow_down"（放慢轮询）/ "complete"（登录完成）
    pub status: String,
    pub account_id: Option<String>,
}

#[tauri::command]
pub async fn codex_poll_login(
    device_auth_id: String,
    user_code: String,
) -> Result<CodexPollStatus, String> {
    let http = build_proxied_client(15).await.map_err(|e| e.to_string())?;
    let outcome = codex_auth::poll_device_login(&http, &device_auth_id, &user_code)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match outcome {
        codex_auth::PollOutcome::Pending => CodexPollStatus {
            status: "pending".to_string(),
            account_id: None,
        },
        codex_auth::PollOutcome::SlowDown => CodexPollStatus {
            status: "slow_down".to_string(),
            account_id: None,
        },
        codex_auth::PollOutcome::Complete(cred) => CodexPollStatus {
            status: "complete".to_string(),
            account_id: Some(cred.account_id),
        },
    })
}

#[tauri::command]
pub async fn codex_logout() -> Result<(), String> {
    codex_auth::logout().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn codex_get_quota() -> Result<codex_auth::CodexUsage, String> {
    let http = build_proxied_client(15).await.map_err(|e| e.to_string())?;
    let cred = codex_auth::get_valid_credential(&http)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "未登录 Codex，请先登录".to_string())?;
    codex_auth::get_usage(&http, &cred)
        .await
        .map_err(|e| e.to_string())
}
