//! 检索：向量余弦 + 关键词 LIKE 两路召回，RRF 融合排序。
//!
//! 与 BaiShou 的 `mergeRRF` 对齐：ftsWeight 0.3 / vectorWeight 0.7 / k=60；
//! 融合后的 `score` 优先用原始向量相似度（更直观），纯关键词命中的行才用 RRF 合并分。
//! `min_score` 过滤作用在最终 score 上（纯关键词命中的分数通常远低于 0.4，
//! 所以默认阈值下混合模式基本等价于"向量召回 + 关键词提权"）。

use super::store::MemoryRow;

/// RRF 常数 k。
const RRF_K: f64 = 60.0;
/// 关键词路权重。
const FTS_WEIGHT: f64 = 0.3;
/// 向量路权重。
const VECTOR_WEIGHT: f64 = 0.7;
/// 关键词路最多取多少条参与融合。
const FTS_CANDIDATES: usize = 50;

/// 搜索模式：纯语义 / 语义 + 关键词混合。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMode {
    Vector,
    #[default]
    Hybrid,
}

impl SearchMode {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("vector") => Self::Vector,
            _ => Self::Hybrid,
        }
    }
}

/// 一条检索结果（按 `memory_id` 聚合后的整条记忆）。
#[derive(Clone, Debug)]
pub struct SearchHit {
    pub memory_id: String,
    pub content: String,
    pub score: f64,
    /// 来源：`vector` / `fts` / `hybrid`。
    pub source: &'static str,
    pub created_at: i64,
}

/// 混合检索器（无状态，纯函数集合）。
#[derive(Default)]
pub struct HybridSearch;

impl HybridSearch {
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        query_vec: &[f32],
        query: &str,
        rows: &[MemoryRow],
        mode: SearchMode,
        top_k: usize,
        min_score: f64,
        time_range: Option<(i64, i64)>,
    ) -> Vec<SearchHit> {
        let rows: Vec<&MemoryRow> = rows
            .iter()
            .filter(|row| {
                time_range
                    .map(|(start, end)| row.created_at >= start && row.created_at <= end)
                    .unwrap_or(true)
            })
            .collect();

        // 向量路：整块记忆取其分块的最高相似度。
        let mut vector: Vec<(usize, f64)> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.embedding.len() == query_vec.len())
            .map(|(index, row)| (index, cosine(query_vec, &row.embedding) as f64))
            .filter(|(_, score)| *score > 0.0)
            .collect();
        vector.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        vector.truncate(top_k.max(FTS_CANDIDATES));

        // 关键词路：大小写不敏感的 LIKE（SQLite LIKE 对 ASCII 天然不区分大小写）。
        let mut fts: Vec<usize> = Vec::new();
        if mode == SearchMode::Hybrid {
            for (index, row) in rows.iter().enumerate() {
                if row.content.to_lowercase().contains(&query.to_lowercase()) {
                    fts.push(index);
                }
            }
            fts.truncate(FTS_CANDIDATES);
        }

        let mut merged: Vec<(usize, f64, &'static str)> = Vec::new();
        if mode == SearchMode::Hybrid && !fts.is_empty() {
            // 以关键词路为主干注入 RRF 分，再把向量分叠加上去。
            let mut score_map: Vec<(usize, f64, f64, f64)> = Vec::new(); // (row, fts, vec, raw_vec)
            for (rank, index) in fts.iter().enumerate() {
                let fts_score = FTS_WEIGHT / (RRF_K + rank as f64 + 1.0);
                score_map.push((*index, fts_score, 0.0, 0.0));
            }
            for (rank, (index, score)) in vector.iter().enumerate() {
                let rrf = VECTOR_WEIGHT / (RRF_K + rank as f64 + 1.0);
                match score_map.iter_mut().find(|(i, ..)| i == index) {
                    Some(entry) => {
                        entry.2 = rrf;
                        entry.3 = *score;
                    },
                    None => score_map.push((*index, 0.0, rrf, *score)),
                }
            }
            score_map.sort_by(|a, b| {
                (b.1 + b.2)
                    .partial_cmp(&(a.1 + a.2))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for (index, fts_score, vec_rrf, raw_vec) in score_map {
                let source = if fts_score > 0.0 && vec_rrf > 0.0 {
                    "hybrid"
                } else if fts_score > 0.0 {
                    "fts"
                } else {
                    "vector"
                };
                let score = if raw_vec > 0.0 {
                    raw_vec
                } else {
                    fts_score + vec_rrf
                };
                merged.push((index, score, source));
            }
        } else {
            for (index, score) in vector {
                merged.push((index, score, "vector"));
            }
        }

        merged.retain(|(_, score, _)| *score >= min_score);
        merged.truncate(top_k);
        merged
            .into_iter()
            .map(|(index, score, source)| {
                let row = rows[index];
                SearchHit {
                    memory_id: row.memory_id.clone(),
                    content: aggregate_memory(&rows, &row.memory_id),
                    score,
                    source,
                    created_at: row.created_at,
                }
            })
            .collect()
    }
}

/// 把同一张记忆的所有分块按序拼回完整文本。
fn aggregate_memory(rows: &[&MemoryRow], memory_id: &str) -> String {
    let mut parts: Vec<&MemoryRow> = rows
        .iter()
        .filter(|row| row.memory_id == memory_id)
        .copied()
        .collect();
    parts.sort_by_key(|row| row.chunk_index);
    parts
        .iter()
        .map(|row| row.content.as_str())
        .collect::<Vec<_>>()
        .join("")
}

/// 余弦相似度（输入向量已 L2 归一化，这里仍按定义计算以兼容未归一化的输入）。
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
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
    if denom == 0.0 { 0.0 } else { dot / denom }
}
