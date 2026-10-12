//! 检索：向量余弦（可选）+ 字符 n-gram 词法打分，两路 RRF 融合。
//!
//! - **词法路**（不依赖任何嵌入模型）：中文按 2 字滑窗 bigram + 单字，ASCII 按词；
//!   用「查询词项在候选集上的 IDF 加权覆盖率」打分，范围 [0,1]。这正是「非嵌入式
//!   RAG」——没有嵌入模型时也能检索；
//! - **向量路**：查询向量与行向量的余弦（配置了嵌入模型时才有）；
//! - **Hybrid**：两路 RRF 融合（沿用 BaiShou 的 ftsWeight 0.3 / vectorWeight 0.7 /
//!   k=60），展示分优先用原始向量余弦，纯词法命中的行用覆盖率。没有向量时 Hybrid
//!   自动退化为纯词法。
//!
//! `min_score` 过滤作用在最终 score 上（余弦与覆盖率都在 [0,1]）。

use std::collections::{HashMap, HashSet};

use super::store::MemoryRow;

/// RRF 常数 k。
const RRF_K: f64 = 60.0;
/// 词法路权重。
const LEXICAL_WEIGHT: f64 = 0.3;
/// 向量路权重。
const VECTOR_WEIGHT: f64 = 0.7;
/// 词法路最多取多少条参与融合。
const LEXICAL_CANDIDATES: usize = 50;

/// 中文 bigram 词项权重。
const CJK_BIGRAM_WEIGHT: f64 = 1.0;
/// 长 run 内单字词项权重（弱于 bigram，避免单字噪音）。
const CJK_UNIGRAM_WEIGHT: f64 = 0.4;
/// 单字 run（整个 run 只有一个字）的权重。
const CJK_SOLO_WEIGHT: f64 = 1.0;
/// ASCII 词的权重。
const ASCII_WORD_WEIGHT: f64 = 1.0;

/// 搜索模式：纯语义 / 语义 + 词法混合 / 纯词法。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMode {
    Vector,
    #[default]
    Hybrid,
    /// 纯词法（不嵌入；无嵌入模型时自动使用）。
    Lexical,
}

/// 一条检索结果（按 `memory_id` 聚合后的整条记忆）。
#[derive(Clone, Debug)]
pub struct SearchHit {
    pub memory_id: String,
    pub content: String,
    pub score: f64,
    /// 来源：`vector` / `lexical` / `hybrid`。
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

        let merged = self.rank(query_vec, query, &rows, mode, top_k);
        // 检索按「分块行」打分，但输出按整条记忆聚合：同一 `memory_id` 的多个分块
        // 都可能命中，这里按 `memory_id` 去重、保留融合排序中位次最高的那块，避免
        // RAG 注入 / memory_query 返回重复的整条记忆。
        let mut seen: HashSet<&str> = HashSet::new();
        merged
            .into_iter()
            .filter(|(index, _, _)| seen.insert(rows[*index].memory_id.as_str()))
            .filter(|(_, score, _)| *score >= min_score)
            .take(top_k)
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

    /// 计算排序结果 `(行下标, 展示分, 来源)`，按融合顺序排列（未截断）。
    fn rank(
        &self,
        query_vec: &[f32],
        query: &str,
        rows: &[&MemoryRow],
        mode: SearchMode,
        top_k: usize,
    ) -> Vec<(usize, f64, &'static str)> {
        if rows.is_empty() {
            return Vec::new();
        }
        let lexical = lexical_scores(query, rows);

        // 词法候选：得分 > 0 的按覆盖率降序。
        let mut lex: Vec<(usize, f64)> = lexical
            .iter()
            .enumerate()
            .filter(|(_, score)| **score > 0.0)
            .map(|(index, score)| (index, *score))
            .collect();
        lex.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        lex.truncate(LEXICAL_CANDIDATES.max(top_k));

        let has_vector = !query_vec.is_empty() && mode != SearchMode::Lexical;
        if !has_vector {
            return lex.into_iter().map(|(i, s)| (i, s, "lexical")).collect();
        }

        if mode == SearchMode::Vector {
            let mut vector: Vec<(usize, f64)> = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.embedding.len() == query_vec.len())
                .map(|(index, row)| (index, cosine(query_vec, &row.embedding) as f64))
                .filter(|(_, score)| *score > 0.0)
                .collect();
            vector.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            vector.truncate(top_k.max(LEXICAL_CANDIDATES));
            return vector.into_iter().map(|(i, s)| (i, s, "vector")).collect();
        }

        // Hybrid：RRF 融合（排序按 RRF，展示分优先原始向量余弦）。
        let mut vec_rank: Vec<(usize, f64)> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.embedding.len() == query_vec.len())
            .map(|(index, row)| (index, cosine(query_vec, &row.embedding) as f64))
            .filter(|(_, score)| *score > 0.0)
            .collect();
        vec_rank.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        vec_rank.truncate(LEXICAL_CANDIDATES.max(top_k));

        // index -> (lexical, vector, rrf)
        let mut map: HashMap<usize, (f64, f64, f64)> = HashMap::new();
        for (rank, (index, score)) in lex.iter().enumerate() {
            let entry = map.entry(*index).or_insert((0.0, 0.0, 0.0));
            entry.0 = *score;
            entry.2 += LEXICAL_WEIGHT / (RRF_K + rank as f64 + 1.0);
        }
        for (rank, (index, score)) in vec_rank.iter().enumerate() {
            let entry = map.entry(*index).or_insert((0.0, 0.0, 0.0));
            entry.1 = *score;
            entry.2 += VECTOR_WEIGHT / (RRF_K + rank as f64 + 1.0);
        }

        let mut merged: Vec<(usize, f64, &'static str, f64)> = map
            .into_iter()
            .map(|(index, (lex_score, vec_score, rrf))| {
                let source = if vec_score > 0.0 && lex_score > 0.0 {
                    "hybrid"
                } else if vec_score > 0.0 {
                    "vector"
                } else {
                    "lexical"
                };
                // 展示分用原始相似度：有向量用余弦，否则用词法覆盖率。
                let score = if vec_score > 0.0 {
                    vec_score
                } else {
                    lex_score
                };
                (index, score, source, rrf)
            })
            .collect();
        merged.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        merged
            .into_iter()
            .map(|(i, s, src, _)| (i, s, src))
            .collect()
    }
}

/// 对候选行逐一做「字符 n-gram + IDF 加权覆盖率」打分（不依赖嵌入模型）。
///
/// 长查询（如自动 RAG 的最近对话拼接）对「整段覆盖率」极不友好——一行相关就足以
/// 说明命中。因此先把 query 拆成片段（换行 / 句末标点），对每个片段算覆盖率，**取
/// 片段最大值**作为该行得分：打分 = max_seg Σ_{t∈seg∩行} w(t)·idf(t) ÷ Σ_{t∈seg} w(t)·idf(t)。
///
/// 其中 idf 由候选集内的文档频率估计；行正文包含整段查询时给 0.95 下限。
/// 返回与 `rows` 等长、范围 [0,1] 的分数。
pub fn lexical_scores(query: &str, rows: &[&MemoryRow]) -> Vec<f64> {
    let mut scores = vec![0.0f64; rows.len()];
    if rows.is_empty() {
        return scores;
    }
    let segments = query_segments(query);
    if segments.is_empty() {
        return scores;
    }

    let doc_sets: Vec<HashSet<String>> =
        rows.iter().map(|row| doc_term_set(&row.content)).collect();
    let n = doc_sets.len() as f64;

    for segment in &segments {
        let terms = weighted_terms(segment);
        if terms.is_empty() {
            continue;
        }
        // 片段内查询词项去重，同一词项取最大权重。
        let mut q_weight: HashMap<&str, f64> = HashMap::new();
        for (term, weight) in &terms {
            let entry = q_weight.entry(term.as_str()).or_insert(0.0);
            if *weight > *entry {
                *entry = *weight;
            }
        }
        let idf: HashMap<&str, f64> = q_weight
            .keys()
            .map(|term| {
                let df = doc_sets.iter().filter(|set| set.contains(*term)).count() as f64;
                (*term, ((n + 1.0) / (df + 1.0)).ln() + 1.0)
            })
            .collect();
        let total: f64 = q_weight
            .iter()
            .map(|(term, weight)| weight * idf.get(*term).copied().unwrap_or(1.0))
            .sum();
        if total <= 0.0 {
            continue;
        }
        for (index, set) in doc_sets.iter().enumerate() {
            let mut matched = 0.0;
            for (term, weight) in &q_weight {
                if set.contains(*term) {
                    matched += weight * idf.get(*term).copied().unwrap_or(1.0);
                }
            }
            let score = (matched / total).clamp(0.0, 1.0);
            if score > scores[index] {
                scores[index] = score;
            }
        }
    }

    // 整段查询被行正文完整包含时给一个强下限（短查询 / `memory_query` 精确命中）。
    let needle = query.trim();
    if !needle.is_empty() {
        for (index, row) in rows.iter().enumerate() {
            if row.content.contains(needle) {
                scores[index] = scores[index].max(0.95);
            }
        }
    }
    scores
}

/// 把查询拆成可独立匹配的片段：按换行与句末标点切分，去掉空片段。
/// 逗号不切，避免把一句话拆碎后由极短片段刷高覆盖率。
fn query_segments(query: &str) -> Vec<String> {
    query
        .split(|c: char| {
            c == '\n' || matches!(c, '。' | '！' | '？' | '；' | '!' | '?' | ';' | '…')
        })
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect()
}

/// 把文本切成带权词项：中文 run 产出 bigram（权重 1.0）与内部单字（0.4），
/// 单字 run 产出该字（1.0），连续 ASCII 字母数字作为整词（1.0）。
fn weighted_terms(text: &str) -> Vec<(String, f64)> {
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if is_cjk(c) {
            let start = i;
            while i < chars.len() && is_cjk(chars[i]) {
                i += 1;
            }
            let run = &chars[start..i];
            if run.len() == 1 {
                out.push((run[0].to_string(), CJK_SOLO_WEIGHT));
            } else {
                for k in 0..run.len() {
                    out.push((run[k].to_string(), CJK_UNIGRAM_WEIGHT));
                    if k + 1 < run.len() {
                        out.push((format!("{}{}", run[k], run[k + 1]), CJK_BIGRAM_WEIGHT));
                    }
                }
            }
        } else if c.is_ascii_alphanumeric() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                i += 1;
            }
            out.push((chars[start..i].iter().collect(), ASCII_WORD_WEIGHT));
        } else {
            i += 1;
        }
    }
    out
}

/// 行正文的词项集合（只保留词项本身，忽略权重；用于 idf 的文档频率统计与包含判断）。
fn doc_term_set(text: &str) -> HashSet<String> {
    weighted_terms(text)
        .into_iter()
        .map(|(term, _)| term)
        .collect()
}

/// 是否 CJK（含假名 / 谚文），用于中文按字切 bigram。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF      // 平假名 / 片假名
        | 0x3400..=0x4DBF    // CJK 扩展 A
        | 0x4E00..=0x9FFF    // CJK 基本
        | 0xF900..=0xFAFF    // CJK 兼容
        | 0xAC00..=0xD7AF    // 谚文
        | 0x20000..=0x2FA1F  // CJK 扩展 B+
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(memory_id: &str, content: &str) -> MemoryRow {
        MemoryRow {
            id: 0,
            memory_id: memory_id.to_string(),
            chunk_index: 0,
            content: content.to_string(),
            tags: None,
            embedding: Vec::new(),
            model: String::new(),
            created_at: 0,
        }
    }

    /// 同一记忆的多个分块（id / chunk_index 依次递增）。
    fn chunk(memory_id: &str, chunk_index: i32, content: &str) -> MemoryRow {
        MemoryRow {
            id: chunk_index,
            memory_id: memory_id.to_string(),
            chunk_index,
            content: content.to_string(),
            tags: None,
            embedding: Vec::new(),
            model: String::new(),
            created_at: 0,
        }
    }

    #[test]
    fn lexical_matches_related_chinese_content() {
        let rows = vec![
            row("a", "用户喜欢喝冰美式咖啡，每天早上一杯"),
            row("b", "她们约定每周三晚上一起打游戏"),
            row("c", "A型血摩羯座，讨厌被打断说话"),
        ];
        let refs: Vec<&MemoryRow> = rows.iter().collect();
        let scores = lexical_scores("我想喝咖啡", &refs);
        assert!(scores[0] > scores[1], "咖啡相关应排前: {scores:?}");
        assert!(scores[0] > scores[2], "咖啡相关应排前: {scores:?}");
        assert!(scores[0] > 0.25, "应有明显命中: {}", scores[0]);
    }

    #[test]
    fn lexical_max_pools_over_query_segments() {
        // 长查询（多行）里只有一行与某条记忆相关；逐行取最大值应让该记忆胜出，
        // 而不会因为整段覆盖率低而被埋没。
        let rows = vec![
            row("coffee", "用户喜欢喝冰美式咖啡"),
            row("game", "她们每周三晚上一起打游戏"),
        ];
        let refs: Vec<&MemoryRow> = rows.iter().collect();
        let query = "今天有什么安排？\n我们周三晚上照常一起打游戏吧\n哦对了随便提一句天气";
        let scores = lexical_scores(query, &refs);
        assert!(
            scores[1] > scores[0],
            "应按最相关的一行命中 game: {scores:?}"
        );
        assert!(scores[1] > 0.3, "相关行应有明显命中: {}", scores[1]);
    }

    #[test]
    fn lexical_handles_single_char_and_ascii() {
        let rows = vec![
            row("a", "主角养了一只叫团子的猫"),
            row("b", "完全无关的内容"),
        ];
        let refs: Vec<&MemoryRow> = rows.iter().collect();
        assert!(lexical_scores("猫", &refs)[0] > lexical_scores("猫", &refs)[1]);
        let rows2 = vec![
            row("a", "I love cold brew coffee"),
            row("b", "unrelated text"),
        ];
        let refs2: Vec<&MemoryRow> = rows2.iter().collect();
        assert!(lexical_scores("coffee", &refs2)[0] > lexical_scores("coffee", &refs2)[1]);
    }

    #[test]
    fn lexical_mode_ranks_without_vectors() {
        let rows = vec![row("a", "用户喜欢喝冰美式咖啡"), row("b", "今天天气很好")];
        let hits = HybridSearch.run(&[], "冰美式", &rows, SearchMode::Lexical, 5, 0.2, None);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory_id, "a");
        assert_eq!(hits[0].source, "lexical");
    }

    #[test]
    fn results_dedupe_multi_chunk_memories() {
        // 同一记忆被切成两块，两块都与查询相关：输出应只保留一条整条记忆。
        let rows = vec![
            chunk(
                "mem-A",
                0,
                "用户喜欢喝冰美式咖啡，每天早上一杯，周末会换冷萃。",
            ),
            chunk("mem-A", 1, "用户讨厌被拉进奶茶拼单，只喝不加糖的纯咖啡。"),
            row("mem-B", "她们约定每周三晚上一起打游戏。"),
        ];
        let hits = HybridSearch.run(
            &[],
            "用户喜欢喝咖啡",
            &rows,
            SearchMode::Lexical,
            5,
            0.0,
            None,
        );
        let ids: Vec<&str> = hits.iter().map(|h| h.memory_id.as_str()).collect();
        assert_eq!(ids, vec!["mem-A"], "只应出一条整条记忆，不应重复: {ids:?}");
        // 对拍：不分块时同一作答内容也只有一条命中，多分块去重后数量应一致。
        let dedup_content = hits[0].content.clone();
        assert!(dedup_content.contains("冰美式咖啡"));
    }
}
