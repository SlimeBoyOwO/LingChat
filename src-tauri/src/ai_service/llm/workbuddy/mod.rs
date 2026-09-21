//! WorkBuddy（腾讯 CodeBuddy）订阅模块。
//!
//! 收拢 WorkBuddy 相关的全部实现：
//! - `auth`：浏览器授权登录、令牌刷新、凭据存储与订阅额度查询
//! - `models`：登录账号的在线模型目录（`/v3/config`）
//! - `provider`：OpenAI 兼容 Chat Completions 流式对话 provider
//!
//! 协议参考 astrbot_plugin_workbuddy_provider（H:\botA\bot\AstrBotLauncher-0.2.0）。

pub mod auth;
mod models;
pub mod provider;
