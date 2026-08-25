//! OpenAI Codex（ChatGPT 订阅）OAuth 设备码登录、令牌刷新与额度查询。
//!
//! 协议常量与流程来自 dsh-codex / pi-ai：
//! - 设备码：`POST auth.openai.com/api/accounts/deviceauth/usercode`
//! - 轮询：`POST auth.openai.com/api/accounts/deviceauth/token`
//!   → `{authorization_code, code_verifier}`（200）/ pending（403/404）/ slow_down
//! - 交换/刷新：`POST auth.openai.com/oauth/token`（x-www-form-urlencoded）
//! - 额度：`GET chatgpt.com/backend-api/wham/usage`
//!
//! 凭据落盘 `data/codex-auth.json`；access_token 过期前 60 秒自动用
//! refresh_token 换新。accountId 从 access_token 的 JWT payload 提取。

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTH_BASE: &str = "https://auth.openai.com";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";

// ============================================================
// 凭据存储
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCredential {
    pub access: String,
    pub refresh: String,
    /// 过期时刻（Unix 毫秒时间戳）
    pub expires: i64,
    pub account_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodexAuthFile {
    version: u32,
    credential: Option<CodexCredential>,
}

fn auth_file_path() -> PathBuf {
    crate::api::data_dir().join("codex-auth.json")
}

pub fn load_credential() -> Option<CodexCredential> {
    let path = auth_file_path();
    let text = std::fs::read_to_string(path).ok()?;
    let file: CodexAuthFile = serde_json::from_str(&text).ok()?;
    file.credential
}

fn save_credential(cred: &CodexCredential) -> Result<()> {
    let path = auth_file_path();
    let file = CodexAuthFile {
        version: 1,
        credential: Some(cred.clone()),
    };
    let text = serde_json::to_string_pretty(&file)?;
    std::fs::write(&path, text).with_context(|| format!("写入 {:?} 失败", path))?;
    Ok(())
}

/// 退出登录：删除凭据文件。
pub fn logout() -> Result<()> {
    let path = auth_file_path();
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("删除 {:?} 失败", path))?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 从 access_token 的 JWT payload 提取 `chatgpt_account_id`（不验签，仅解析）。
fn extract_account_id(access: &str) -> Result<String> {
    let payload = access.split('.').nth(1).context("access_token 不是有效 JWT")?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .context("JWT payload base64 解码失败")?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).context("JWT payload 非 JSON")?;
    json.get("https://api.openai.com/auth")
        .and_then(|v| v.get("chatgpt_account_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("JWT 中缺少 chatgpt_account_id")
}

// ============================================================
// 设备码登录
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct DeviceLoginStart {
    pub device_auth_id: String,
    pub user_code: String,
    pub interval: u64,
    pub verification_url: String,
}

#[derive(Debug, Deserialize)]
struct UserCodeResponse {
    device_auth_id: String,
    user_code: String,
    /// pi-ai 注释：interval 可能是数字或数字字符串
    interval: serde_json::Value,
}

/// 第一步：请求设备码，返回用户码与验证地址。
pub async fn start_device_login(http: &Client) -> Result<DeviceLoginStart> {
    let resp = http
        .post(format!("{AUTH_BASE}/api/accounts/deviceauth/usercode"))
        .json(&serde_json::json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        .context("请求设备码失败（网络不可达？）")?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("请求设备码失败 ({status}): {text}");
    }
    let body: UserCodeResponse = resp.json().await.context("设备码响应解析失败")?;
    let interval = body
        .interval
        .as_u64()
        .or_else(|| body.interval.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(5);
    Ok(DeviceLoginStart {
        device_auth_id: body.device_auth_id,
        user_code: body.user_code,
        interval,
        verification_url: "https://auth.openai.com/codex/device".to_string(),
    })
}

#[derive(Debug)]
pub enum PollOutcome {
    /// 用户尚未完成授权，继续等
    Pending,
    /// 服务端要求放慢轮询（间隔 +5 秒）
    SlowDown,
    /// 授权完成，凭据已落盘
    Complete(CodexCredential),
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    authorization_code: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

async fn exchange_code(http: &Client, code: &str, verifier: &str) -> Result<TokenResponse> {
    let resp = http
        .post(format!("{AUTH_BASE}/oauth/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", DEVICE_REDIRECT_URI),
        ])
        .send()
        .await
        .context("交换授权码失败")?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("交换授权码失败 ({status}): {text}");
    }
    resp.json().await.context("令牌响应解析失败")
}

fn token_to_credential(token: TokenResponse) -> Result<CodexCredential> {
    let account_id = extract_account_id(&token.access_token)?;
    Ok(CodexCredential {
        access: token.access_token,
        refresh: token.refresh_token,
        expires: now_ms() + token.expires_in * 1000,
        account_id,
    })
}

/// 第二步（循环调用）：轮询一次设备授权状态。
pub async fn poll_device_login(
    http: &Client,
    device_auth_id: &str,
    user_code: &str,
) -> Result<PollOutcome> {
    let resp = http
        .post(format!("{AUTH_BASE}/api/accounts/deviceauth/token"))
        .json(&serde_json::json!({
            "device_auth_id": device_auth_id,
            "user_code": user_code,
        }))
        .send()
        .await
        .context("轮询设备授权失败")?;
    let status = resp.status();
    if status.is_success() {
        let device_token: DeviceTokenResponse = resp.json().await.context("设备令牌响应解析失败")?;
        let token = exchange_code(http, &device_token.authorization_code, &device_token.code_verifier).await?;
        let cred = token_to_credential(token)?;
        save_credential(&cred)?;
        return Ok(PollOutcome::Complete(cred));
    }
    if status.as_u16() == 403 || status.as_u16() == 404 {
        return Ok(PollOutcome::Pending);
    }
    let text = resp.text().await.unwrap_or_default();
    if text.contains("deviceauth_authorization_pending") {
        return Ok(PollOutcome::Pending);
    }
    if text.contains("slow_down") {
        return Ok(PollOutcome::SlowDown);
    }
    bail!("轮询设备授权失败 ({status}): {text}")
}

// ============================================================
// 令牌刷新
// ============================================================

/// 读取有效凭据：未过期直接返回；过期前 60 秒内用 refresh_token 自动换新并落盘。
/// 未登录返回 Ok(None)；刷新失败（令牌被撤销等）返回 Err。
pub async fn get_valid_credential(http: &Client) -> Result<Option<CodexCredential>> {
    let Some(cred) = load_credential() else {
        return Ok(None);
    };
    if now_ms() < cred.expires - 60_000 {
        return Ok(Some(cred));
    }
    tracing::info!("[Codex] access_token 即将过期，自动刷新");
    let resp = http
        .post(format!("{AUTH_BASE}/oauth/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", cred.refresh.as_str()),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .context("刷新令牌请求失败")?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        // 刷新令牌被撤销/失效：清掉本地凭据，要求重新登录
        if status.as_u16() == 400 || status.as_u16() == 401 {
            let _ = logout();
            bail!("Codex 登录已失效，请重新登录 ({status})");
        }
        bail!("刷新 Codex 令牌失败 ({status}): {text}");
    }
    let token: TokenResponse = resp.json().await.context("刷新令牌响应解析失败")?;
    let new_cred = token_to_credential(token)?;
    save_credential(&new_cred)?;
    Ok(Some(new_cred))
}

// ============================================================
// 额度查询（wham/usage）
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct QuotaWindow {
    /// 剩余百分比（100 - used_percent）
    pub remaining_percent: f64,
    /// 窗口长度（秒）：18000=5 小时窗，604800=7 天周窗
    pub window_seconds: u64,
    /// 重置时刻（Unix 秒），可空
    pub reset_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RateLimitQuota {
    pub primary: Option<QuotaWindow>,
    pub secondary: Option<QuotaWindow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdditionalQuota {
    /// 额度桶显示名（limit_name 或 metered_feature）
    pub name: String,
    pub quota: RateLimitQuota,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexUsage {
    pub rate_limit: RateLimitQuota,
    pub additional: Vec<AdditionalQuota>,
}

#[derive(Debug, Deserialize)]
struct RawWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<u64>,
    reset_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawRateLimit {
    primary_window: Option<RawWindow>,
    secondary_window: Option<RawWindow>,
}

#[derive(Debug, Deserialize)]
struct RawAdditional {
    metered_feature: Option<String>,
    limit_name: Option<String>,
    rate_limit: Option<RawRateLimit>,
}

#[derive(Debug, Deserialize)]
struct RawUsage {
    rate_limit: Option<RawRateLimit>,
    additional_rate_limits: Option<Vec<RawAdditional>>,
}

fn project_window(raw: Option<RawWindow>) -> Option<QuotaWindow> {
    raw.map(|w| QuotaWindow {
        remaining_percent: (100.0 - w.used_percent.unwrap_or(0.0)).clamp(0.0, 100.0),
        window_seconds: w.limit_window_seconds.unwrap_or(0),
        reset_at: w.reset_at,
    })
}

fn project_rate_limit(raw: Option<RawRateLimit>) -> RateLimitQuota {
    match raw {
        Some(r) => RateLimitQuota {
            primary: project_window(r.primary_window),
            secondary: project_window(r.secondary_window),
        },
        None => RateLimitQuota {
            primary: None,
            secondary: None,
        },
    }
}

/// 查询当前账号额度（需要有效凭据）。
pub async fn get_usage(http: &Client, cred: &CodexCredential) -> Result<CodexUsage> {
    let resp = http
        .get(USAGE_URL)
        .bearer_auth(&cred.access)
        .header("chatgpt-account-id", &cred.account_id)
        .header("accept", "application/json")
        .header("cache-control", "no-store")
        .header("user-agent", "lingchat-codex")
        .send()
        .await
        .context("查询 Codex 额度失败")?;
    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        bail!("Codex 登录状态失效（{status}），请重新登录");
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("查询 Codex 额度失败 ({status}): {text}");
    }
    let raw: RawUsage = resp.json().await.context("额度响应解析失败")?;
    let additional = raw
        .additional_rate_limits
        .unwrap_or_default()
        .into_iter()
        .map(|a| AdditionalQuota {
            name: a
                .limit_name
                .or(a.metered_feature)
                .unwrap_or_else(|| "unknown".to_string()),
            quota: project_rate_limit(a.rate_limit),
        })
        .collect();
    Ok(CodexUsage {
        rate_limit: project_rate_limit(raw.rate_limit),
        additional,
    })
}
