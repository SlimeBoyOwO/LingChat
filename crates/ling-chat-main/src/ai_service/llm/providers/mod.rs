mod genai_provider;
mod kimi_code;

pub use crate::ai_service::llm::codex::provider::CodexProvider;
pub use genai_provider::GenaiProvider;
pub use kimi_code::KimiCodeProvider;

// 视觉调用（llm::vision）复用同一套 base_url 规范化，避免手写拼接出现 #787 式的尾斜杠坑。
pub(crate) use genai_provider::normalize_base_url;
