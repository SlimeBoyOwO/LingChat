//! 向量记忆服务：长期记忆（memory_store / memory_delete / memory_summarize）。
//! 检索不暴露成工具：生成管线在用户新消息轮自动做 RAG 检索（[`MemoryService::search`]），
//! 命中结果落成 `LineAttribute::Memory` 台词行注入当轮上下文。
//!
//! 对标 BaiShou-Next 的记忆范式，并支持**无嵌入模型**运行（非嵌入式 RAG）：
//! - 配置了远程 OpenAI 兼容 embeddings 接口或自备本地模型时，文本变成 L2 归一化向量；
//! - **存储**：长期记忆写入每「剧本 × 角色」一张的共用归档表
//!   `memory_archive_{role_id}`（目录在 `memory_archive_profiles`，与旧版记忆整合
//!   归档共用同一套表，见 [`store`]）；没有嵌入模型时向量列为 NULL，只存正文；
//! - 检索 = 向量余弦（有模型时）+ 字符 n-gram 词法打分，RRF 融合排序；没有嵌入
//!   模型时全程走纯词法（见 [`search`]）；
//! - 写入前三级去重：相似度 > 0.92 拒收，0.70–0.92 交给 LLM 判 merge/skip/new，
//!   LLM 失败时降级为直接存储；无嵌入模型时用词法相似度（阈值相应调整）。
//!
//! 生命周期的「出口」是 [`MemoryService::summarize`]：把较早的一批记忆交给 LLM
//! 压缩成一条 `summary` 记忆，源条目随后删除。触发方式有三种——模型主动调用
//! `memory_summarize` 工具、写入后条数越过 [`AUTO_SUMMARIZE_THRESHOLD`] 的后台
//! 自动压缩、插件 `compress_context()` 的显式触发。
//!
//! 配置（`embedding.*` / `memory.*` / `features.memory_enabled`）在启动时快照，
//! 修改后需重启生效，与 `api/settings.rs` 的保存提示一致。例外：**本地嵌入模型的
//! 存在性与内容指纹是实时的**——[`MemoryService::is_configured`] 每次调用都做文件
//! 存在性检查，嵌入器缓存以文件指纹为键（[`local_embedding::model_fingerprint`]），
//! 因此在设置页「嵌入模型」子页导入 / 删除 / 更换本地模型后立即切换向量 / 词法检索，
//! 无需重启。

pub mod embedding;
pub mod local_embedding;
pub mod search;
pub mod store;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use sea_orm::DatabaseConnection;
use serde_json::json;

use crate::ai_service::llm::{LlmClient, LlmSlot, slot_snapshot};
use crate::ai_service::types::LlmMessage;
use crate::config::AppConfig;

pub use embedding::EmbeddingClient;
pub use local_embedding::{LOCAL_MODEL_NAME, LocalEmbedder};
pub use search::{HybridSearch, SearchHit, SearchMode};
pub use store::{MemoryRow, chunk_text};

/// 余弦相似度高于该值视为重复，直接拒收。
const DEDUP_REJECT_THRESHOLD: f32 = 0.92;
/// 余弦相似度落在 [0.70, 0.92] 时交给 LLM 判定是否合并。
const DEDUP_LLM_LOWER: f32 = 0.70;
/// 无嵌入模型时的词法去重：字符 n-gram 覆盖率高于该值视为重复，直接拒收。
const LEXICAL_DEDUP_REJECT: f32 = 0.85;
/// 无嵌入模型时的词法去重：覆盖率高于该值交给 LLM 判定是否合并。
const LEXICAL_DEDUP_LLM_LOWER: f32 = 0.45;
/// 纯词法模式下 `min_score` 的放宽系数：字符 n-gram 覆盖率天然低于语义余弦
/// （长查询尤其明显），直接套用同一阈值会大量漏召回，故按此系数缩小。
const LEXICAL_MIN_SCORE_SCALE: f64 = 0.5;
/// 语义删除的最低相似度。
const DELETE_MIN_SCORE: f32 = 0.5;
/// 语义删除时考察的候选条数。
const DELETE_CANDIDATES: usize = 5;
/// 嵌入请求超时。
const EMBED_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// 自动摘要阈值：一次写入后记忆条数达到该值，后台自动把较早的记忆压缩掉。
pub const AUTO_SUMMARIZE_THRESHOLD: usize = 64;
/// 摘要时默认保留的最新记忆条数（越新的越可能还被用到，不参与压缩）。
pub const DEFAULT_KEEP_RECENT: usize = 20;
/// 单次摘要至少要压缩的记忆条数，低于该值视为「没什么好整理的」。
const MIN_SUMMARIZE_ENTRIES: usize = 2;
/// 单次摘要喂给 LLM 的源记忆字符预算；超出的部分留到下一轮，避免提示词爆炸。
const SUMMARIZE_MAX_CHARS: usize = 24_000;
/// 自动/摘要生成的记忆打的标签。
const SUMMARY_TAG: &str = "summary";
/// 「旧版整合」条目向量回填时的单批嵌入条数（避免一次请求过大）。
const BACKFILL_BATCH: usize = 32;

/// 一次 `store` 的结果。
pub struct StoreOutcome {
    /// `stored` / `merged` / `skipped`。
    pub action: String,
    /// 记忆 ID（`skipped` 时为 None）。
    pub memory_id: Option<String>,
    /// 人类可读的说明/内容预览。
    pub detail: String,
}

/// 一次 `summarize` 的结果。
pub struct SummarizeOutcome {
    /// `summarized` / `noop`。
    pub action: String,
    /// 被压缩掉的源记忆条数。
    pub summarized: usize,
    /// 新摘要记忆的 ID（`noop` 时为 None）。
    pub memory_id: Option<String>,
    /// 摘要预览，或未执行的原因。
    pub detail: String,
}

/// 供 UI 浏览的一条长期记忆（同一 `memory_id` 的分块已聚合，标签已从正文剥离）。
#[derive(Clone, Debug, serde::Serialize)]
pub struct MemoryItem {
    pub memory_id: String,
    pub content: String,
    /// `store` 时传入的标签（正文末尾 `\n[标签: …]` 行拆出来的元数据）。
    pub tags: Vec<String>,
    /// 创建时间（Unix 毫秒）。
    pub created_at: i64,
}

/// 聚合成单条后的记忆（同一 `memory_id` 的分块按序拼接）。
struct MemoryEntry {
    memory_id: String,
    content: String,
    created_at: i64,
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

/// 本地嵌入器的缓存状态。
enum LocalCache {
    /// 尚未加载（第一次嵌入时才懒加载）。
    Empty,
    /// 已成功加载：`(模型指纹, 加载器)`。
    Ready(u64, Arc<LocalEmbedder>),
    /// 上次加载失败且指纹未变化（模型文件删除 / 替换后指纹变化自然失效重试）。
    Failed(u64, String),
}

/// 嵌入 + 向量存储 + 混合检索 + 去重 + 摘要的统一入口。
pub struct MemoryService {
    db: DatabaseConnection,
    llm: LlmSlot,
    /// 远程 OpenAI 兼容 embeddings 客户端（配置了 `embedding.*` 时启用，优先于本地模型）。
    remote: Option<EmbeddingClient>,
    /// 可选本地嵌入模型（懒加载：第一次嵌入时才读表 + 建 ONNX Session）。
    /// 缓存带上模型指纹（[`local_embedding::model_fingerprint`]），文件被删除 / 替换后
    /// 指纹变化会自动重新加载，使设置页的导入 / 删除**立即生效**（无需重启）。
    /// 失败也进缓存（见 [`LocalCache::Failed`]）：模型文件齐备但加载不了的场景下，
    /// 后续嵌入快速失败而非反复做几百毫秒的完整加载；`is_configured` / `retrieval_mode`
    /// 同步读取该状态，如实降级为词法检索而非误报 hybrid。
    local: Mutex<LocalCache>,
    search: HybridSearch,
    enabled: bool,
    top_k: usize,
    min_score: f64,
    /// 摘要互斥锁：压缩要跨「LLM 生成 + 删源条目」，并发跑会产出重复摘要。
    summarize_lock: tokio::sync::Mutex<()>,
    /// 正在跑后台自动摘要的角色集合：按角色单独限流，避免连续写入堆起一串压缩任务
    /// （不同角色可各跑一个，互不阻塞）。
    auto_summarize_running: Mutex<HashSet<i32>>,
}

impl MemoryService {
    /// 从启动时的配置快照构造。`llm` 是聊天 LLM 槽位，去重合并与摘要复用它。
    pub fn new(db: DatabaseConnection, llm: LlmSlot, cfg: &AppConfig) -> Self {
        // 显式配置了远程嵌入时优先用远程；否则回落到本地量化模型（若自备）。
        let remote_configured =
            !cfg.embedding_base_url.trim().is_empty() && !cfg.embedding_model.trim().is_empty();
        let remote = remote_configured.then(|| {
            EmbeddingClient::new(
                cfg.embedding_base_url.clone(),
                cfg.embedding_api_key.clone(),
                cfg.embedding_model.clone(),
            )
        });
        let local_available = local_embedding::is_model_available();
        if remote_configured {
            tracing::info!("记忆嵌入使用远程接口: model={}", cfg.embedding_model);
        } else if local_available {
            tracing::info!("记忆嵌入使用本地模型: {}", LOCAL_MODEL_NAME);
        } else {
            tracing::info!(
                "记忆嵌入未配置：既无远程 embedding 配置，也缺少本地模型文件；检索回退为字符 n-gram 词法模式（非嵌入式 RAG）"
            );
        }
        Self {
            db,
            llm,
            remote,
            local: Mutex::new(LocalCache::Empty),
            search: HybridSearch::default(),
            enabled: cfg.memory_enabled,
            top_k: cfg.memory_top_k as usize,
            min_score: cfg.memory_min_score,
            summarize_lock: tokio::sync::Mutex::new(()),
            auto_summarize_running: Mutex::new(HashSet::new()),
        }
    }

    /// 记忆功能开关（`features.memory_enabled`）。
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 是否配置了远程 embeddings 接口（设置页提示用；本地模型存在与否与之无关）。
    pub fn remote_configured(&self) -> bool {
        self.remote.is_some()
    }

    /// 嵌入配置是否就绪：远程已配置，或本地模型文件齐备且（若已尝试过）能成功加载。
    ///
    /// 注意：这只决定「是否走向量路」；记忆功能本身**不依赖**它——未就绪时检索与
    /// 写入自动退化为字符 n-gram 词法模式（非嵌入式 RAG）。本地模型的存在性是**实时**
    /// 文件检查，因此在设置页导入 / 删除模型后立即生效，无需重启。若本地模型文件齐备
    /// 但曾加载失败（产物损坏 / ONNX Runtime 缺失），会在首次失败后如实返回 `false`，
    /// 与设置页展示的 `load_error` 口径一致，直到文件指纹变化后重试。
    pub fn is_configured(&self) -> bool {
        if self.remote.is_some() {
            return true;
        }
        let Some(fingerprint) = local_embedding::model_fingerprint() else {
            return false;
        };
        let cache = self.local.lock().unwrap_or_else(|e| e.into_inner());
        // 已缓存「加载失败且指纹未变」→ 如实视为未配置；其余（空缓存 / 加载成功 /
        // 指纹已变化）按文件存在性判定。
        match &*cache {
            LocalCache::Failed(cached, _) if *cached == fingerprint => false,
            _ => true,
        }
    }

    /// 当前使用的嵌入模型名（换模型后旧向量不参与检索）。
    pub fn model(&self) -> &str {
        match &self.remote {
            Some(remote) => remote.model(),
            None => LOCAL_MODEL_NAME,
        }
    }

    /// 当前检索模式：配置了嵌入模型走 `hybrid`（向量 + 词法），否则纯词法 `lexical`
    /// （非嵌入式 RAG）。
    pub fn retrieval_mode(&self) -> &'static str {
        if self.is_configured() {
            "hybrid"
        } else {
            "lexical"
        }
    }

    /// 取到可用的本地嵌入器（首次调用时在阻塞线程池里懒加载）。
    ///
    /// 缓存以模型文件指纹为键：指纹一致时复用已加载的 `LocalEmbedder`；文件被
    /// 删除 / 替换（指纹变化）时重新加载，因此设置页导入 / 删除后立即生效。
    /// 加载失败也会按指纹缓存原因：指纹不变时后续调用**快速失败**，不会每次重复
    /// 做几百毫秒的完整加载（坏模型 / ONNX Runtime 缺失场景）。
    async fn ensure_local(&self) -> Result<Arc<LocalEmbedder>> {
        let fingerprint =
            local_embedding::model_fingerprint().context("本地嵌入模型文件缺失或不可读")?;
        {
            let cache = self.local.lock().unwrap_or_else(|e| e.into_inner());
            match &*cache {
                LocalCache::Ready(cached, embedder) if *cached == fingerprint => {
                    return Ok(Arc::clone(embedder));
                },
                LocalCache::Failed(cached, reason) if *cached == fingerprint => {
                    return Err(anyhow!("本地嵌入模型加载失败: {reason}"));
                },
                _ => {},
            }
        }
        let dir = local_embedding::model_dir();
        match tokio::task::spawn_blocking(move || LocalEmbedder::load(&dir)).await {
            Ok(Ok(embedder)) => {
                let embedder = Arc::new(embedder);
                let mut cache = self.local.lock().unwrap_or_else(|e| e.into_inner());
                *cache = LocalCache::Ready(fingerprint, Arc::clone(&embedder));
                Ok(embedder)
            },
            Ok(Err(error)) => {
                let reason = format!("{error:#}");
                let mut cache = self.local.lock().unwrap_or_else(|e| e.into_inner());
                *cache = LocalCache::Failed(fingerprint, reason.clone());
                Err(anyhow!("本地嵌入模型加载失败: {reason}"))
            },
            Err(error) => Err(anyhow!("本地嵌入模型加载线程失败: {error}")),
        }
    }

    /// 探测本地嵌入模型是否**真的可用**（不仅文件存在，还能加载）。
    ///
    /// 供设置页「嵌入模型」子页的状态展示、以及旧版记忆向量回填的门禁使用：
    /// 已配置远程接口时视为可用；否则文件齐备时尝试加载（成功会顺带暖缓存，
    /// 失败会被 [`ensure_local`](Self::ensure_local) 按指纹缓存），
    /// 返回 `Err(原因)` 表示只能走词法。
    pub async fn probe_local_embedder(&self) -> Result<(), String> {
        if self.remote.is_some() {
            return Ok(());
        }
        if !local_embedding::is_model_available() {
            return Err("本地嵌入模型未安装".to_string());
        }
        self.ensure_local()
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 嵌入一段文本：远程优先，否则用本地模型。
    async fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        if let Some(remote) = &self.remote {
            return remote.embed(text).await;
        }
        self.ensure_local().await?.embed(text).await
    }

    /// 嵌入多段文本：远程优先，否则用本地模型。
    async fn embed_many(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if let Some(remote) = &self.remote {
            return remote.embed_batch(texts).await;
        }
        self.ensure_local().await?.embed_batch(texts).await
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
        // 标签拼成正文尾行 `\n[标签: …]`；`insert_memory` 会用同一函数重建，二者一致。
        let full = with_tags_suffix(content, tags);

        // 去重判定需要一次嵌入。这里先把 `full` 的向量算出来：判定用它，若走到「新存」
        // 分支还能把同一文本的向量复用给 `insert_memory`，避免同一条内容被嵌入两次
        // （远程接口下即少一次 HTTP 往返）。嵌入失败则退回词法去重，写入会自行再试。
        let query_vec = if self.is_configured() {
            match self.embed_one(&full).await {
                Ok(vector) => Some(vector),
                Err(error) => {
                    tracing::warn!("记忆去重嵌入失败，降级为词法去重: {error}");
                    None
                },
            }
        } else {
            None
        };

        match self.dedup(role_id, &full, query_vec.as_deref()).await? {
            DedupDecision::Skip { similarity } => Ok(StoreOutcome {
                action: "skipped".into(),
                memory_id: None,
                detail: format!("与既有记忆过于相似（相似度 {similarity:.3}），已放弃写入"),
            }),
            DedupDecision::Merge { memory_id, merged } => {
                // 先插后删：合并结果（可能含远程嵌入）成功落库后才删旧记忆，插入失败
                // 不会丢数据（与 `summarize` 的「先插后删」一致）。合并正文里可能已带上
                // 旧标签行（来自候选列表），先剥掉再由 `insert_memory` 按本次标签重建。
                let id = self
                    .insert_memory(role_id, strip_tags_suffix(&merged), tags, None)
                    .await?;
                if let Err(error) = store::delete_memory_scoped(&self.db, role_id, &memory_id).await
                {
                    tracing::warn!("记忆合并：旧记忆 {memory_id} 删除失败，可能残留重复: {error}");
                }
                Ok(StoreOutcome {
                    action: "merged".into(),
                    memory_id: Some(id),
                    detail: format!("已合并进既有记忆：{merged}"),
                })
            },
            DedupDecision::Store => {
                let id = self
                    .insert_memory(role_id, content, tags, query_vec.as_deref())
                    .await?;
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

    /// 语义 + 关键词混合检索（长期记忆）。生成管线每轮 RAG 注入走这里。
    ///
    /// 只召回 `kind='memory'` 的长期记忆；旧版整合条目由 [`Self::query`] /
    /// `memory_query` 工具按需检索（不参与自动注入）。
    pub async fn search(
        &self,
        role_id: i32,
        query: &str,
        mode: SearchMode,
        top_k: Option<usize>,
        min_score: Option<f64>,
        time_range: Option<(i64, i64)>,
    ) -> Result<Vec<SearchHit>> {
        let rows = store::load_role_rows(&self.db, role_id).await?;
        self.search_rows(rows, query, mode, top_k, min_score, time_range)
            .await
    }

    /// 供 `memory_query` 工具：在「长期记忆 + 已带向量的旧版整合条目」上做混合检索。
    pub async fn query(
        &self,
        role_id: i32,
        query: &str,
        mode: SearchMode,
        top_k: Option<usize>,
        min_score: Option<f64>,
        time_range: Option<(i64, i64)>,
    ) -> Result<Vec<SearchHit>> {
        let rows = store::load_role_search_rows(&self.db, role_id).await?;
        self.search_rows(rows, query, mode, top_k, min_score, time_range)
            .await
    }

    /// 在给定候选行上做「嵌入 query → 混合检索」的公共逻辑。
    async fn search_rows(
        &self,
        rows: Vec<MemoryRow>,
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

        // 有嵌入模型 → 按请求模式（Hybrid/Vector）走向量路；未配置或嵌入失败 →
        // 退化为纯词法（字符 n-gram）检索，保证「非嵌入式 RAG」始终可用。
        let (query_vec, mode) = if self.is_configured() {
            match self.embed_one(query).await {
                Ok(vector) => (vector, mode),
                Err(error) => {
                    tracing::warn!("记忆检索嵌入失败，降级为纯词法检索: {error}");
                    (Vec::new(), SearchMode::Lexical)
                },
            }
        } else {
            (Vec::new(), SearchMode::Lexical)
        };
        // 词法覆盖率天然低于语义余弦，纯词法模式下按系数放宽最低分阈值。
        let min_score = if mode == SearchMode::Lexical {
            min_score * LEXICAL_MIN_SCORE_SCALE
        } else {
            min_score
        };
        Ok(self
            .search
            .run(&query_vec, query, &rows, mode, top_k, min_score, time_range))
    }

    /// 为「旧版整合」条目补算向量，使其能被 `memory_query` 语义检索命中。
    ///
    /// `role_id = None` 扫描全部角色（启动时用），`Some` 只补指定角色（导入后用）。
    /// 幂等：只处理还没有向量的行；嵌入配置未就绪时返回 0。分批嵌入，单批失败
    /// 记警告跳过，不因一批失败丢掉整次回填。返回补算成功的条数。
    pub async fn backfill_legacy_embeddings(&self, role_id: Option<i32>) -> Result<usize> {
        // 远程已配置直接用；本地模型做一次「真可用」探测（结果与失败原因进缓存）：
        // 文件齐备但模型加载不了的场景下直接跳过，避免对每条待回填记录逐批刷新失败日志。
        let configured = if self.remote.is_some() {
            true
        } else {
            self.probe_local_embedder().await.is_ok()
        };
        if !configured {
            return Ok(0);
        }
        let pending = store::load_legacy_pending_embeddings(&self.db, role_id).await?;
        if pending.is_empty() {
            return Ok(0);
        }
        let total = pending.len();
        let mut saved = 0usize;
        for batch in pending.chunks(BACKFILL_BATCH) {
            let texts: Vec<String> = batch.iter().map(|p| p.content.clone()).collect();
            let embeddings = match self.embed_many(&texts).await {
                Ok(v) if v.len() == batch.len() => v,
                Ok(v) => {
                    tracing::warn!(
                        "旧版整合记忆回填：嵌入数量不匹配（{} != {}），本批跳过",
                        v.len(),
                        batch.len()
                    );
                    continue;
                },
                Err(e) => {
                    tracing::warn!("旧版整合记忆回填：嵌入失败，本批跳过: {e}");
                    continue;
                },
            };
            for (p, vector) in batch.iter().zip(embeddings.iter()) {
                if let Err(e) =
                    store::save_legacy_embedding(&self.db, &p.table, p.row_id, vector, self.model())
                        .await
                {
                    tracing::warn!("旧版整合记忆回填：写入向量失败 row={}: {e}", p.row_id);
                    continue;
                }
                saved += 1;
            }
        }
        if saved > 0 {
            tracing::info!("旧版整合记忆回填完成：{saved}/{total} 条已补算向量");
        }
        Ok(saved)
    }

    /// 按 `memory_id` 精确删除。
    ///
    /// 浏览面板里旧版整合条目以 `legacy_{行id}` 作为展示用 id，这里识别该前缀后
    /// 改为按行删除单条整合条目；其余按 `kind='memory'` 分块整体删除。
    pub async fn delete_by_id(&self, role_id: i32, memory_id: &str) -> Result<usize> {
        if let Some(id_str) = memory_id.strip_prefix("legacy_") {
            if let Ok(row_id) = id_str.parse::<i32>() {
                return store::delete_legacy_row(&self.db, role_id, row_id).await;
            }
        }
        store::delete_memory_scoped(&self.db, role_id, memory_id).await
    }

    /// 语义删除：向量检索相似度 ≥ 0.5 的记忆并删除，返回被删条目的预览。
    ///
    /// 无嵌入模型（或嵌入失败）时退化为字符 n-gram 词法匹配，阈值按词法系数（0.5）放宽。
    pub async fn delete_by_query(&self, role_id: i32, query: &str) -> Result<Vec<(String, f32)>> {
        let rows = store::load_role_rows(&self.db, role_id).await?;
        let query = query.trim();
        if query.is_empty() {
            return Err(anyhow!("删除查询不能为空"));
        }
        let mut used_vector = false;
        let mut scored: Vec<(MemoryRow, f32)> = if self.is_configured() {
            match self.embed_one(query).await {
                Ok(query_vec) => {
                    used_vector = true;
                    score_rows_by_vector(rows, &query_vec, self.model())
                },
                Err(error) => {
                    tracing::warn!("语义删除嵌入失败，降级为词法匹配: {error}");
                    lexical_pairs(rows, query)
                },
            }
        } else {
            lexical_pairs(rows, query)
        };
        // 词法覆盖率天然低于余弦，词法模式下同样放宽最低分。
        let min_score = if used_vector {
            DELETE_MIN_SCORE
        } else {
            DELETE_MIN_SCORE * LEXICAL_MIN_SCORE_SCALE as f32
        };
        scored.retain(|(_, score)| *score >= min_score);
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(DELETE_CANDIDATES);

        let mut deleted = Vec::new();
        for (row, score) in scored {
            store::delete_memory_scoped(&self.db, role_id, &row.memory_id).await?;
            deleted.push((row.content, score));
        }
        Ok(deleted)
    }

    /// 某角色当前的记忆条数（按 `memory_id` 去重，分块不重复计）。
    pub async fn count_memories(&self, role_id: i32) -> Result<usize> {
        store::count_role_memories(&self.db, role_id).await
    }

    /// 列出某角色的全部长期记忆（最新在前），供设置页「记忆系统」浏览面板使用。
    ///
    /// 统一视图：长期记忆（`kind='memory'`，按 `memory_id` 聚合分块）+ 旧版整合
    /// 条目（`kind='legacy'`，逐行一条，`memory_id` 展示为 `legacy_{行id}`）。
    /// 只读库表：不依赖嵌入配置，`memory_enabled` 关闭时也能查看和清理已存内容。
    pub async fn list_memories(&self, role_id: i32) -> Result<Vec<MemoryItem>> {
        let mut items: Vec<MemoryItem> = {
            let entries = group_entries(store::load_role_rows(&self.db, role_id).await?);
            entries
                .into_iter()
                .map(|entry| {
                    let (content, tags) = split_tags_suffix(&entry.content);
                    MemoryItem {
                        memory_id: entry.memory_id,
                        content: content.to_string(),
                        tags,
                        created_at: entry.created_at,
                    }
                })
                .collect()
        };
        // 追加旧版整合条目（浏览面板共用一张表，两类条目一并可见）。
        for legacy in store::load_role_legacy_rows(&self.db, role_id).await? {
            items.push(MemoryItem {
                memory_id: format!("legacy_{}", legacy.id),
                content: legacy.content,
                tags: Vec::new(),
                created_at: legacy.created_at,
            });
        }
        items.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then(a.memory_id.cmp(&b.memory_id))
        });
        Ok(items)
    }

    /// 摘要：把较早的一批记忆压缩成一条 `summary` 记忆，随后删除源条目。
    ///
    /// `keep_recent` 条最新的记忆保持原样（越新越可能还被用到）；更早的按时间从旧
    /// 到新挑选，直到 [`SUMMARIZE_MAX_CHARS`] 字符预算用完，剩下的留到下一轮。
    /// 全程持 [`summarize_lock`](Self::summarize_lock)，并发调用会串行执行，
    /// 后到者重新取快照后通常已无可压缩内容，返回 `noop`。
    ///
    /// 顺序是「先插摘要、再删源条目」：插入（含远程嵌入）失败时源记忆原样保留，
    /// 不会因为压缩出错丢数据。
    pub async fn summarize(&self, role_id: i32, keep_recent: usize) -> Result<SummarizeOutcome> {
        let _serialized = self.summarize_lock.lock().await;

        let rows = store::load_role_rows(&self.db, role_id).await?;
        let mut entries = group_entries(rows);
        if entries.is_empty() {
            return Ok(noop_outcome("记忆库为空，无需摘要".to_string()));
        }
        // 最新在前，方便按 keep_recent 切出「保留区」；keep_recent = 0 表示全部参与压缩。
        entries.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then(a.memory_id.cmp(&b.memory_id))
        });
        let keep = keep_recent.min(entries.len());
        let mut candidates = entries.split_off(keep);
        if candidates.len() < MIN_SUMMARIZE_ENTRIES {
            return Ok(noop_outcome(format!(
                "保留最新 {} 条后只剩 {} 条较早记忆，未达到摘要门槛 {MIN_SUMMARIZE_ENTRIES} 条，无需整理",
                keep,
                candidates.len()
            )));
        }

        // 从最旧的开始挑；至少压缩 MIN_SUMMARIZE_ENTRIES 条（候选数已由上一道闸保证），
        // 其余按字符预算取，超预算的部分留到下一轮。
        candidates.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let mut selected = Vec::new();
        let mut budget = 0usize;
        for entry in candidates {
            let len = entry.content.chars().count();
            if selected.len() >= MIN_SUMMARIZE_ENTRIES && budget + len > SUMMARIZE_MAX_CHARS {
                break;
            }
            budget += len;
            selected.push(entry);
            if budget >= SUMMARIZE_MAX_CHARS && selected.len() >= MIN_SUMMARIZE_ENTRIES {
                break;
            }
        }

        let source_count = selected.len();
        let summary = self.llm_summarize(&selected).await?;
        // 先插后删：这一步失败只是没压成，不能反过来先删了源条目。
        let memory_id = self
            .insert_memory(role_id, &summary, &[SUMMARY_TAG.to_string()], None)
            .await?;
        for entry in &selected {
            store::delete_memory_scoped(&self.db, role_id, &entry.memory_id).await?;
        }

        let preview: String = summary.chars().take(100).collect();
        Ok(SummarizeOutcome {
            action: "summarized".into(),
            summarized: source_count,
            memory_id: Some(memory_id),
            detail: format!("{preview}…"),
        })
    }

    /// 尝试为 `role_id` 占用「后台自动摘要」槽位；该角色已有任务在跑时返回 false。
    fn try_begin_auto_summarize(&self, role_id: i32) -> bool {
        let mut running = self
            .auto_summarize_running
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        running.insert(role_id)
    }

    /// 释放 `role_id` 的自动摘要槽位。
    fn end_auto_summarize(&self, role_id: i32) {
        let mut running = self
            .auto_summarize_running
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        running.remove(&role_id);
    }

    /// 让 LLM 把一批较早的记忆压缩成一条摘要正文。
    async fn llm_summarize(&self, entries: &[MemoryEntry]) -> Result<String> {
        let client: Arc<LlmClient> = slot_snapshot(&self.llm)
            .await
            .context("未配置聊天 LLM，无法生成记忆摘要")?;

        let mut listing = String::new();
        for (index, entry) in entries.iter().enumerate() {
            listing.push_str(&format!(
                "{}. {}\n",
                index + 1,
                strip_tags_suffix(&entry.content)
            ));
        }
        let prompt = format!(
            "以下是同一个角色记忆库中较早的一批记忆：\n\
             \n{listing}\n\
             请把它们压缩成一条连贯的摘要：\n\
             - 保留全部关键事实（人物关系、约定、承诺、偏好、重要事件与时间）；\n\
             - 去掉重复与无关细节，互相矛盾的表述保留较新的说法；\n\
             - 不要添加原文没有的信息，不要解释你做了什么；\n\
             - 直接输出摘要正文（可分多行），不要输出 JSON、序号或标题。"
        );
        let reply = client
            .complete(&[
                LlmMessage::system("你是长期记忆压缩器，只输出压缩后的摘要正文。"),
                LlmMessage::user(prompt),
            ])
            .await?;
        let summary = reply.trim();
        if summary.is_empty() {
            return Err(anyhow!("LLM 返回了空摘要"));
        }
        Ok(summary.to_string())
    }

    /// 逐块嵌入并写入一张记忆。
    ///
    /// `tags` 统一在这里拼成正文尾行 `\n[标签: …]`（写入方只管正文），写入 / 检索 /
    /// 浏览共用同一种编码，`split_tags_suffix` 再从尾行还原标签。
    ///
    /// 无嵌入模型（或嵌入失败/数量不匹配）时改为写纯文本记忆（向量列为 NULL），
    /// 检索自动走词法——写入永远不该因为嵌入不可用而失败。
    ///
    /// `known_vector` 是调用方已算好的**整条内容**的向量（去重时的结果）：仅当内容
    /// 只有一个分块（≤1024 字，绝大多数）时直接复用，省掉一次重复嵌入。
    async fn insert_memory(
        &self,
        role_id: i32,
        content: &str,
        tags: &[String],
        known_vector: Option<&[f32]>,
    ) -> Result<String> {
        let memory_id = uuid::Uuid::new_v4().to_string();
        let tagged = with_tags_suffix(content, tags);
        let chunks = chunk_text(&tagged);
        let embeddings = match known_vector {
            Some(vector) if chunks.len() == 1 => vec![vector.to_vec()],
            _ => self.embed_chunks(&chunks).await,
        };
        store::insert_memory(
            &self.db,
            role_id,
            &memory_id,
            &chunks,
            &embeddings,
            self.model(),
        )
        .await?;
        Ok(memory_id)
    }

    /// 逐块嵌入。未配置嵌入模型（或嵌入失败 / 数量不匹配）时返回与分块等长的空向量，
    /// 该记忆改存纯文本，检索自动走字符 n-gram 词法路。
    async fn embed_chunks(&self, chunks: &[String]) -> Vec<Vec<f32>> {
        if !self.is_configured() || chunks.is_empty() {
            return vec![Vec::new(); chunks.len()];
        }
        match self.embed_many(chunks).await {
            Ok(vectors) if vectors.len() == chunks.len() => vectors,
            Ok(vectors) => {
                tracing::warn!(
                    "记忆嵌入数量不匹配（{} != {}），该条改存纯文本",
                    vectors.len(),
                    chunks.len()
                );
                vec![Vec::new(); chunks.len()]
            },
            Err(error) => {
                tracing::warn!("记忆嵌入失败，改存纯文本记忆: {error}");
                vec![Vec::new(); chunks.len()]
            },
        }
    }

    /// 三级去重（见模块文档）。
    ///
    /// `query_vec` 是调用方按 `content` 预先算好的向量（`store` 会把它复用给写入，
    /// 避免同一条内容重复嵌入）：`Some` 走向量余弦，`None`（未配置嵌入或嵌入失败）
    /// 退化为字符 n-gram 词法相似度，并使用相应的阈值。
    async fn dedup(
        &self,
        role_id: i32,
        content: &str,
        query_vec: Option<&[f32]>,
    ) -> Result<DedupDecision> {
        let rows = store::load_role_rows(&self.db, role_id).await?;
        let (mut scored, used_vector): (Vec<(MemoryRow, f32)>, bool) = match query_vec {
            Some(vector) => (score_rows_by_vector(rows, vector, self.model()), true),
            None => (lexical_pairs(rows, content), false),
        };
        let (reject_threshold, llm_lower) = if used_vector {
            (DEDUP_REJECT_THRESHOLD, DEDUP_LLM_LOWER)
        } else {
            (LEXICAL_DEDUP_REJECT, LEXICAL_DEDUP_LLM_LOWER)
        };
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let Some(best_score) = scored.first().map(|(_, score)| *score) else {
            return Ok(DedupDecision::Store);
        };
        if best_score > reject_threshold {
            return Ok(DedupDecision::Skip {
                similarity: best_score,
            });
        }
        if best_score < llm_lower {
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
                // 回传命中的最高相似度（候选已按分数降序），别拿阈值下限冒充。
                similarity: candidates
                    .first()
                    .map(|(_, _, score)| *score)
                    .unwrap_or(DEDUP_LLM_LOWER),
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

/// 向量打分：过滤出与查询向量同维、同模型的行，逐行算余弦。
fn score_rows_by_vector(
    rows: Vec<MemoryRow>,
    query_vec: &[f32],
    model: &str,
) -> Vec<(MemoryRow, f32)> {
    rows.into_iter()
        .filter(|row| row.embedding.len() == query_vec.len() && row.model == model)
        .map(|row| {
            let score = search::cosine(query_vec, &row.embedding);
            (row, score)
        })
        .collect()
}

/// 词法打分：无嵌入模型时对每行做字符 n-gram 覆盖率打分（不依赖任何模型）。
fn lexical_pairs(rows: Vec<MemoryRow>, query: &str) -> Vec<(MemoryRow, f32)> {
    let refs: Vec<&MemoryRow> = rows.iter().collect();
    let scores = search::lexical_scores(query, &refs);
    rows.into_iter()
        .zip(scores)
        .map(|(row, score)| (row, score as f32))
        .collect()
}

/// 把按 `memory_id` 分组的分块行聚合成单条记忆（分块按 `chunk_index` 拼接）。
fn group_entries(rows: Vec<MemoryRow>) -> Vec<MemoryEntry> {
    let mut ordered = rows;
    ordered.sort_by(|a, b| {
        a.memory_id
            .cmp(&b.memory_id)
            .then(a.chunk_index.cmp(&b.chunk_index))
    });

    let mut entries: Vec<MemoryEntry> = Vec::new();
    for row in ordered {
        match entries.last_mut() {
            Some(last) if last.memory_id == row.memory_id => {
                last.content.push_str(&row.content);
                last.created_at = last.created_at.min(row.created_at);
            },
            _ => entries.push(MemoryEntry {
                memory_id: row.memory_id,
                content: row.content,
                created_at: row.created_at,
            }),
        }
    }
    entries
}

/// 把标签拼成正文尾行 `\n[标签: a,b]`；无标签时原样返回。
///
/// 与 [`split_tags_suffix`] 互逆——写入（`store` / 合并 / 摘要）与读取（浏览面板、
/// 摘要前剥离标签）共用这一编码，避免各处各拼一份导致不一致。
fn with_tags_suffix(content: &str, tags: &[String]) -> String {
    if tags.is_empty() {
        content.to_string()
    } else {
        format!("{content}\n[标签: {}]", tags.join(","))
    }
}

/// `store` 会把标签拼进正文末尾（`\n[标签: a,b]`）；拆成「正文 + 标签列表」两部分。
/// 只有当它是最后一行（其后不再换行）时才拆，避免误伤正文里提到的同名片段。
fn split_tags_suffix(content: &str) -> (&str, Vec<String>) {
    let Some(index) = content.rfind("\n[标签: ") else {
        return (content, Vec::new());
    };
    if content[index + 1..].contains('\n') {
        return (content, Vec::new());
    }
    let line = &content[index + 1..]; // "[标签: a,b]"
    match line
        .strip_prefix("[标签: ")
        .and_then(|s| s.strip_suffix(']'))
    {
        Some(inner) => (
            &content[..index],
            inner
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        ),
        None => (content, Vec::new()),
    }
}

/// [`split_tags_suffix`] 的正文部分：喂给摘要 LLM 前剥掉标签行，
/// 标签只作为元数据存在，不该占摘要的篇幅。
fn strip_tags_suffix(content: &str) -> &str {
    split_tags_suffix(content).0
}

/// 构造 `noop` 结果。
fn noop_outcome(detail: String) -> SummarizeOutcome {
    SummarizeOutcome {
        action: "noop".into(),
        summarized: 0,
        memory_id: None,
        detail,
    }
}

/// 写入后的自动摘要闸门：记忆条数越过 [`AUTO_SUMMARIZE_THRESHOLD`] 且当前没有
/// 摘要任务在跑时，后台把较早的记忆压缩成一条，不阻塞调用方。
///
/// 每次写入调用一次，成本只是一次计数查询；计数与压缩都在后台任务里完成。
pub fn spawn_auto_summarize_if_needed(memory: Arc<MemoryService>, role_id: i32) {
    tokio::spawn(async move {
        let count = match memory.count_memories(role_id).await {
            Ok(count) => count,
            Err(error) => {
                tracing::warn!("自动记忆摘要：计数失败: {error}");
                return;
            },
        };
        if count < AUTO_SUMMARIZE_THRESHOLD {
            return;
        }
        if !memory.try_begin_auto_summarize(role_id) {
            return;
        }
        let outcome = memory.summarize(role_id, DEFAULT_KEEP_RECENT).await;
        memory.end_auto_summarize(role_id);
        match outcome {
            Ok(result) if result.action == "summarized" => tracing::info!(
                role_id,
                summarized = result.summarized,
                total_after = count.saturating_sub(result.summarized),
                "记忆条数越过阈值，后台自动摘要完成"
            ),
            Ok(_) => tracing::debug!(role_id, "自动摘要无可压缩内容"),
            Err(error) => tracing::warn!(role_id, "后台自动摘要失败: {error}"),
        }
    });
}

/// 给前端/调试用的当前配置摘要。
pub fn describe(service: &MemoryService) -> serde_json::Value {
    let (top_k, min_score) = service.defaults();
    json!({
        "enabled": service.enabled(),
        "configured": service.is_configured(),
        "embedder": if service.remote.is_some() { "remote" } else if local_embedding::is_model_available() { "local" } else { "none" },
        "retrieval": service.retrieval_mode(),
        "model": service.model(),
        "top_k": top_k,
        "min_score": min_score,
        "auto_summarize_threshold": AUTO_SUMMARIZE_THRESHOLD,
        "keep_recent": DEFAULT_KEEP_RECENT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_suffix_round_trips() {
        let tags = vec!["偏好".to_string(), "饮品".to_string()];
        let tagged = with_tags_suffix("用户喜欢冰美式", &tags);
        assert_eq!(tagged, "用户喜欢冰美式\n[标签: 偏好,饮品]");
        let (body, parsed) = split_tags_suffix(&tagged);
        assert_eq!(body, "用户喜欢冰美式");
        assert_eq!(parsed, tags);
    }

    #[test]
    fn no_tags_leaves_content_unchanged() {
        assert_eq!(with_tags_suffix("abc", &[]), "abc");
        assert_eq!(split_tags_suffix("abc").0, "abc");
        assert!(split_tags_suffix("abc").1.is_empty());
    }

    /// 正文里出现 `[标签: …]` 字样、但不在最后一行时不应被误拆。
    #[test]
    fn mid_content_tag_like_text_is_not_split() {
        let content = "她提到过\n[标签: 某某]\n后面还有正文";
        assert_eq!(split_tags_suffix(content).0, content);
        assert!(split_tags_suffix(content).1.is_empty());
    }
}
