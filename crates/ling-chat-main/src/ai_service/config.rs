//! 本模块内部的 `AIService` 专用配置；与 app 级 [`crate::config::AppConfig`] 不同：
//! 前者承载会话上下文（比如当前主用户、已连接的前端 client id 集合等），
//! 后者承载可由用户改动并持久化的运行时偏好（LLM API key、TTS 开关等）。

use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct AIServiceConfig {
    pub clients: HashSet<String>,
    pub last_active_client: Option<String>,
}
