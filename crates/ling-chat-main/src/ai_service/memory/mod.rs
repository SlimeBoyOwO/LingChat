//! 向量记忆服务：工具驱动的长期记忆（memory_store / vector_search / memory_delete）。
//!
//! 对标 BaiShou-Next 的记忆范式：
//! - 远程 OpenAI 兼容 embeddings 接口把文本变成 L2 归一化向量；
//! - 向量存进 `memory_embeddings` 表（Float32 小端字节）；
//! - 检索 = 内存余弦（向量路）+ LIKE 关键词（文本路），RRF 融合排序；
//! - 写入前三级去重：余弦 > 0.92 拒收，0.70–0.92 交给 LLM 判 merge/skip/new，
//!   LLM 失败时降级为直接存储。
//!
//! 配置（`embedding.*` / `memory.*` / `features.memory_enabled`）在启动时快照，
//! 修改后需重启生效，与 `api/settings.rs` 的保存提示一致。

pub mod embedding;
pub mod search;
pub mod store;

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use sea_orm::DatabaseConnection;
use serde_json::json;

use crate::ai_service::llm::{LlmClient, LlmSlot, slot_snapshot};
use crate::ai_service::types::LlmMessage;
use crate::config::AppConfig;

pub use embedding::EmbeddingClient;
pub use search::{HybridSearch, SearchHit, SearchMode};
pub use store::{MemoryRow, chunk_text};

/// 余弦相似度高于该值视为重复，直接拒收。
const DEDUP_REJECT_THRESHOLD: f32 = 0.92;
/// 余弦相似度落在 [0.70, 0.92] 时交给 LLM 判定是否合并。
const DEDUP_LLM_LOWER: f32 = 0.70;
/// 语义删除的最低相似度。
const DELETE_MIN_SCORE: f32 = 0.5;
/// 语义删除时考察的候选条数。
const DELETE_CANDIDATES: usize = 5;
/// 嵌入请求超时。
const EMBED_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// 一次 `store` 的结果。
pub struct StoreOutcome {
    /// `stored` / `merged` / `skipped`。
    pub action: String,
    /// 记忆 ID（`skipped` 时为 None）。
    pub memory_id: Option<String>,
    /// 人类可读的说明/内容预览。
    pub detail: String,
}

/// 去重判定结果。
enum DedupDecision {
    /// 与既有记忆过近，放弃写入。
    Skip { similarity: f32 },
    /// LLM 判定合并到 `memory_id`，用 `merged` 覆盖原记忆。
    Merge { memory_id: String, merged: String },
    /// 正常写入。
    Store,
}

/// 嵌入 + 向量存储 + 混合检索 + 去重的统一入口。
pub struct MemoryService {
    db: DatabaseConnection,
    llm: LlmSlot,
    embedder: EmbeddingClient,
    search: HybridSearch,
    enabled: bool,
    top_k: usize,
    min_score: f64,
}

impl MemoryService {
    /// 从启动时的配置快照构造。`llm` 是聊天 LLM 槽位，去重合并复用它。
    pub fn new(db: DatabaseConnection, llm: LlmSlot, cfg: &AppConfig) -> Self {
        Self {
            db,
            llm,
            embedder: EmbeddingClient::new(
                cfg.embedding_base_url.clone(),
                cfg.embedding_api_key.clone(),
                cfg.embedding_model.clone(),
            ),
            search: HybridSearch::default(),
            enabled: cfg.memory_enabled,
            top_k: cfg.memory_top_k as usize,
            min_score: cfg.memory_min_score,
        }
    }

    /// 记忆功能开关（`features.memory_enabled`）。
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 嵌入配置是否就绪（base_url + model 均非空）。
    pub fn is_configured(&self) -> bool {
        self.embedder.is_configured()
    }

    /// 当前配置的嵌入模型名（换模型后旧向量不参与检索）。
    pub fn model(&self) -> &str {
        self.embedder.model()
    }

    /// 默认检索参数（top_k / min_score），供工具调用时兜底。
    pub fn defaults(&self) -> (usize, f64) {
        (self.top_k, self.min_score)
    }

    /// 把一段文本写成向量记忆：去重 → 分块 → 逐块嵌入 → 落库。
    pub async fn store(
        &self,
        role_id: i32,
        content: &str,
        tags: &[String],
    ) -> Result<StoreOutcome> {
        let content = content.trim();
        if content.is_empty() {
            return Err(anyhow!("记忆内容不能为空"));
        }
        let tags_line = if tags.is_empty() {
            String::new()
        } else {
            format!("\n[标签: {}]", tags.join(","))
        };
        let full = format!("{content}{tags_line}");

        match self.dedup(role_id, &full).await? {
            DedupDecision::Skip { similarity } => Ok(StoreOutcome {
                action: "skipped".into(),
                memory_id: None,
                detail: format!("与既有记忆过于相似（相似度 {similarity:.3}），已放弃写入"),
            }),
            DedupDecision::Merge { memory_id, merged } => {
                store::delete_memory_scoped(&self.db, role_id, &memory_id).await?;
                let id = self.insert_memory(role_id, &merged, tags).await?;
                Ok(StoreOutcome {
                    action: "merged".into(),
                    memory_id: Some(id),
                    detail: format!("已合并进既有记忆：{merged}"),
                })
            },
            DedupDecision::Store => {
                let id = self.insert_memory(role_id, &full, tags).await?;
                let preview = if full.chars().count() > 100 {
                    let head: String = full.chars().take(100).collect();
                    format!("{head}…")
                } else {
                    full.clone()
                };
                Ok(StoreOutcome {
                    action: "stored".into(),
                    memory_id: Some(id),
                    detail: preview,
                })
            },
        }
    }

    /// 语义 + 关键词混合检索。
    pub async fn search(
        &self,
        role_id: i32,
        query: &str,
        mode: SearchMode,
        top_k: Option<usize>,
        min_score: Option<f64>,
        time_range: Option<(i64, i64)>,
    ) -> Result<Vec<SearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            return Err(anyhow!("搜索查询不能为空"));
        }
        let (default_top_k, default_min_score) = (self.top_k, self.min_score);
        let top_k = top_k.unwrap_or(default_top_k).clamp(1, 200);
        let min_score = min_score.unwrap_or(default_min_score).clamp(0.0, 1.0);

        let rows = store::load_role_rows(&self.db, role_id).await?;
        let query_vec = self.embedder.embed(query).await?;
        Ok(self
            .search
            .run(&query_vec, query, &rows, mode, top_k, min_score, time_range))
    }

    /// 按 `memory_id` 精确删除。
    pub async fn delete_by_id(&self, role_id: i32, memory_id: &str) -> Result<usize> {
        store::delete_memory_scoped(&self.db, role_id, memory_id).await
    }

    /// 语义删除：向量检索相似度 ≥ 0.5 的记忆并删除，返回被删条目的预览。
    pub async fn delete_by_query(&self, role_id: i32, query: &str) -> Result<Vec<(String, f32)>> {
        let rows = store::load_role_rows(&self.db, role_id).await?;
        let query_vec = self.embedder.embed(query).await?;
        let mut scored: Vec<(MemoryRow, f32)> = rows
            .into_iter()
            .filter(|row| row.embedding.len() == query_vec.len() && row.model == self.model())
            .map(|row| {
                let score = search::cosine(&query_vec, &row.embedding);
                (row, score)
            })
            .filter(|(_, score)| *score >= DELETE_MIN_SCORE)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(DELETE_CANDIDATES);

        let mut deleted = Vec::new();
        for (row, score) in scored {
            store::delete_memory(&self.db, &row.memory_id).await?;
            deleted.push((row.content, score));
        }
        Ok(deleted)
    }

    /// 逐块嵌入并写入一张记忆。
    async fn insert_memory(&self, role_id: i32, content: &str, tags: &[String]) -> Result<String> {
        let memory_id = uuid::Uuid::new_v4().to_string();
        let chunks = chunk_text(content);
        let embeddings = self.embedder.embed_batch(&chunks).await?;
        let tags_csv = if tags.is_empty() {
            None
        } else {
            Some(tags.join(","))
        };
        store::insert_memory(
            &self.db,
            role_id,
            &memory_id,
            &chunks,
            &embeddings,
            tags_csv,
            self.embedder.model(),
        )
        .await?;
        Ok(memory_id)
    }

    /// 三级去重（见模块文档）。
    async fn dedup(&self, role_id: i32, content: &str) -> Result<DedupDecision> {
        let Ok(query_vec) = self.embedder.embed(content).await else {
            // 嵌入失败不阻塞写入：去重是优化，不是硬门槛。
            return Ok(DedupDecision::Store);
        };
        let rows = store::load_role_rows(&self.db, role_id).await?;
        let mut scored: Vec<(MemoryRow, f32)> = rows
            .into_iter()
            .filter(|row| row.embedding.len() == query_vec.len() && row.model == self.model())
            .map(|row| {
                let score = search::cosine(&query_vec, &row.embedding);
                (row, score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let Some(best_score) = scored.first().map(|(_, score)| *score) else {
            return Ok(DedupDecision::Store);
        };
        if best_score > DEDUP_REJECT_THRESHOLD {
            return Ok(DedupDecision::Skip {
                similarity: best_score,
            });
        }
        if best_score < DEDUP_LLM_LOWER {
            return Ok(DedupDecision::Store);
        }

        // 中间地带：交给 LLM 判 merge / skip / new；失败则降级直接存。
        let candidates: Vec<(String, String, f32)> = scored
            .iter()
            .take(3)
            .map(|(row, score)| (row.memory_id.clone(), row.content.clone(), *score))
            .collect();
        match self.llm_dedup(content, &candidates).await {
            Ok(decision) => Ok(decision),
            Err(error) => {
                tracing::warn!("记忆去重判定失败，降级为直接存储: {error}");
                Ok(DedupDecision::Store)
            },
        }
    }

    /// 让 LLM 在 merge / skip / new 之间做判定。仅接受白名单内的 action。
    async fn llm_dedup(
        &self,
        new_content: &str,
        candidates: &[(String, String, f32)],
    ) -> Result<DedupDecision> {
        let client: Arc<LlmClient> = slot_snapshot(&self.llm)
            .await
            .context("未配置聊天 LLM，无法做记忆合并判定")?;

        let mut listing = String::new();
        for (index, (_, content, score)) in candidates.iter().enumerate() {
            listing.push_str(&format!("{}. 相似度 {score:.3}：{content}\n", index + 1));
        }
        let prompt = format!(
            "以下是要存入的新记忆，以及角色记忆库中已有的相近条目：\n\
             \n【新记忆】\n{new_content}\n【已有记忆】\n{listing}\n\
             判断三选一：\n\
             - merge：新记忆与第 N 条已有记忆说的是同一件事，应合并成一条更完整的记忆（用 target 指明 N）；\n\
             - skip：新记忆已被已有记忆覆盖，无须写入；\n\
             - new：两者是不同信息，照原样新存。\n\
             只输出一个 JSON 对象，不要输出其它文字：\n\
             {{\"action\":\"merge|skip|new\",\"target\":N,\"merged_content\":\"合并后的完整记忆（仅 merge 时给）\"}}"
        );
        let reply = client
            .complete(&[
                LlmMessage::system("你是记忆去重判定器，只输出合法 JSON。"),
                LlmMessage::user(prompt),
            ])
            .await?;

        let value: serde_json::Value = serde_json::from_str(reply.trim())
            .or_else(|_| extract_json(reply.trim()))
            .context("去重判定返回的不是合法 JSON")?;
        let action = value
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        match action {
            "skip" => Ok(DedupDecision::Skip {
                similarity: DEDUP_LLM_LOWER,
            }),
            "merge" => {
                let merged = value
                    .get("merged_content")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .context("merge 判定缺少 merged_content")?
                    .to_string();
                let target = value
                    .get("target")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .max(1) as usize;
                let memory_id = candidates
                    .get(target - 1)
                    .map(|(id, _, _)| id.clone())
                    .context("merge 判定的 target 越界")?;
                Ok(DedupDecision::Merge { memory_id, merged })
            },
            "new" => Ok(DedupDecision::Store),
            other => Err(anyhow!("去重判定 action 非法: {other}")),
        }
    }
}

/// 容忍模型把 JSON 包在 ```json 代码块或其它文字里，截取第一个 `{` 到最后一个 `}`。
fn extract_json(text: &str) -> Result<serde_json::Value> {
    let start = text.find('{').context("回复中没有 JSON")?;
    let end = text.rfind('}').context("回复中没有 JSON")?;
    serde_json::from_str(&text[start..=end]).context("JSON 解析失败")
}

/// 给前端/调试用的当前配置摘要。
pub fn describe(service: &MemoryService) -> serde_json::Value {
    let (top_k, min_score) = service.defaults();
    json!({
        "enabled": service.enabled(),
        "configured": service.is_configured(),
        "top_k": top_k,
        "min_score": min_score,
    })
}
