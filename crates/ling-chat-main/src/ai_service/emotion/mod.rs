//! 情绪识别子系统（ONNX 推理）。
//!
//! 对应 Python `ling_chat.core.emotion`。

pub mod classifier;

pub use classifier::EmotionClassifier;
