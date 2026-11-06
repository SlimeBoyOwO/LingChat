//! 文本嵌入子系统（语义向量）。
//!
//! [`Embedder`] 是本 crate 唯一的对外契约：笔记 / 记忆检索与 Tool RAG 只依赖它。
//! 本地 ONNX 实现见 [`engine::EmbeddingEngine`]；云端实现由宿主提供（本 crate 位于
//! 依赖链最底层，不引 HTTP 栈）。[`cosine_similarity`] 与 [`l2_normalize`] 是两种
//! 后端共用的向量数学。

pub mod engine;

pub use engine::{EmbeddingEngine, MODEL_DIR_NAME};

use anyhow::Result;

/// 句向量后端。
///
/// 方法一律**同步阻塞**（本地是 ONNX 推理，云端内部自己驱动一次异步请求），
/// 调用方负责放进 `spawn_blocking`——整条检索链路的签名与并发模型因此不必区分后端。
pub trait Embedder: Send + Sync {
    /// 是否可用。本地看模型齐备与否，云端看配置就绪且未熔断。
    fn is_enabled(&self) -> bool;

    /// 向量维度。云端在首次成功响应前无法确定，返回 `None` 表示还不能建库。
    fn dimension(&self) -> Option<usize>;

    /// 向量索引的分库目录名。不同后端的向量空间永不落进同一个库文件。
    /// 与 [`Self::dimension`] 同理，未定向时返回 `None`。
    fn namespace(&self) -> Option<String>;

    /// 单条文本的 L2 归一化句向量。
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// 批量文本的 L2 归一化句向量，返回顺序与输入一致。
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// L2 归一化；零向量原样返回，避免除零。
pub fn l2_normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
    vector
}

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
