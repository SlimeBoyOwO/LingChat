//! WorkBuddy（腾讯 CodeBuddy 订阅）相关 Tauri commands。
//!
//! 前端大模型管理页用这些命令完成浏览器授权登录（打开链接 → 轮询）、
//! 查看登录状态、退出登录、查询订阅额度；具体协议见 `llm::workbuddy::auth`。

use std::time::Duration;

use serde::Serialize;

use crate::ai_service::llm::workbuddy::auth;
use crate::utils::proxy::build_proxied_client;

/// 按区域构建 HTTP 客户端：国内版直连（上游按区域锁定，走代理会失败），
/// 国际版尝试本地代理。TLS 统一用 webpki-roots 配置（Android 上必须）。
async fn realm_client(realm: &str, timeout_secs: u64) -> Result<reqwest::Client, String> {
    if auth::normalize_realm(realm) == auth::REALM_GLOBAL {
        return build_proxied_client(timeout_secs)
            .await
            .map_err(|e| e.to_string());
    }
    let tls = crate::utils::tls::build_tls_config()?;
    reqwest::Client::builder()
        .read_timeout(Duration::from_secs(timeout_secs))
        .tls_backend_preconfigured(tls)
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct WorkBuddyAuthStatus {
    pub logged_in: bool,
    pub uid: Option<String>,
    pub nickname: Option<String>,
    /// "cn" | "global"
    pub realm: Option<String>,
}

#[tauri::command]
pub async fn workbuddy_auth_status() -> Result<WorkBuddyAuthStatus, String> {
    Ok(match auth::load_credential() {
        Some(cred) if cred.is_logged_in() => WorkBuddyAuthStatus {
            logged_in: true,
            uid: Some(cred.uid),
            nickname: Some(cred.nickname),
            realm: Some(cred.realm),
        },
        _ => WorkBuddyAuthStatus {
            logged_in: false,
            uid: None,
            nickname: None,
            realm: None,
        },
    })
}

#[derive(Debug, Serialize)]
pub struct WorkBuddyLoginStart {
    pub state: String,
    pub auth_url: String,
    pub realm: String,
}

#[tauri::command]
pub async fn workbuddy_start_login(realm: Option<String>) -> Result<WorkBuddyLoginStart, String> {
    let realm = auth::normalize_realm(realm.as_deref().unwrap_or(auth::REALM_CN));
    let http = realm_client(&realm, 15).await?;
    let start = auth::start_device_login(&http, &realm)
        .await
        .map_err(|e| e.to_string())?;
    Ok(WorkBuddyLoginStart {
        state: start.state,
        auth_url: start.auth_url,
        realm: start.realm,
    })
}

#[derive(Debug, Serialize)]
pub struct WorkBuddyPollStatus {
    /// "pending"（等待授权）/ "complete"（登录完成）
    pub status: String,
    pub uid: Option<String>,
    pub nickname: Option<String>,
}

#[tauri::command]
pub async fn workbuddy_poll_login(
    state: String,
    realm: String,
) -> Result<WorkBuddyPollStatus, String> {
    let http = realm_client(&realm, 15).await?;
    let outcome = auth::poll_device_login(&http, &state, &realm)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match outcome {
        Some(complete) => WorkBuddyPollStatus {
            status: "complete".to_string(),
            uid: Some(complete.uid),
            nickname: Some(complete.nickname),
        },
        None => WorkBuddyPollStatus {
            status: "pending".to_string(),
            uid: None,
            nickname: None,
        },
    })
}

#[tauri::command]
pub async fn workbuddy_logout() -> Result<(), String> {
    auth::logout().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn workbuddy_get_quota() -> Result<auth::WorkBuddyUsage, String> {
    let realm = auth::load_credential()
        .map(|c| c.realm)
        .unwrap_or_else(|| auth::REALM_CN.to_string());
    let http = realm_client(&realm, 15).await?;
    let cred = auth::get_valid_credential(&http)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "未登录 WorkBuddy，请先登录".to_string())?;
    auth::get_usage(&http, &cred)
        .await
        .map_err(|e| e.to_string())
}
