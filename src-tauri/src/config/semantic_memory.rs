//! 独立语义记忆配置（`AppConfig.semantic_memory`）。
//!
//! 语义记忆与普通记忆库（MemoryBank）/手动笔记完全解耦：内容由 AI 通过
//! `semantic_mem_*` 工具写入一颗独立的向量数据库（SQLite），与
//! `embedding`（嵌入引擎）共享同一个本地 ONNX 模型做编码。

use serde::{Deserialize, Serialize};

use super::keys;

fn default_enabled() -> bool {
    false
}

fn default_top_k() -> u32 {
    4
}

/// 独立语义记忆配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemoryConfig {
    /// 是否启用独立的语义记忆库（需嵌入引擎就绪后才能编码/检索）。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 每轮对话自动注入上下文的最多召回条数（1–20）。
    #[serde(default = "default_top_k")]
    pub top_k: u32,
}

impl Default for SemanticMemoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            top_k: default_top_k(),
        }
    }
}

impl SemanticMemoryConfig {
    /// 从已打开的 store 读取。缺失项使用默认值。
    pub fn from_store(store: Option<&tauri_plugin_store::Store<tauri::Wry>>) -> Self {
        let get_bool = |key: &str, default: bool| -> bool {
            store
                .and_then(|s| s.get(key))
                .and_then(|v| v.as_bool())
                .unwrap_or(default)
        };
        let get_u32 = |key: &str, default: u32| -> u32 {
            store
                .and_then(|s| s.get(key))
                .and_then(|v| {
                    v.as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|n| (1..=20).contains(n))
                })
                .unwrap_or(default)
        };

        Self {
            enabled: get_bool(keys::SEMANTIC_MEMORY_ENABLED, default_enabled()),
            top_k: get_u32(keys::SEMANTIC_MEMORY_TOP_K, default_top_k()),
        }
    }
}