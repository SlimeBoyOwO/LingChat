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

use super::service::{EmbeddingManager, cosine};

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
    /// 最近一次重建内容的签名（内容未变时跳过不必要的重建）。
    signature: RwLock<Option<u64>>,
    /// 为空的 fragment id 集合（语义检索时排除零碎文本）。
    min_chars: usize,
    top_k: usize,
}

impl MemoryIndex {
    pub fn new(manager: Arc<EmbeddingManager>) -> Self {
        Self {
            manager,
            fragments: RwLock::new(Vec::new()),
            signature: RwLock::new(None),
            min_chars: MIN_INDEX_CHARS,
            top_k: DEFAULT_TOP_K,
        }
    }

    /// 内容指纹：拼合（来源标签 + 文本），内容不变则指纹不变。
    fn signature_of(texts: &[(String, FragmentSource)]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (t, s) in texts {
            s.label().hash(&mut h);
            b"\x00".hash(&mut h);
            t.hash(&mut h);
            b"\x1e".hash(&mut h);
        }
        h.finish()
    }

    /// 若内容与上次重建不同则重建索引（避免每个对话轮次重复编码）。返回是否实际重建。
    pub async fn rebuild_if_changed(&self, texts: &[(String, FragmentSource)]) -> bool {
        let sig = Self::signature_of(texts);
        if *self.signature.read().await == Some(sig) {
            return false;
        }
        if self.rebuild(texts).await {
            *self.signature.write().await = Some(sig);
            true
        } else {
            // 编码失败：不推进指纹，下次内容变化（或恢复后）仍会重试重建。
            false
        }
    }

    /// 嵌入服务是否可用。
    pub fn enabled(&self) -> bool {
        self.manager.configured()
    }

    /// 重建索引：清空并以给定片段文本重建（用于角色记忆/笔记变更后批量刷新）。
    ///
    /// 返回是否成功；当嵌入不可用时返回 `false` 并保留旧索引，
    /// 避免一次瞬时失败把可用的旧语义索引清成空。
    pub async fn rebuild(&self, texts: &[(String, FragmentSource)]) -> bool {
        let usable: Vec<(String, FragmentSource)> = texts
            .iter()
            .filter(|(t, _)| t.chars().count() >= self.min_chars)
            .cloned()
            .collect();
        let encoded = self
            .manager
            .embed_passages(&usable.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>())
            .await;
        let Some(embedded) = encoded else {
            return false;
        };
        let mut fragments = Vec::new();
        for ((text, source), emb) in usable.iter().zip(embedded.into_iter()) {
            fragments.push(Fragment {
                id: fragment_id(&emb.text, source),
                source: source.clone(),
                text: text.clone(),
                vector: emb.vector,
            });
        }
        let mut guard = self.fragments.write().await;
        *guard = fragments;
        true
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
            if guard.iter().any(|f| f.id == id) {
                continue;
            }
            // 去重：与已有片段相似度 ≥ 阈值则跳过
            if guard
                .iter()
                .any(|f| cosine(&f.vector, &emb.vector) >= DUP_THRESHOLD)
            {
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
        *self.signature.write().await = None;
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
        if query.trim().is_empty() {
            return Vec::new();
        }
        let k = top_k.unwrap_or(self.top_k).min(20);
        let Some(query_vec) = self.manager.encode_queries(&[query.to_string()]).await else {
            return Vec::new();
        };
        let qv = &query_vec[0];
        let guard = self.fragments.read().await;
        if guard.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<SearchHit> = guard
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
        guard.iter().any(|f| cosine(&f.vector, v) >= threshold)
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
