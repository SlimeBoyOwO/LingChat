//! 记忆语义索引：利用嵌入服务为记忆片段生成向量，提供语义检索与去重。
//!
//! 索引内容来源：
//! - 记忆库（MemoryBank）各段：长期经历、用户信息、重要约定。
//! - 手动笔记（每个角色若干条）。
//!
//! 运行时能力：
//! - [`MemoryIndex::search`]：给定查询文本，返回按余弦相似度排序的 top-k 记忆片段。
//! - [`MemoryIndex::is_duplicate`]：判断新片段与已有片段是否语义重复（用于去重）。

use std::sync::Arc;

use tokio::sync::RwLock;

use super::service::{EmbeddingManager, cosine, cosine_ge_threshold};

/// 跳过检索/去重的零碎文本阈值（低于此长度不建索引，避免噪音）。
const MIN_INDEX_CHARS: usize = 2;
/// 去重判定阈值（余弦相似度高于此值视为重复）。
const DUP_THRESHOLD: f32 = 0.88;
/// 检索返回的上限。
const DEFAULT_TOP_K: usize = 4;

/// 一条可检索的记忆片段。
#[derive(Debug, Clone)]
pub struct Fragment {
    pub id: String,
    pub source: FragmentSource,
    pub text: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentSource {
    /// 记忆库段落（long_term / user_info / promises / short_term）。
    MemoryBank(&'static str),
    /// 手动笔记（id）。
    Note(String),
}

impl FragmentSource {
    fn label(&self) -> &str {
        match self {
            FragmentSource::MemoryBank(s) => s,
            FragmentSource::Note(_) => "note",
        }
    }
}

/// 记忆语义索引。持有向量的内存副本，串行更新。
pub struct MemoryIndex {
    manager: Arc<EmbeddingManager>,
    fragments: RwLock<Vec<Fragment>>,
    /// 为空的 fragment id 集合（语义检索时排除零碎文本）。
    min_chars: usize,
    top_k: usize,
}

impl MemoryIndex {
    pub fn new(manager: Arc<EmbeddingManager>) -> Self {
        Self {
            manager,
            fragments: RwLock::new(Vec::new()),
            min_chars: MIN_INDEX_CHARS,
            top_k: DEFAULT_TOP_K,
        }
    }

    /// 嵌入服务是否可用。
    pub fn enabled(&self) -> bool {
        self.manager.configured()
    }

    /// 增量添加若干片段（去重后保留）。返回实际新增数量。
    pub async fn add(&self, texts: &[(String, FragmentSource)]) -> usize {
        let mut added = 0usize;
        let encoded = self
            .manager
            .embed_passages(&texts.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>())
            .await;
        let Some(embedded) = encoded else {
            return 0;
        };
        let mut guard = self.fragments.write().await;
        for ((text, source), emb) in texts.iter().zip(embedded.into_iter()) {
            if text.chars().count() < self.min_chars {
                continue;
            }
            let id = fragment_id(&emb.text, source);
            // 单遍去重：精确 id 匹配视为重复；否则做余弦去重（L2 前缀剪枝快速排除）。
            let dominated = guard.iter().any(|f| {
                f.id == id
                    || cosine_ge_threshold(&[f.vector.as_slice()], &emb.vector, DUP_THRESHOLD)
            });
            if dominated {
                continue;
            }
            guard.push(Fragment {
                id,
                source: source.clone(),
                text: text.clone(),
                vector: emb.vector,
            });
            added += 1;
        }
        added
    }

    /// 清空索引。
    pub async fn clear(&self) {
        self.fragments.write().await.clear();
    }

    /// 更新一条已有片段（笔记内容被编辑时用）。用 `source + old_text` 定位旧片段，
    /// 找到则整体替换 text/vector；找不到则按新增处理（新片段直接入索引，不去重，
    /// 因为这是对既有记忆的明确更新）。返回是否实际改变了索引。
    pub async fn replace(&self, source: &FragmentSource, old_text: &str, new_text: &str) -> bool {
        let new_text = new_text.trim();
        // 新内容过短：等语义删除
        if new_text.chars().count() < self.min_chars {
            return self.remove(source, old_text).await;
        }
        let Some(emb) = self.manager.embed_passages(&[new_text.to_string()]).await else {
            return false;
        };
        let mut guard = self.fragments.write().await;
        let new_id = fragment_id(new_text, source);
        if old_text.trim().is_empty() {
            // 旧文本为空 → 纯新增语义
            if guard
                .iter()
                .any(|f| &f.source == source && f.text == new_text)
            {
                return false;
            }
            guard.push(Fragment {
                id: new_id,
                source: source.clone(),
                text: new_text.to_string(),
                vector: emb[0].vector.clone(),
            });
            return true;
        }
        if let Some(f) = guard
            .iter_mut()
            .find(|f| &f.source == source && f.text == old_text)
        {
            f.id = new_id;
            f.text = new_text.to_string();
            f.vector = emb[0].vector.clone();
            true
        } else {
            if guard
                .iter()
                .any(|f| &f.source == source && f.text == new_text)
            {
                return false;
            }
            guard.push(Fragment {
                id: new_id,
                source: source.clone(),
                text: new_text.to_string(),
                vector: emb[0].vector.clone(),
            });
            true
        }
    }

    /// 移除一条片段（笔记被删除时用）。返回是否实际删除了片段。
    pub async fn remove(&self, source: &FragmentSource, text: &str) -> bool {
        let mut guard = self.fragments.write().await;
        let before = guard.len();
        guard.retain(|f| !(&f.source == source && f.text == text));
        guard.len() != before
    }

    /// 底层嵌入管理器（供状态查询：维度/模型名/错误诊断）。
    pub fn manager(&self) -> &EmbeddingManager {
        &self.manager
    }

    /// 底层管理器句柄副本（供异步状态查询在释放 GameStatus 锁后使用）。
    pub fn manager_arc(&self) -> Arc<EmbeddingManager> {
        self.manager.clone()
    }

    /// 语义检索：给定查询，返回按相似度降序的片段。
    pub async fn search(&self, query: &str, top_k: Option<usize>) -> Vec<SearchHit> {
        self.search_context().await.search(query, top_k).await
    }

    /// 生成检索上下文快照：克隆片段向量 + 管理器句柄（短暂读取锁 + 内存复制）。
    ///
    /// 调用方可在快照后立即释放 `GameStatus` 等外部锁，再在快照上执行
    /// `search`，避免 ONNX 推理期间长时间独占全局锁阻塞消息处理等其它逻辑。
    pub async fn search_context(&self) -> SearchContext {
        let guard = self.fragments.read().await;
        SearchContext {
            manager: self.manager.clone(),
            fragments: guard.clone(),
            min_chars: self.min_chars,
            top_k: self.top_k,
        }
    }

    /// 判断候选片段是否与已有索引片段语义重复。
    pub async fn is_duplicate(&self, text: &str, threshold: Option<f32>) -> bool {
        if text.chars().count() < self.min_chars {
            return false;
        }
        let Some(vec) = self.manager.embed_passages(&[text.to_string()]).await else {
            return false;
        };
        let v = &vec[0].vector;
        let threshold = threshold.unwrap_or(DUP_THRESHOLD);
        let guard = self.fragments.read().await;
        let candidates: Vec<&[f32]> = guard.iter().map(|f| f.vector.as_slice()).collect();
        cosine_ge_threshold(&candidates, v, threshold)
    }

    /// 当前索引片段数。
    pub async fn len(&self) -> usize {
        self.fragments.read().await.len()
    }

    /// 把检索命中片段格式化为注入上下文的文本块。
    pub fn format_hits(hits: &[SearchHit]) -> String {
        if hits.is_empty() {
            return String::new();
        }
        let mut lines = Vec::new();
        lines.push("【语义召回的记忆】".to_string());
        for h in hits {
            if h.score < 0.35 {
                continue;
            }
            let tag = match &h.source {
                FragmentSource::MemoryBank(s) => format!("[{}]", s),
                FragmentSource::Note(_) => "[笔记]".to_string(),
            };
            lines.push(format!("{tag} {:.0}% {}", h.score * 100.0, h.text));
        }
        if lines.len() == 1 {
            return String::new();
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub fragment_id: String,
    pub source: FragmentSource,
    pub text: String,
    pub score: f32,
}

/// 检索上下文快照：持有片段向量副本与管理器句柄，可在释放外部锁后独立检索。
///
/// 由 [`MemoryIndex::search_context`] 生成，目的是把 ONNX 编码推理移出
/// `GameStatus` 等全局锁的作用域，避免 CPU 密集推理阻塞其它对话/工具逻辑。
#[derive(Clone)]
pub struct SearchContext {
    manager: Arc<EmbeddingManager>,
    fragments: Vec<Fragment>,
    min_chars: usize,
    top_k: usize,
}

impl SearchContext {
    /// 语义检索：给定查询，返回按余弦相似度降序的片段。
    pub async fn search(&self, query: &str, top_k: Option<usize>) -> Vec<SearchHit> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let k = top_k.unwrap_or(self.top_k).min(20);
        let Some(query_vec) = self.manager.encode_queries(&[query.to_string()]).await else {
            return Vec::new();
        };
        let qv = &query_vec[0];
        if self.fragments.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<SearchHit> = self
            .fragments
            .iter()
            .filter(|f| f.text.chars().count() >= self.min_chars)
            .map(|f| SearchHit {
                fragment_id: f.id.clone(),
                source: f.source.clone(),
                text: f.text.clone(),
                score: cosine(qv, &f.vector),
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(k);
        scored
    }
}

fn fragment_id(text: &str, source: &FragmentSource) -> String {
    format!("{}::{}", source.label(), text)
}

impl Default for MemoryIndex {
    fn default() -> Self {
        // 构造一个未配置 manager 的空索引（manager 不会真正加载模型）。
        Self::new(Arc::new(EmbeddingManager::new(
            super::service::EmbeddingConfig::default(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_index_is_disabled() {
        let idx = MemoryIndex::default();
        assert!(!idx.enabled());
    }

    #[test]
    fn format_hits_empty_returns_empty() {
        assert_eq!(MemoryIndex::format_hits(&[]), "");
    }

    #[test]
    fn format_hits_low_score_is_filtered() {
        let hits = vec![SearchHit {
            fragment_id: "x".into(),
            source: FragmentSource::MemoryBank("long_term"),
            text: "内容".into(),
            score: 0.1,
        }];
        assert_eq!(MemoryIndex::format_hits(&hits), "");
    }

    #[tokio::test]
    async fn disabled_index_replace_is_noop() {
        let idx = MemoryIndex::default();
        // 未配置嵌入：replace 无法编码 → 返回 false 且索引为空
        assert!(
            !idx.replace(&FragmentSource::Note("ai".into()), "旧笔记", "新笔记",)
                .await
        );
        assert_eq!(idx.len().await, 0);
    }

    #[tokio::test]
    async fn disabled_index_remove_is_noop() {
        let idx = MemoryIndex::default();
        assert!(
            !idx.remove(&FragmentSource::Note("ai".into()), "不存在的笔记")
                .await
        );
    }

    #[tokio::test]
    async fn manager_of_default_index_is_not_configured() {
        let idx = MemoryIndex::default();
        assert!(!idx.manager().configured());
    }
}
