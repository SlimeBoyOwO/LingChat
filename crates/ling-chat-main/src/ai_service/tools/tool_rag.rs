//! Tool RAG：把下发给模型的工具定义从「权限全集」收窄为「常驻工具 + 检索激活的工具」。
//!
//! 语料优先取工具配置的「可能的提问方式」（示例问句），没有配置时回退到描述；
//! 每个问句单独算向量，查询与工具的相关度取所有问句中的最大余弦。
//! 检索结果进入按角色划分的激活缓存，后续轮次直接下发，避免模型每次都要先搜一遍。
//!
//! 不变量：本模块只负责「缩小」工具集，权限矩阵放行的集合是所有判断的前提，
//! 激活缓存读取时始终与它求交，因此不可能带来权限提升。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{Result, anyhow};

use super::settings::{
    DEFAULT_TOOL_CACHE_CAPACITY, MAX_TOOL_CACHE_CAPACITY, MIN_TOOL_CACHE_CAPACITY,
};
use crate::ai_service::embedding::{Embedder, cosine_similarity};
use crate::ai_service::types::ToolDefinition;

/// 检索入口工具名。常驻下发，且不进检索语料（否则会检索到自己）。
pub const TOOL_SEARCH_NAME: &str = "tool_search";

/// 网页搜索工具名：用户要求常驻，不走检索。
pub const WEB_SEARCH_NAME: &str = "web_search";

/// 每次检索取回的候选数。
pub const SEARCH_TOP_K: usize = 5;

/// 静默推荐（玩家每说一句话就自动检索一次）最多推荐几件。
pub const AUTO_RECOMMEND_TOP_K: usize = 3;

/// 静默推荐的分数下限。没有下限的话每句寒暄都会「推荐」几个不相干的工具。
///
/// 实测口径见 `god_agent::capabilities::tool_hint` 的 `threshold_smoke`：
/// 改用示例问句做语料后，明确的工具意图 top-1 落在 0.65~0.86（「帮我加个日程」
/// 0.654、「今天天气怎么样」0.691、「换个场景吧」0.708），寒暄与闲聊 top-1
/// 都在 0.64 以下（「晚上好呀」0.594、「我今天心情不太好」0.633）；0.65 是切口。
pub const AUTO_RECOMMEND_MIN_SCORE: f32 = 0.65;

/// 角色名缺失时的激活缓存键（正常流程下角色名总存在，这只是兜底）。
const DEFAULT_ROLE_KEY: &str = "";

/// 常驻工具：永不走检索，直接随每轮请求下发（仍需在权限集内）。
pub fn is_resident(name: &str) -> bool {
    name == TOOL_SEARCH_NAME || name == WEB_SEARCH_NAME
}

/// 工具名 → (语料指纹, 每个语料文本一条归一化向量)。
type EmbeddingEntry = (u64, Vec<Arc<[f32]>>);

/// 一条待写回缓存的语料向量：工具名、指纹、向量。
type ComputedEntry = (String, EmbeddingEntry);

/// 静默检索的诊断快照：查询文本与过滤前的前三名（工具名、分数）。
type ProbeSnapshot = (String, Vec<(String, f32)>);

pub struct ToolRag {
    engine: Arc<dyn Embedder>,
    /// 工具名 -> (语料指纹, 归一化向量)。指纹变化或工具消失时自动失效，
    /// 因此插件热重载/卸载不需要任何主动通知。
    embeddings: RwLock<HashMap<String, EmbeddingEntry>>,
    /// 角色名 -> 激活工具。下标 0 为最近激活，用于 LRU 淘汰。
    activated: Mutex<HashMap<String, Vec<String>>>,
    /// 最近一次静默检索的诊断快照。
    /// 只给「查看推荐工具」解释「为什么没推荐」，不参与任何判断。
    last_probe: Mutex<Option<ProbeSnapshot>>,
    /// 每个角色同时保留的普通工具数量，随工具设置热更新。
    capacity: AtomicUsize,
}

impl ToolRag {
    pub fn new(engine: Arc<dyn Embedder>) -> Self {
        Self {
            engine,
            embeddings: RwLock::new(HashMap::new()),
            activated: Mutex::new(HashMap::new()),
            last_probe: Mutex::new(None),
            capacity: AtomicUsize::new(DEFAULT_TOOL_CACHE_CAPACITY),
        }
    }

    /// 更新每个角色保留的普通工具数量，并把已有缓存裁到新容量。
    pub fn set_capacity(&self, capacity: usize) {
        let capacity = capacity.clamp(MIN_TOOL_CACHE_CAPACITY, MAX_TOOL_CACHE_CAPACITY);
        self.capacity.store(capacity, Ordering::Relaxed);
        if let Ok(mut guard) = self.activated.lock() {
            for list in guard.values_mut() {
                list.truncate(capacity);
            }
        }
    }

    /// 当前激活缓存容量（不含常驻工具）。
    pub fn capacity(&self) -> usize {
        self.capacity.load(Ordering::Relaxed)
    }

    /// 嵌入引擎是否可用。不可用时调用方应回退到全量工具下发。
    pub fn is_enabled(&self) -> bool {
        self.engine.is_enabled()
    }

    /// 嵌入后端换了（本地↔云端、或换了云端模型）时作废语料缓存。
    ///
    /// 向量缓存是纯内存的，但新旧维度的向量混在一张表里会直接打错分，
    /// 因此后端变化必须由调用方显式清空。
    pub fn invalidate_embeddings(&self) {
        if let Ok(mut guard) = self.embeddings.write() {
            guard.clear();
        }
    }

    /// 预热：把候选工具的语料全部嵌入并写入缓存，返回（预热工具数, 语料条数）。
    ///
    /// 与检索走同一套指纹对账，已缓存的不会重复调用后端。
    pub async fn warm(&self, candidates: &[ToolDefinition]) -> Result<(usize, usize)> {
        if !self.is_enabled() {
            return Err(anyhow!("文本嵌入后端未启用，无法预热工具语料"));
        }
        let corpus = build_corpus(candidates);
        let pending: Vec<(String, Vec<String>, u64)> = {
            let guard = self
                .embeddings
                .read()
                .map_err(|e| anyhow!("嵌入缓存锁中毒: {e}"))?;
            corpus
                .iter()
                .filter(|entry| {
                    guard
                        .get(&entry.name)
                        .map(|(cached, _)| *cached != entry.fingerprint)
                        .unwrap_or(true)
                })
                .map(|entry| (entry.name.clone(), entry.texts.clone(), entry.fingerprint))
                .collect()
        };
        if pending.is_empty() {
            return Ok((0, 0));
        }

        let engine = self.engine.clone();
        let (computed, texts) = tokio::task::spawn_blocking(move || {
            let flat: Vec<String> = pending
                .iter()
                .flat_map(|(_, texts, _)| texts.iter().cloned())
                .collect();
            let total = flat.len();
            let vectors = engine.embed_batch(&flat)?;
            let mut offset = 0usize;
            let mut computed: Vec<ComputedEntry> = Vec::with_capacity(pending.len());
            for (name, texts, fingerprint) in pending {
                let end = offset + texts.len();
                let entry_vectors: Vec<Arc<[f32]>> = vectors[offset..end]
                    .iter()
                    .cloned()
                    .map(Arc::from)
                    .collect();
                computed.push((name, (fingerprint, entry_vectors)));
                offset = end;
            }
            Ok::<_, anyhow::Error>((computed, total))
        })
        .await
        .map_err(|e| anyhow!("预热嵌入任务失败: {e}"))??;

        let tools = computed.len();
        let mut guard = self
            .embeddings
            .write()
            .map_err(|e| anyhow!("嵌入缓存锁中毒: {e}"))?;
        for (name, entry) in computed {
            guard.insert(name, entry);
        }
        tracing::info!(tools, texts, "Tool RAG 语料预热完成");
        Ok((tools, texts))
    }

    /// 读取某角色已激活的工具（按最近激活在前）。
    pub fn activated_tools(&self, role: Option<&str>) -> Vec<String> {
        let key = role.unwrap_or(DEFAULT_ROLE_KEY);
        self.activated
            .lock()
            .map(|guard| guard.get(key).cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    /// 把工具加入激活缓存。
    ///
    /// `names` 按优先级从高到低给出（检索结果即按相似度降序），最重要的那个最终
    /// 排在最前、最不容易被淘汰。超出容量时淘汰队尾（最久未激活的）。
    pub fn touch(&self, role: Option<&str>, names: &[String]) {
        if names.is_empty() {
            return;
        }
        let key = role.unwrap_or(DEFAULT_ROLE_KEY).to_string();
        let Ok(mut guard) = self.activated.lock() else {
            return;
        };
        let list = guard.entry(key).or_default();
        // 逆序插入，使 names[0] 最终位于队首。
        for name in names.iter().rev() {
            // 常驻工具一直随请求下发，不占激活额度。
            if is_resident(name) {
                continue;
            }
            if let Some(pos) = list.iter().position(|existing| existing == name) {
                list.remove(pos);
            }
            list.insert(0, name.clone());
        }
        list.truncate(self.capacity());
    }

    /// 检索最匹配的 top_k 个工具，并把它们加入该角色的激活缓存。
    ///
    /// 常驻工具（[`TOOL_SEARCH_NAME`]、[`WEB_SEARCH_NAME`]）不进语料：它们本来就
    /// 一直下发，占用候选位只会挤掉真正需要被发现的工具。
    ///
    /// `min_score` 以下的命中直接不要，也不进激活缓存：`tool_search` 传负无穷（检索
    /// 是模型主动要求的，总得给点东西），静默推荐传 [`AUTO_RECOMMEND_MIN_SCORE`]。
    pub async fn search_and_activate(
        &self,
        role: Option<&str>,
        query: &str,
        candidates: &[ToolDefinition],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<(String, f32)>> {
        if !self.is_enabled() {
            return Err(anyhow!("文本嵌入引擎未启用，无法检索工具"));
        }
        let query = query.trim();
        if query.is_empty() {
            return Err(anyhow!("检索关键词为空"));
        }

        let corpus = build_corpus(candidates);
        if corpus.is_empty() {
            return Ok(Vec::new());
        }

        // 1) 找出需要计算向量的语料（缺失或语料变了）。锁只在这一步与写回时持有，
        //    绝不跨 await/阻塞计算持有。
        let pending: Vec<(String, Vec<String>)> = {
            let guard = self
                .embeddings
                .read()
                .map_err(|e| anyhow!("嵌入缓存锁中毒: {e}"))?;
            corpus
                .iter()
                .filter(|entry| {
                    guard
                        .get(&entry.name)
                        .map(|(cached, _)| *cached != entry.fingerprint)
                        .unwrap_or(true)
                })
                .map(|entry| (entry.name.clone(), entry.texts.clone()))
                .collect()
        };

        // 2) 一次性把查询与缺失语料丢进阻塞线程池（ort 推理是同步阻塞的）。
        //    首次检索会一次算完全部示例问句，之后按指纹直接命中缓存。
        let engine = self.engine.clone();
        let query_owned = query.to_string();
        let (query_vec, computed) = tokio::task::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let query_vec = engine.embed(&query_owned)?;
            let texts: Vec<String> = pending
                .iter()
                .flat_map(|(_, texts)| texts.iter().cloned())
                .collect();
            let vectors = engine.embed_batch(&texts)?;
            let mut computed: Vec<ComputedEntry> = Vec::with_capacity(pending.len());
            let mut offset = 0usize;
            for (name, texts) in pending {
                let end = offset + texts.len();
                let entry_vectors: Vec<Arc<[f32]>> = vectors[offset..end]
                    .iter()
                    .cloned()
                    .map(Arc::from)
                    .collect();
                computed.push((name, (fingerprint_of_texts(&texts), entry_vectors)));
                offset = end;
            }
            if !computed.is_empty() {
                tracing::info!(
                    tools = computed.len(),
                    texts = offset,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "Tool RAG 语料嵌入完成"
                );
            }
            Ok::<_, anyhow::Error>((query_vec, computed))
        })
        .await
        .map_err(|e| anyhow!("嵌入计算任务失败: {e}"))??;

        // 3) 写回缓存。不按本轮候选集修剪：不同调用来源的权限集不同，按候选修剪会让
        //    用户聊天与主动对话之间来回切换时反复重算全部向量；插件卸载留下的旧条目
        //    只占几个向量，进程重启自然清空。
        let mut guard = self
            .embeddings
            .write()
            .map_err(|e| anyhow!("嵌入缓存锁中毒: {e}"))?;
        for (name, entry) in computed {
            guard.insert(name, entry);
        }

        // 4) 打分排序取 top_k。一个工具有多条示例问句时取最高分。
        let mut scored: Vec<(String, f32)> = corpus
            .iter()
            .filter_map(|entry| {
                let (_, vectors) = guard.get(&entry.name)?;
                Some((entry.name.clone(), best_cosine(&query_vec, vectors)))
            })
            .collect();
        drop(guard);
        // 静默推荐的诊断快照：留下过滤前的前三名，调试指令据此解释「为什么没推荐」。
        // tool_search 传负无穷阈值，不参与记录。
        if min_score > f32::NEG_INFINITY {
            let mut probe = scored.clone();
            probe.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            probe.truncate(3);
            if let Ok(mut slot) = self.last_probe.lock() {
                *slot = Some((query.to_string(), probe));
            }
        }
        select_hits(&mut scored, top_k, min_score);

        // 5) 命中的工具进入该角色的激活缓存，下一轮即可作为工具定义下发。
        let hit_names: Vec<String> = scored.iter().map(|(name, _)| name.clone()).collect();
        self.touch(role, &hit_names);

        Ok(scored)
    }

    /// 最近一次静默检索的查询与过滤前的前三名（调试指令用）。
    pub fn last_probe(&self) -> Option<(String, Vec<(String, f32)>)> {
        self.last_probe.lock().ok().and_then(|guard| guard.clone())
    }
}

/// 从打分结果里挑出要激活的：按分数降序，低于阈值的一律不要，再取前 `top_k`。
///
/// 单独拆出来是为了能不起模型、直接单测阈值过滤与截断的边界。
fn select_hits(scored: &mut Vec<(String, f32)>, top_k: usize, min_score: f32) {
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.retain(|(_, score)| *score >= min_score);
    scored.truncate(top_k);
}

/// 一条工具的检索语料。
struct CorpusEntry {
    name: String,
    /// 每条语料单独算一个向量。
    texts: Vec<String>,
    fingerprint: u64,
}

/// 构建检索语料。
///
/// 常驻工具被排除：它们本来就一直下发，占用候选位只会挤掉真正需要被发现的工具。
fn build_corpus(candidates: &[ToolDefinition]) -> Vec<CorpusEntry> {
    candidates
        .iter()
        .filter(|def| !is_resident(&def.function.name))
        .filter_map(|def| {
            let texts = corpus_texts(def);
            if texts.is_empty() {
                return None;
            }
            Some(CorpusEntry {
                name: def.function.name.clone(),
                fingerprint: fingerprint_of_texts(&texts),
                texts,
            })
        })
        .collect()
}

/// 语料总条数（预热进度的分母）。
pub fn corpus_size(candidates: &[ToolDefinition]) -> usize {
    build_corpus(candidates)
        .iter()
        .map(|entry| entry.texts.len())
        .sum()
}

/// 语料文本：优先用工具配置的示例问句；没配置时回退到描述。
///
/// 回退只用描述、不拼英文工具名：`schedule_add_todo` 这类纯英文 token 对中文短句
/// 只是噪声，实测拼进去会拖低命中（见 `tool_hint` 的 `threshold_smoke`）。
fn corpus_texts(def: &ToolDefinition) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();
    for example in &def.rag_examples {
        let text = example.trim();
        if !text.is_empty() && !texts.iter().any(|existing| existing.as_str() == text) {
            texts.push(text.to_string());
        }
    }
    if texts.is_empty() {
        let description = def.function.description.trim();
        if !description.is_empty() {
            texts.push(description.to_string());
        }
    }
    texts
}

/// 语料指纹：条数、每条长度与内容按顺序入哈希，避免拼接歧义。
fn fingerprint_of_texts(texts: &[String]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    texts.len().hash(&mut hasher);
    for text in texts {
        text.len().hash(&mut hasher);
        text.hash(&mut hasher);
    }
    hasher.finish()
}

/// 查询与一组语料向量的最高余弦；没有向量时返回负无穷，交给阈值过滤。
fn best_cosine(query: &[f32], vectors: &[Arc<[f32]>]) -> f32 {
    vectors
        .iter()
        .map(|vector| cosine_similarity(query, vector))
        .fold(f32::NEG_INFINITY, f32::max)
}

/// 本轮实际下发给模型的工具名集合。
///
/// `rag_on == false` 时原样返回权限集，即今天的行为。开启时结果为
/// `{tool_search} ∪ {web_search} ∪ 激活集` 与权限集的交集。
///
/// 不变量：权限集为空（未归属角色组 / 场景组关闭）时不注入检索入口 —— 没有工具
/// 权限的角色不该白拿一个检索入口，结果也保证 ⊆ `base_allowed ∪ {tool_search}`。
pub fn effective_allowed(
    base_allowed: &HashSet<String>,
    rag: &ToolRag,
    role: Option<&str>,
    rag_on: bool,
) -> HashSet<String> {
    if !rag_on {
        return base_allowed.clone();
    }
    let mut effective = HashSet::new();
    if base_allowed.is_empty() {
        return effective;
    }
    effective.insert(TOOL_SEARCH_NAME.to_string());
    if base_allowed.contains(WEB_SEARCH_NAME) {
        effective.insert(WEB_SEARCH_NAME.to_string());
    }
    for name in rag.activated_tools(role) {
        if base_allowed.contains(&name) {
            effective.insert(name);
        }
    }
    effective
}

/// 清单里每个工具用途的截断长度。只做「认得出是什么」用，完整说明留给检索结果。
const MENU_PURPOSE_MAX_CHARS: usize = 36;

/// 没有可检索工具时的兜底描述。
const SEARCH_DESCRIPTION_FALLBACK: &str =
    "按需检索工具。当前没有可检索的工具，请如实告诉用户你做不到。";

/// 把 `tool_search` 的描述换成「当前可检索到的工具清单」。
///
/// 只给名字 + 一句用途，不给参数：模型必须先知道「有这些能力存在」才可能想到去检索，
/// 否则它会直接回答做不到；而具体用法仍由检索结果提供，token 收益得以保留。
pub fn apply_dynamic_search_description(
    definitions: &mut [ToolDefinition],
    available: &[ToolDefinition],
) {
    let Some(target) = definitions
        .iter_mut()
        .find(|def| def.function.name == TOOL_SEARCH_NAME)
    else {
        return;
    };
    target.function.description = search_description(available);
}

/// 渲染 `tool_search` 的动态描述。`available` 可以是权限集的全量定义，
/// 常驻工具（网页搜索等）会被自动跳过 —— 它们本来就一直下发，不必再列一遍。
pub fn search_description(available: &[ToolDefinition]) -> String {
    let mut menu = String::new();
    let mut count = 0usize;
    for def in available {
        if is_resident(&def.function.name) {
            continue;
        }
        let purpose = short_purpose(&def.function.description);
        if purpose.is_empty() {
            menu.push_str(&format!("- {}\n", def.function.name));
        } else {
            menu.push_str(&format!("- {}：{}\n", def.function.name, purpose));
        }
        count += 1;
    }

    if count == 0 {
        return SEARCH_DESCRIPTION_FALLBACK.to_string();
    }

    format!(
        "按需检索工具。下面是当前可用的工具清单（共 {count} 个，只列名字与用途，\
         完整参数需要检索后取得）：\n\
         {menu}\n\
         当用户要求你做某件事时，先从这份清单判断有没有合适的工具：有就用一句自然语言\
         描述你想完成的事调用本工具，它会返回匹配工具的名称与完整说明（含参数），\
         拿到之后再调用对应工具完成操作；若清单与检索结果里都没有合适的工具，\
         如实说明自己做不到，不要编造能力，也不要假装已经完成。"
    )
}

/// 取描述里第一句作为清单用途：优先到第一个句号为止，过长则按长度截断。
///
/// 截断时尽量切在逗号处（且保留长度不少于上限的一半），避免把一句话从词中间劈开、
/// 留下「返回 文件:行号: 内容…」这种读不出意思的尾巴。
fn short_purpose(description: &str) -> String {
    let first_line = description.lines().next().unwrap_or("").trim();
    let sentence = first_line
        .split('。')
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .unwrap_or(first_line);

    let chars: Vec<char> = sentence.chars().collect();
    if chars.len() <= MENU_PURPOSE_MAX_CHARS {
        return sentence.to_string();
    }

    let min_keep = MENU_PURPOSE_MAX_CHARS / 2;
    let clause_break = chars[..MENU_PURPOSE_MAX_CHARS]
        .iter()
        .rposition(|c| *c == '，')
        .filter(|index| *index >= min_keep);
    match clause_break {
        Some(index) => chars[..index].iter().collect(),
        None => {
            let mut truncated: String = chars[..MENU_PURPOSE_MAX_CHARS].iter().collect();
            truncated.push('…');
            truncated
        },
    }
}
