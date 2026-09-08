//! 独立语义记忆：与普通记忆库（MemoryBank）和手动笔记完全解耦的记忆系统。
//!
//! - 数据：写入一颗独立的 SQLite 向量库（`data/game_data/semantic_memory.db`），
//!   按 `role_id` 分库，向量持久化落盘（见 [`store`]）。
//! - 写入：AI 通过 `semantic_mem_add` 主动写入专属记忆（与 LLM 压缩的记忆库无关）。
//! - 检索/召回：`semantic_mem_search` 供 AI 检索；每轮对话自动召回相关片段
//!   注入 LLM 上下文（`recall`），不再重建 MemoryBank/笔记的内存索引。
//! - 引擎：与嵌入模块共享同一个 [`EmbeddingManager`]（Rust 原生 ONNX 推理）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::ai_service::embedding::service::cosine;
use crate::ai_service::embedding::EmbeddingManager;

use self::store::Store;

pub mod store;

/// 忽略的零碎文本阈值（过短不建库，避免噪音）。
const MIN_CHARS: usize = 2;
/// 去重判定阈值（余弦相似度高于此值视为重复）。
const DUP_THRESHOLD: f32 = 0.88;
/// 注入上下文时过滤低相关命中的阈值。
const RECALL_MIN_SCORE: f32 = 0.35;
/// 退出注入时的标题行。
const RECALL_TITLE: &str = "【语义召回的记忆】";

/// 新增结果。
#[derive(Debug, Clone)]
pub enum AddOutcome {
    /// 已新增，携带 id。
    Added(String),
    /// 与已有记忆语义重复（超过 0.88 余弦）而未新增。
    Duplicate,
}

/// 检索命中。
#[derive(Debug, Clone)]
pub struct Hit {
    pub id: String,
    pub text: String,
    pub score: f32,
}

/// 列表项（不含向量）。
#[derive(Debug, Clone)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

/// 独立语义记忆管理器。共享嵌入引擎 + 持久化向量库。
pub struct SemanticMemory {
    embedding: Option<Arc<EmbeddingManager>>,
    store: Store,
    top_k: usize,
    last_error: std::sync::Mutex<Option<String>>,
    db_path: PathBuf,
}

impl SemanticMemory {
    /// 打开向量库并构造管理器。
    ///
    /// - `db_path`：向量库文件（不存在则自动创建）。
    /// - `embedding`：嵌入引擎；`None` 时编码/检索不可用，但列表/删除仍可操作。
    /// - `top_k`：`recall`/`search` 的默认返回上限。
    pub async fn open(
        db_path: &Path,
        embedding: Option<Arc<EmbeddingManager>>,
        top_k: usize,
    ) -> Result<Self, String> {
        let store = Store::open(db_path).await?;
        Ok(Self {
            embedding,
            store,
            top_k: top_k.clamp(1, 20),
            last_error: std::sync::Mutex::new(None),
            db_path: db_path.to_path_buf(),
        })
    }

    /// 向量库文件路径。
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// 嵌入引擎是否就绪（模型文件齐备且可推理）。
    pub fn embedding_ready(&self) -> bool {
        self.embedding
            .as_ref()
            .map(|m| m.configured())
            .unwrap_or(false)
    }

    /// 最近一次操作失败诊断。
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    fn record_error(&self, message: impl Into<String>) {
        *self.last_error.lock().unwrap() = Some(message.into());
    }

    /// 新增一条语义记忆（去重后写入）。返回 `Duplicate` 表示与已有记忆重复。
    pub async fn add(
        &self,
        role_id: i32,
        text: &str,
        tags: &[String],
    ) -> Result<AddOutcome, String> {
        if text.trim().chars().count() < MIN_CHARS {
            return Err("语义记忆内容过短".to_string());
        }
        let Some(embedding) = self.embedding.as_ref() else {
            return Err("语义记忆未就绪：缺少嵌入引擎".to_string());
        };
        let Some(mut encoded) = embedding.embed_passages(&[text.to_string()]).await else {
            let msg = "语义记忆编码失败：嵌入模型不可用".to_string();
            self.record_error(&msg);
            return Err(msg);
        };
        let vector = std::mem::take(&mut encoded[0].vector);

        // 与该角色的既有记忆做语义去重
        let existing = self.store.fetch_role(role_id).await?;
        if existing
            .iter()
            .any(|m| cosine(&m.vector, &vector) >= DUP_THRESHOLD)
        {
            return Ok(AddOutcome::Duplicate);
        }

        let id = Uuid::new_v4().to_string();
        let ts = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        self.store
            .insert(&id, role_id, text.trim(), vector.len(), &vector, &tags_json, &ts)
            .await?;
        tracing::info!(
            "[semantic_memory] role_id={} 新增语义记忆 id={} len={}",
            role_id,
            id,
            text.chars().count()
        );
        Ok(AddOutcome::Added(id))
    }

    /// 语义检索：给定查询，返回按余弦降序排列的记忆。嵌入不可用时返回空列表。
    pub async fn search(
        &self,
        role_id: i32,
        query: &str,
        top_k: Option<usize>,
    ) -> Vec<Hit> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let Some(embedding) = self.embedding.as_ref() else {
            return Vec::new();
        };
        let Some(mut encoded) = embedding.encode_queries(&[query.to_string()]).await else {
            return Vec::new();
        };
        let qv = std::mem::take(&mut encoded[0]);

        let Ok(existing) = self.store.fetch_role(role_id).await else {
            return Vec::new();
        };
        if existing.is_empty() {
            return Vec::new();
        }
        let k = top_k.unwrap_or(self.top_k).clamp(1, 20);
        let mut scored: Vec<Hit> = existing
            .iter()
            .filter(|m| m.text.trim().chars().count() >= MIN_CHARS)
            .map(|m| Hit {
                id: m.id.clone(),
                text: m.text.clone(),
                score: cosine(&qv, &m.vector),
            })
            .collect();
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// 把检索命中格式化为注入 LLM 上下文的文本块（空命中/全部低于阈值返回空串）。
    pub fn format_hits(hits: &[Hit]) -> String {
        let mut lines: Vec<String> = hits
            .iter()
            .filter(|h| h.score >= RECALL_MIN_SCORE)
            .map(|h| format!("{:.0}% {}", h.score * 100.0, h.text))
            .collect();
        if lines.is_empty() {
            return String::new();
        }
        lines.insert(0, RECALL_TITLE.to_string());
        lines.join("\n")
    }

    /// 删除指定 id 的记忆。返回是否实际删除。
    pub async fn delete(&self, role_id: i32, id: &str) -> Result<bool, String> {
        self.store.delete(role_id, id).await
    }

    /// 按文本删除（兜底）。返回是否实际删除。
    pub async fn delete_by_text(&self, role_id: i32, text: &str) -> Result<bool, String> {
        self.store.delete_by_text(role_id, text).await
    }

    /// 列出某个角色的全部记忆（不含向量）。
    pub async fn list(&self, role_id: i32) -> Result<Vec<Item>, String> {
        let rows = self.store.fetch_role(role_id).await?;
        Ok(rows
            .into_iter()
            .map(|m| Item {
                id: m.id,
                text: m.text,
                tags: serde_json::from_str(&m.tags).unwrap_or_default(),
                created_at: m.created_at,
            })
            .collect())
    }

    /// 全局记忆总数。
    pub async fn count(&self) -> usize {
        self.store.count_all().await
    }

    /// 统计快照（供前端状态面板）。
    pub async fn snapshot(&self) -> (usize, bool) {
        (self.count().await, self.embedding_ready())
    }
}