//! OpenAI Codex（ChatGPT 订阅）模块。
//!
//! 收拢 Codex 相关的全部实现：
//! - `auth`：设备码 OAuth 凭据（登录/刷新/额度查询）
//! - `provider`：Responses API 流式对话 provider
//! - `models`：登录账号的在线模型目录

pub mod auth;
mod models;
pub mod provider;
