pub mod affection;
pub mod asr;
pub mod config;
// 嵌入引擎实现已下沉到 notes crate，这里保持 `ai_service::embedding` 路径稳定。
pub use ling_chat_notes::embedding;
// 云端向量化后端与本地/云端路由（依赖 HTTP 栈，不能放进 notes crate）。
pub mod embedding_remote;
pub mod emotion;
pub mod game_system;
pub mod god_agent;
pub mod llm;
pub mod message_system;
pub mod proactive_system;
pub mod screen_analyzer;
pub mod service;
pub mod skill_agent;
pub mod tools;
pub mod translator;
pub mod tts;
pub mod types;
pub mod worldbook;
