//! 情绪识别子系统（ONNX 推理）。
//!
//! 对应 Python `ling_chat.core.emotion`。
//!
//! 除了 [`EmotionClassifier`] 本体，这里还放了一份**进程级**的分类器句柄：
//! 消息链路在启动时会把分类器交给 `MessageProcessor`，但 TTS 适配器（例如
//! sherpa-onnx 把情绪落成语速/停顿）也需要在「上游没给情绪」时按正文补一次
//! 预测，而适配器是按角色懒加载构造的，拿不到启动期那份 `Arc`。全局句柄让
//! 「模型只加载一次、谁都能用」。

pub mod classifier;

pub use classifier::EmotionClassifier;

use std::sync::{Arc, OnceLock};

static GLOBAL: OnceLock<Arc<EmotionClassifier>> = OnceLock::new();

/// 注册全局情绪分类器。应用启动时调用一次；重复调用会被忽略。
pub fn set_global_classifier(classifier: Arc<EmotionClassifier>) {
    let _ = GLOBAL.set(classifier);
}

/// 取全局情绪分类器；配置里禁用或模型加载失败时为 `None`。
pub fn global_classifier() -> Option<&'static Arc<EmotionClassifier>> {
    GLOBAL.get()
}

/// 用全局分类器给一段文本预测情绪标签。
///
/// 分类器未注册、模型被禁用、或预测退化成「原样透传」时返回 `None`——调用方
/// 据此保留自己的兜底（例如 sherpa-onnx 退回中立韵律）。
pub fn classify_text(text: &str) -> Option<String> {
    classify_with(global_classifier()?, text)
}

/// 用指定实例预测情绪标签；语义同 [`classify_text`]，但没有分类器时返回 `None`。
///
/// 单测要注入一个真实模型（`EmotionClassifier::load`）验证链路，所以单独留出
/// 这个入口，不必先往全局句柄里注册（`OnceLock` 一注册就影响整个测试进程）。
pub fn classify_with(classifier: &EmotionClassifier, text: &str) -> Option<String> {
    let prediction = classifier.predict(text, None);
    if prediction.disabled || prediction.label.trim().is_empty() {
        return None;
    }
    Some(prediction.label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_text_is_quiet_without_classifier() {
        // 单测进程里没人注册全局分类器（OnceLock 进程级，注册点在 src-tauri 的
        // setup 里），此时必须安静地返回 None 而不是 panic。
        let out = classify_text("今天天气真不错");
        if global_classifier().is_none() {
            assert!(out.is_none(), "未注册分类器时应返回 None，实际 {out:?}");
        }
    }
}
