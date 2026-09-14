//! 记忆嵌入支持：为记忆系统提供文本向量化（语义检索 + 去重）。
//!
//! 加载中小型嵌入模型（默认
//! `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2`，多语言含中/英/日，
//! 维度 384），在 Rust 侧以 ONNX Runtime 直接推理，不依赖 Python。分词同时支持
//! WordPiece（BERT 系）与 SentencePiece Unigram（XLM-R 系），推理用 `ort`（与 ASR/TTS/
//! 情绪分类共用同一套依赖）。
//!
//! - [`service::EmbeddingManager`]：加载模型并提供 encode/相似度能力。
//! - [`memory_index::MemoryIndex`]：记忆片段向量索引，支持检索与去重。
//! - [`tokenizer::Tokenizer`]：统一的分词器接口（WordPiece / SentencePiece Unigram）。

pub mod memory_index;
pub mod service;
pub mod tokenizer;

pub use memory_index::{FragmentSource, MemoryIndex};
pub use service::{EmbeddingConfig, EmbeddingManager};
