//! 文本嵌入子系统（ONNX 语义向量）。
//!
//! 提供笔记 / 记忆检索与 Tool RAG 共用的句向量能力。调用方只需要
//! [`EmbeddingEngine::embed`] 与 [`cosine_similarity`]。

pub mod engine;

pub use engine::{EmbeddingEngine, MODEL_DIR_NAME};

/// 余弦相似度。
///
/// 维度不一致或任一侧为零向量时返回 0（不做 panic / NaN 传播）。引擎产出的向量
/// 已 L2 归一化，此处仍按通用余弦实现，方便别的来源的向量直接复用。
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom <= f32::EPSILON {
        return 0.0;
    }
    dot / denom
}

#[cfg(test)]
mod tests {
    use super::cosine_similarity;

    #[test]
    fn identical_direction_scores_one() {
        let score = cosine_similarity(&[1.0, 0.0], &[2.0, 0.0]);
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors_score_zero() {
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn opposite_vectors_score_minus_one() {
        let score = cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]);
        assert!((score + 1.0).abs() < 1e-6);
    }

    #[test]
    fn mismatched_length_or_zero_vector_scores_zero() {
        assert_eq!(cosine_similarity(&[1.0, 2.0], &[1.0]), 0.0);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }
}
