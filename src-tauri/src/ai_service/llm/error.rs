//! LLM 错误统一分类与友好提示。
//!
//! 项目的 LLM 调用（genai / 手写 HTTP provider）大多把底层错误压扁成字符串，
//! 这里基于错误链文本做兜底分类，返回稳定的 `error_code` 与面向用户的直白原因说明
//! （含「可能原因」），供主对话流（`ai:error`）与设置页（测试/拉模型）共同使用。
//!
//! 设计取舍：不强依赖 `genai` 内部错误类型（跨版本易碎），用字符串匹配覆盖主流的
//! 401/403/404/429/5xx/超时/网络/空响应 场景；后续如需更精细，可升级为结构化
//! `LlmError` 枚举（保留原始错误链，供 `downcast`）。

/// 分类结果：稳定错误码 + 原始错误文本（供前端 i18n 映射与调试）。
pub struct LlmErrorInfo {
    pub code: &'static str,
    pub raw: String,
}

/// 序列化给前端的 LLM 错误载荷（设置页测试/拉模型命令使用）。
#[derive(serde::Serialize)]
pub struct LlmErrorPayload {
    pub code: String,
    pub detail: String,
}

impl From<LlmErrorInfo> for LlmErrorPayload {
    fn from(info: LlmErrorInfo) -> Self {
        Self {
            code: info.code.to_string(),
            detail: info.raw,
        }
    }
}

impl LlmErrorPayload {
    /// 直接构造错误载荷（用于未走分类器的配置校验类错误）。
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            detail: detail.into(),
        }
    }
}

/// 对 LLM 错误分类，返回稳定错误码（前端 i18n 查文案）与原始错误文本。
///
/// 面向用户的文案（含俏皮/直白风格）统一放在前端 i18n：`stores.llmErrors.*`，
/// Rust 只负责给出稳定的 `error_code`。
pub fn classify_llm_error(err: &anyhow::Error) -> LlmErrorInfo {
    // 用 {:#} 展开整条错误链，避免只看到最外层包装而漏掉底层状态码
    let raw = format!("{err:#}");
    let lc = raw.to_lowercase();

    // 优先级：401 → 403 → 404 → 429 → 5xx → 超时 → 网络 → 空响应 → 其它
    let code = if is_invalid_api_key(&raw, &lc) {
        "invalid_api_key"
    } else if is_403(&raw, &lc) {
        "forbidden"
    } else if is_404(&raw, &lc) {
        "not_found"
    } else if is_429(&raw, &lc) {
        "rate_limited"
    } else if is_5xx(&raw, &lc) {
        "server_error"
    } else if is_timeout(&lc) {
        "timeout"
    } else if is_network(&lc) {
        "network_error"
    } else if is_empty_response(&lc) {
        "empty_response"
    } else {
        "other"
    };

    LlmErrorInfo { code, raw }
}

/// API Key 无效 / 未授权（401 及常见授权文案）
fn is_invalid_api_key(raw: &str, lc: &str) -> bool {
    raw.contains("401")
        || lc.contains("unauthorized")
        || lc.contains("invalid api key")
        || lc.contains("invalid_api_key")
        || lc.contains("api key is invalid")
        || lc.contains("authentication failed")
        || lc.contains("authentication error")
        || lc.contains("invalid authentication")
        || lc.contains("invalid key")
}

/// 权限不足（403）
fn is_403(raw: &str, lc: &str) -> bool {
    raw.contains("403") || lc.contains("forbidden") || lc.contains("permission denied")
}

/// 资源/接口/模型不存在（404）
fn is_404(raw: &str, lc: &str) -> bool {
    raw.contains("404") || lc.contains("not found")
}

/// 频率/额度限制（429）
fn is_429(raw: &str, lc: &str) -> bool {
    raw.contains("429")
        || lc.contains("rate limit")
        || lc.contains("too many requests")
        || lc.contains("quota exceeded")
        || lc.contains("rate limit exceeded")
}

/// 服务商服务器错误（5xx）
fn is_5xx(raw: &str, lc: &str) -> bool {
    ["500", "502", "503", "504"].iter().any(|c| raw.contains(c))
        || lc.contains("internal server error")
        || lc.contains("bad gateway")
        || lc.contains("service unavailable")
        || lc.contains("server error")
}

/// 请求/连接超时
fn is_timeout(lc: &str) -> bool {
    lc.contains("timed out")
        || lc.contains("timeout")
        || lc.contains("超时")
        || lc.contains("timedout")
}

/// 网络连接类错误
fn is_network(lc: &str) -> bool {
    lc.contains("connection refused")
        || lc.contains("connection reset")
        || lc.contains("error sending request")
        || lc.contains("failed to connect")
        || lc.contains("couldn't resolve")
        || lc.contains("dns")
        || lc.contains("tcp connect error")
        || lc.contains("network")
        || lc.contains("网络")
}

/// 空响应 / 无内容
fn is_empty_response(lc: &str) -> bool {
    lc.contains("empty response")
        || lc.contains("no content")
        || lc.contains("no text content")
        || lc.contains("没有内容")
        || lc.contains("无可用文本")
        || lc.contains("empty")
}
