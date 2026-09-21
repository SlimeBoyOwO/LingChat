//! WorkBuddy（腾讯 CodeBuddy）订阅账号：浏览器授权登录、令牌自动刷新、
//! 凭据存储与订阅额度查询。
//!
//! 协议（与官方 CodeBuddy CLI 一致，参考 astrbot_plugin_workbuddy_provider）：
//! - 登录：`POST {base}/v2/plugin/auth/state?platform=CLI` → `{state, authUrl}`
//! - 轮询：`GET {base}/v2/plugin/auth/token?state=...` → 信封 `{code,msg,data}`，
//!   `code != 0` 视为「等待授权」；完成后 data 携带 accessToken/refreshToken/expiresIn
//! - 账号：`GET {base}/v2/plugin/login/account?state=...`（Bearer）→ uid/nickname
//! - 刷新：`POST {base}/v2/plugin/auth/token/refresh`，头 `X-Refresh-Token` +
//!   `X-Auth-Refresh-Source: plugin`，响应 data 同样轮换 accessToken/refreshToken
//! - 额度：`POST {billing_base}/v2/billing/meter/get-user-resource`（空 JSON 体）
//!
//! 区域（realm）：
//! - `cn`：chat=`https://copilot.tencent.com`，origin=`https://www.codebuddy.cn`，
//!   billing=`https://www.codebuddy.cn`
//! - `global`：chat/billing/origin=`https://www.workbuddy.ai`
//!
//! 凭据落盘 `data/workbuddy-auth.json`（明文 JSON，与 codex-auth.json 同级同风格）。

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use base64::Engine;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const REALM_CN: &str = "cn";
pub const REALM_GLOBAL: &str = "global";

/// 客户端指纹（与官方 CLI 对齐，版本随上游更新）。
const CLIENT_VERSION: &str = "5.5.4";
const CLI_VERSION: &str = "2.137.1";
/// 登录阶段 UA 与运行期不同（官方客户端行为）。
const LOGIN_USER_AGENT: &str = "CLI/2.63.2 CodeBuddy/2.63.2";

/// 凭据文档版本（写入时带在 JSON 里，便于将来兼容升级）。
pub const AUTH_STORE_VERSION: u32 = 1;

// ============================================================
// 区域与端点
// ============================================================

pub fn normalize_realm(value: &str) -> String {
    let text = value.trim().to_lowercase();
    if text == REALM_GLOBAL || text.ends_with("workbuddy.ai") {
        REALM_GLOBAL.to_string()
    } else {
        REALM_CN.to_string()
    }
}

/// 返回 `(chat_base, web_origin, billing_base)`。
pub fn realm_endpoints(realm: &str) -> (&'static str, &'static str, &'static str) {
    if normalize_realm(realm) == REALM_GLOBAL {
        (
            "https://www.workbuddy.ai",
            "https://www.workbuddy.ai",
            "https://www.workbuddy.ai",
        )
    } else {
        (
            "https://copilot.tencent.com",
            "https://www.codebuddy.cn",
            "https://www.codebuddy.cn",
        )
    }
}

/// 运行期三段式 UA：`WorkBuddy/<v> <platform>/<v> CLI/<v>`。
/// global 平台段是 `WorkBuddy AI`（送错平台段可能触发风控 403）。
pub fn runtime_user_agent(realm: &str) -> String {
    let platform = if normalize_realm(realm) == REALM_GLOBAL {
        "WorkBuddy AI"
    } else {
        "WorkBuddy"
    };
    format!("WorkBuddy/{CLIENT_VERSION} {platform}/{CLIENT_VERSION} CLI/{CLI_VERSION}")
}

// ============================================================
// 凭据存储
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkBuddyCredential {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    /// 过期时刻（Unix 秒）；0 表示未知
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub enterprise_id: String,
    /// 原始 domain（如 www.codebuddy.cn），留空用 realm 默认
    #[serde(default)]
    pub domain: String,
    #[serde(default = "default_realm")]
    pub realm: String,
}

fn default_realm() -> String {
    REALM_CN.to_string()
}

impl WorkBuddyCredential {
    pub fn is_logged_in(&self) -> bool {
        !self.access_token.trim().is_empty()
    }

    /// 从 JWT payload 取 `exp`（Unix 秒），非 JWT 返回 None。
    pub fn token_expiry(&self) -> Option<i64> {
        jwt_exp(&self.access_token)
    }
}

fn auth_file_path() -> PathBuf {
    crate::api::data_dir().join("workbuddy-auth.json")
}

/// 凭据文档外壳（与 codex-auth.json 同为带版本号的 JSON）。
#[derive(Debug, Serialize, Deserialize)]
struct WorkBuddyAuthFile {
    version: u32,
    credential: Option<WorkBuddyCredential>,
}

pub fn load_credential() -> Option<WorkBuddyCredential> {
    let path = auth_file_path();
    let text = std::fs::read_to_string(path).ok()?;
    let file: WorkBuddyAuthFile = serde_json::from_str(&text).ok()?;
    file.credential
}

pub fn save_credential(cred: &WorkBuddyCredential) -> Result<()> {
    let path = auth_file_path();
    let file = WorkBuddyAuthFile {
        version: AUTH_STORE_VERSION,
        credential: Some(cred.clone()),
    };
    let text = serde_json::to_string_pretty(&file)?;
    std::fs::write(&path, text).with_context(|| format!("写入 {:?} 失败", path))?;
    Ok(())
}

pub fn logout() -> Result<()> {
    let path = auth_file_path();
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("删除 {:?} 失败", path))?;
    }
    Ok(())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 解码 JWT payload 并取 `exp`（Unix 秒）。不验签，仅解析。
pub fn jwt_exp(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let json: Value = serde_json::from_slice(&decoded).ok()?;
    json.get("exp").and_then(Value::as_i64)
}

/// 从 JWT payload 取 `sub`（即 uid）。
fn jwt_sub(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let json: Value = serde_json::from_slice(&decoded).ok()?;
    json.get("sub")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

// ============================================================
// 会话头（模型目录 / 对话共用）
// ============================================================

/// 构建上游请求的完整头集（含派生的设备/会话标识）。
pub fn account_headers(cred: &WorkBuddyCredential, token: &str) -> Result<HeaderMap> {
    let (base, origin, _) = realm_endpoints(&cred.realm);
    let is_global = normalize_realm(&cred.realm) == REALM_GLOBAL;
    let uid = if cred.uid.is_empty() {
        jwt_sub(token).unwrap_or_default()
    } else {
        cred.uid.clone()
    };
    let platform = if is_global {
        "WorkBuddy AI"
    } else {
        "WorkBuddy"
    };
    // 会话/消息级 trace ID：每轮随机（与官方客户端行为一致）
    let message_id = uuid::Uuid::new_v4().simple().to_string();
    let conversation_id = uuid::Uuid::new_v4().simple().to_string();

    let mut headers = HeaderMap::new();
    // 头名大小写不敏感：HeaderName::try_from 会校验并归一为小写
    let mut insert = |name: &str, value: String| {
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::try_from(name),
            HeaderValue::from_str(&value),
        ) {
            headers.insert(name, value);
        }
    };
    insert("User-Agent", runtime_user_agent(&cred.realm));
    insert("Origin", origin.to_string());
    insert("Referer", format!("{origin}/"));
    insert("X-Requested-With", "XMLHttpRequest".to_string());
    // 风控闸门头：所有上游 API 必带
    insert("X-CodeBuddy-Request", "1".to_string());
    insert(
        "Accept-Language",
        if is_global { "en-US" } else { "zh-CN" }.to_string(),
    );
    insert("X-Agent-Purpose", "conversation".to_string());
    insert("X-IDE-Name", platform.to_string());
    insert("X-IDE-Type", platform.to_string());
    insert("X-IDE-Version", CLIENT_VERSION.to_string());
    insert("X-Product", platform.to_string());
    // 会话/消息/追踪 ID 族
    insert("X-Conversation-Request-ID", conversation_id.clone());
    insert("X-Conversation-Message-ID", message_id.clone());
    insert("X-Request-ID", message_id.clone());
    insert("X-Root-Request-ID", conversation_id.clone());
    insert("X-Trace-ID", conversation_id.clone());
    insert("X-B3-TraceId", conversation_id.clone());
    insert("X-B3-SpanId", message_id.chars().take(16).collect());
    insert("X-B3-Sampled", "1".to_string());
    insert("Authorization", format!("Bearer {token}"));

    if !uid.is_empty() {
        // 设备/会话指纹：sha256("wb2a:<purpose>:<uid>") 前 36 hex
        insert(
            "X-Machine-ID",
            sha256_hex(&format!("wb2a:machine:{uid}"))[..36].to_string(),
        );
        insert(
            "X-Session-ID",
            sha256_hex(&format!("wb2a:session:{uid}"))[..36].to_string(),
        );
        insert("X-User-Id", uid.clone());
        insert("X-No-Enterprise-Id", "1".to_string());
        let domain = if is_global {
            "www.workbuddy.ai".to_string()
        } else if !cred.domain.is_empty() {
            cred.domain.clone()
        } else {
            "www.codebuddy.cn".to_string()
        };
        insert("X-Domain", domain);
    }
    let _ = base; // base 仅用于文档可读性；实际 URL 由调用方拼接
    Ok(headers)
}

pub fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let out = hasher.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn login_headers(origin: &str) -> HeaderMap {
    let referer = format!("{origin}/");
    header_map(&[
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/plain, */*"),
        ("X-Requested-With", "XMLHttpRequest"),
        ("Origin", origin),
        ("Referer", &referer),
        ("User-Agent", LOGIN_USER_AGENT),
    ])
}

/// 把 (名称, 值) 列表转成 reqwest `HeaderMap`；非法名称/值直接跳过。
fn header_map(entries: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in entries {
        let (Ok(name), Ok(value)) = (HeaderName::try_from(*name), HeaderValue::from_str(value))
        else {
            continue;
        };
        map.insert(name, value);
    }
    map
}

// ============================================================
// 登录（state + 浏览器授权 + 轮询）
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct DeviceLoginStart {
    /// 登录会话标识，轮询时原样带回
    pub state: String,
    pub auth_url: String,
    /// "cn" | "global"
    pub realm: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceLoginComplete {
    pub uid: String,
    pub nickname: String,
    pub realm: String,
}

/// 解析 `{code,msg,data}` 业务信封；`code != 0` → 等待授权。
fn unwrap_envelope(status: reqwest::StatusCode, body: &str) -> std::result::Result<Value, ()> {
    if status.as_u16() >= 400 {
        return Err(());
    }
    let envelope: Value = serde_json::from_str(body).map_err(|_| ())?;
    let code = envelope.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 0 {
        return Err(());
    }
    Ok(envelope.get("data").cloned().unwrap_or(Value::Null))
}

/// 第一步：申请登录 state 与浏览器授权链接。
pub async fn start_device_login(http: &Client, realm: &str) -> Result<DeviceLoginStart> {
    let realm = normalize_realm(realm);
    let (base, origin, _) = realm_endpoints(&realm);
    let resp = http
        .post(format!("{base}/v2/plugin/auth/state?platform=CLI"))
        .headers(login_headers(origin))
        .body("{}")
        .send()
        .await
        .context("请求 WorkBuddy 登录 state 失败")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let data = unwrap_envelope(status, &text)
        .map_err(|_| anyhow::anyhow!("WorkBuddy 登录服务返回异常 ({status}): {}", short(&text)))?;
    let state = data
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let auth_url = data
        .get("authUrl")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if state.is_empty() || auth_url.is_empty() {
        bail!("WorkBuddy 登录响应缺少 state 或 authUrl");
    }
    Ok(DeviceLoginStart {
        state,
        auth_url,
        realm,
    })
}

fn short(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.len() > 200 {
        &trimmed[..200]
    } else {
        trimmed
    }
}

/// 第二步（循环调用）：轮询一次授权状态。
/// 返回 Ok(None) 表示用户尚未完成浏览器登录。
pub async fn poll_device_login(
    http: &Client,
    state: &str,
    realm: &str,
) -> Result<Option<DeviceLoginComplete>> {
    let realm = normalize_realm(realm);
    let (base, origin, _) = realm_endpoints(&realm);
    let resp = http
        .get(format!("{base}/v2/plugin/auth/token?state={state}"))
        .headers(login_headers(origin))
        .send()
        .await
        .context("轮询 WorkBuddy 登录状态失败")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let data = match unwrap_envelope(status, &text) {
        Ok(data) => data,
        Err(()) => return Ok(None), // 等待授权 / 暂时性失败
    };
    let access = data
        .get("accessToken")
        .or_else(|| data.get("access_token"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if access.is_empty() {
        return Ok(None);
    }
    let mut cred = WorkBuddyCredential {
        access_token: access,
        refresh_token: data
            .get("refreshToken")
            .or_else(|| data.get("refresh_token"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        expires_at: 0,
        uid: String::new(),
        nickname: String::new(),
        enterprise_id: String::new(),
        domain: data
            .get("domain")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        realm: realm.clone(),
    };
    // expiresIn（秒）→ 绝对过期时刻；无则退回 JWT exp
    if let Some(expires_in) = data.get("expiresIn").and_then(Value::as_f64) {
        if expires_in > 0.0 && expires_in < 10.0 * 365.0 * 86400.0 {
            cred.expires_at = (now_secs() as f64 + expires_in) as u64;
        }
    }
    if cred.expires_at == 0 {
        cred.expires_at = cred.token_expiry().unwrap_or(0).max(0) as u64;
    }
    if cred.uid.is_empty() {
        cred.uid = jwt_sub(&cred.access_token).unwrap_or_default();
    }

    // 账号资料（uid/nickname）：失败不影响登录本身
    if let Ok(account) = fetch_login_account(http, state, &realm, &cred.access_token).await {
        if let Some(uid) = account.get("uid").and_then(Value::as_str) {
            cred.uid = uid.to_string();
        }
        if let Some(nick) = account.get("nickname").and_then(Value::as_str) {
            cred.nickname = nick.to_string();
        }
        if let Some(ent) = account.get("enterpriseId").and_then(Value::as_str) {
            cred.enterprise_id = ent.to_string();
        }
        if cred.domain.is_empty() {
            if let Some(domain) = account.get("domain").and_then(Value::as_str) {
                cred.domain = domain.to_string();
            }
        }
    }
    save_credential(&cred)?;
    Ok(Some(DeviceLoginComplete {
        uid: cred.uid.clone(),
        nickname: cred.nickname.clone(),
        realm: cred.realm.clone(),
    }))
}

async fn fetch_login_account(
    http: &Client,
    state: &str,
    realm: &str,
    access_token: &str,
) -> Result<Value> {
    let (base, origin, _) = realm_endpoints(realm);
    let mut headers = login_headers(origin);
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {access_token}")) {
        headers.insert(HeaderName::from_static("authorization"), value);
    }
    let resp = http
        .get(format!("{base}/v2/plugin/login/account?state={state}"))
        .headers(headers)
        .send()
        .await
        .context("获取 WorkBuddy 账号信息失败")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let data = unwrap_envelope(status, &text)
        .map_err(|_| anyhow::anyhow!("WorkBuddy 账号信息响应异常 ({status})"))?;
    Ok(data)
}

// ============================================================
// 令牌刷新
// ============================================================

/// 读取有效凭据：过期前 10 分钟内自动用 refresh_token 换新并落盘。
/// 未登录返回 Ok(None)；刷新失败（令牌被撤销等）返回 Err。
pub async fn get_valid_credential(http: &Client) -> Result<Option<WorkBuddyCredential>> {
    let Some(mut cred) = load_credential() else {
        return Ok(None);
    };
    // 过期判定：优先存档的 expires_at，其次 JWT exp
    let expires = if cred.expires_at > 0 {
        cred.expires_at as i64
    } else {
        cred.token_expiry().unwrap_or(0)
    };
    if expires > 0 && now_secs() < expires - 600 {
        return Ok(Some(cred));
    }
    if cred.refresh_token.trim().is_empty() {
        // 无 refresh_token：令牌可能长期有效（手工粘贴），过期前原样返回
        if expires == 0 || now_secs() < expires {
            return Ok(Some(cred));
        }
        bail!("WorkBuddy 登录已过期且无刷新令牌，请重新登录");
    }
    tracing::info!("[WorkBuddy] access_token 即将过期，自动刷新");
    let refreshed = refresh_access_token(http, &cred.refresh_token, &cred.realm).await?;
    cred.access_token = refreshed.access_token;
    if !refreshed.refresh_token.is_empty() {
        cred.refresh_token = refreshed.refresh_token;
    }
    if refreshed.expires_at > 0 {
        cred.expires_at = refreshed.expires_at;
    }
    save_credential(&cred)?;
    Ok(Some(cred))
}

struct RefreshedTokens {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

async fn refresh_access_token(
    http: &Client,
    refresh_token: &str,
    realm: &str,
) -> Result<RefreshedTokens> {
    if refresh_token.trim().is_empty() {
        bail!("WorkBuddy 刷新令牌为空");
    }
    let (base, _, _) = realm_endpoints(realm);
    let resp = http
        .post(format!("{base}/v2/plugin/auth/token/refresh"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-Refresh-Token", refresh_token)
        .header("X-Auth-Refresh-Source", "plugin")
        .header("User-Agent", LOGIN_USER_AGENT)
        .send()
        .await
        .context("刷新 WorkBuddy 令牌请求失败")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.as_u16() >= 400 {
        // 刷新令牌被撤销/失效：清掉本地凭据，要求重新登录
        if status.as_u16() == 400 || status.as_u16() == 401 {
            let _ = logout();
            bail!("WorkBuddy 登录已失效，请重新登录 ({status})");
        }
        bail!("刷新 WorkBuddy 令牌失败 ({status}): {}", short(&text));
    }
    let envelope: Value = serde_json::from_str(&text).context("刷新 WorkBuddy 令牌响应解析失败")?;
    let data = envelope.get("data").cloned().unwrap_or(Value::Null);
    let access = data
        .get("accessToken")
        .or_else(|| data.get("access_token"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if access.is_empty() {
        bail!("WorkBuddy 令牌刷新失败，请重新登录");
    }
    let refresh = data
        .get("refreshToken")
        .or_else(|| data.get("refresh_token"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut expires_at = 0u64;
    if let Some(expires_in) = data.get("expiresIn").and_then(Value::as_f64) {
        if expires_in > 0.0 && expires_in < 10.0 * 365.0 * 86400.0 {
            expires_at = (now_secs() as f64 + expires_in) as u64;
        }
    }
    if expires_at == 0 {
        expires_at = jwt_exp(&access).unwrap_or(0).max(0) as u64;
    }
    Ok(RefreshedTokens {
        access_token: access,
        refresh_token: refresh,
        expires_at,
    })
}

// ============================================================
// 订阅额度（/v2/billing/meter/get-user-resource）
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct CreditPackage {
    pub name: String,
    pub remain: f64,
    pub size: f64,
    /// 到期时间原文（可能为空）
    pub expired: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkBuddyUsage {
    pub nickname: String,
    pub uid: String,
    pub realm: String,
    /// 累计剩余 credits
    pub remaining: f64,
    /// 累计总额度 credits
    pub total: f64,
    pub packages: Vec<CreditPackage>,
}

/// 查询当前账号订阅额度（需要有效凭据）。
pub async fn get_usage(http: &Client, cred: &WorkBuddyCredential) -> Result<WorkBuddyUsage> {
    let (_, _, billing_base) = realm_endpoints(&cred.realm);
    let is_global = normalize_realm(&cred.realm) == REALM_GLOBAL;
    let token = &cred.access_token;
    let mut headers = account_headers(cred, token)?;
    // Billing 端点期望单段 UA（不带 CLI 段），且不需要会话头族
    headers.remove("User-Agent");
    headers.insert(
        HeaderName::from_static("user-agent"),
        HeaderValue::from_str(&format!("WorkBuddy/{CLIENT_VERSION}"))
            .context("WorkBuddy UA 非法")?,
    );
    headers.insert(
        HeaderName::from_static("accept"),
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/json"),
    );
    if is_global {
        headers.insert(
            HeaderName::from_static("x-tenant-id"),
            HeaderValue::from_str(&cred.enterprise_id).context("WorkBuddy 企业 ID 含非法字符")?,
        );
    }

    let resp = http
        .post(format!("{billing_base}/v2/billing/meter/get-user-resource"))
        .headers(headers)
        .json(&serde_json::json!({}))
        .send()
        .await
        .context("查询 WorkBuddy 订阅额度失败")?;
    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        bail!("WorkBuddy 登录状态失效（{status}），请重新登录");
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("查询 WorkBuddy 订阅额度失败 ({status}): {}", short(&text));
    }
    let payload: Value = resp.json().await.context("额度响应解析失败")?;
    let packages = extract_credit_packages(&payload);
    let remaining = packages.iter().map(|p| p.remain).sum();
    let total = packages.iter().map(|p| p.size).sum();
    Ok(WorkBuddyUsage {
        nickname: cred.nickname.clone(),
        uid: cred.uid.clone(),
        realm: cred.realm.clone(),
        remaining,
        total,
        packages,
    })
}

/// 把腾讯计费的多层包装（data/Response/Data/Accounts）展平成额度包列表。
fn extract_credit_packages(payload: &Value) -> Vec<CreditPackage> {
    let mut data = payload
        .get("data")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    if data.get("Response").is_some() {
        data = data["Response"].clone();
    }
    if data.get("Data").is_some() {
        data = data["Data"].clone();
    }
    let Some(accounts) = data.get("Accounts").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut packages = Vec::new();
    for entry in accounts {
        let size = as_float(
            &entry
                .get("CapacitySizePrecise")
                .or_else(|| entry.get("CapacitySize"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        let remain = as_float(
            &entry
                .get("CycleCapacityRemainPrecise")
                .or_else(|| entry.get("CapacityRemain"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        let (Some(size), Some(remain)) = (size, remain) else {
            continue;
        };
        if size <= 0.0 {
            continue;
        }
        packages.push(CreditPackage {
            name: entry
                .get("PackageName")
                .or_else(|| entry.get("SubProductName"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            remain,
            size,
            expired: entry
                .get("ExpiredTime")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        });
    }
    packages
}

fn as_float(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_normalization_and_endpoints() {
        assert_eq!(normalize_realm("global"), REALM_GLOBAL);
        assert_eq!(normalize_realm("GLOBAL"), REALM_GLOBAL);
        assert_eq!(normalize_realm("www.workbuddy.ai"), REALM_GLOBAL);
        assert_eq!(normalize_realm("cn"), REALM_CN);
        assert_eq!(normalize_realm(""), REALM_CN);
        assert_eq!(normalize_realm("copilot.tencent.com"), REALM_CN);

        let (base, origin, billing) = realm_endpoints(REALM_CN);
        assert_eq!(base, "https://copilot.tencent.com");
        assert_eq!(origin, "https://www.codebuddy.cn");
        assert_eq!(billing, "https://www.codebuddy.cn");
        let (base, origin, billing) = realm_endpoints(REALM_GLOBAL);
        assert_eq!(base, "https://www.workbuddy.ai");
        assert_eq!(origin, "https://www.workbuddy.ai");
        assert_eq!(billing, "https://www.workbuddy.ai");
    }

    #[test]
    fn runtime_user_agent_platform_segment_follows_realm() {
        assert_eq!(
            runtime_user_agent(REALM_CN),
            "WorkBuddy/5.5.4 WorkBuddy/5.5.4 CLI/2.137.1"
        );
        assert_eq!(
            runtime_user_agent(REALM_GLOBAL),
            "WorkBuddy/5.5.4 WorkBuddy AI/5.5.4 CLI/2.137.1"
        );
    }

    #[test]
    fn account_headers_contain_device_identity() {
        let cred = WorkBuddyCredential {
            access_token: String::new(),
            refresh_token: String::new(),
            expires_at: 0,
            uid: "test-uid".to_string(),
            nickname: String::new(),
            enterprise_id: String::new(),
            domain: String::new(),
            realm: REALM_CN.to_string(),
        };
        let headers = account_headers(&cred, "token").expect("headers");
        let get = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        };
        assert_eq!(get("x-codebuddy-request").as_deref(), Some("1"));
        assert_eq!(get("x-user-id").as_deref(), Some("test-uid"));
        assert_eq!(
            get("x-machine-id").as_deref(),
            Some(&sha256_hex("wb2a:machine:test-uid")[..36])
        );
        assert_eq!(get("x-domain").as_deref(), Some("www.codebuddy.cn"));
        assert_eq!(get("authorization").as_deref(), Some("Bearer token"));
        // 不允许携带 refresh token（安全红线）
        assert!(get("x-refresh-token").is_none());
    }

    #[test]
    fn credit_package_extraction_flattens_billing_wrapper() {
        let payload: Value = serde_json::json!({
            "code": 0,
            "data": {
                "Response": {
                    "Data": {
                        "Accounts": [
                            {
                                "PackageName": "月度包",
                                "CapacitySize": "100",
                                "CapacityRemain": "42.5",
                                "ExpiredTime": "2026-12-31"
                            },
                            {
                                "PackageName": "耗尽包",
                                "CapacitySize": 10,
                                "CapacityRemain": 0
                            }
                        ]
                    }
                }
            }
        });
        let packages = extract_credit_packages(&payload);
        // 与插件一致：仅跳过 size<=0 的条目，耗尽（remain=0）的额度包保留
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "月度包");
        assert_eq!(packages[0].size, 100.0);
        assert_eq!(packages[0].remain, 42.5);
        assert_eq!(packages[1].remain, 0.0);
    }
}
