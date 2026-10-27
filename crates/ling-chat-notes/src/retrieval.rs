//! 笔记语义检索：复用 [`VectorStore`] 持久化句向量，按内容指纹增量维护。
//!
//! 每个角色、每个嵌入后端一个独立的向量库文件
//! （`<data_dir>/vector_index/notes/<后端命名空间>/<角色>.db`），本地 ONNX 与云端
//! 模型的向量空间因此永不混用。向量 ID 由 `note_id:fingerprint` 组成，正文或标签
//! 变化后指纹随之变化，旧向量会在下一次检索时被清理并重建。
//!
//! 本模块的接口是同步阻塞的（SQLite 与嵌入推理），调用方负责放进
//! `spawn_blocking`。

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};

use crate::embedding::Embedder;
use crate::vector::VectorStore;
use crate::{Note, sanitize_role_name};

/// 默认返回条数。
pub const DEFAULT_TOP_K: usize = 8;

/// 允许返回的最大条数。
pub const MAX_TOP_K: usize = 20;

/// 一次批量嵌入的条数上限。云端端点对单请求条数有限制，本地也按它切块控制内存。
const WARM_BATCH: usize = 32;

/// 维度未定的后端（云端）用来探测维度的短文本。
const DIMENSION_PROBE: &str = "维度探测";

/// 文档侧文本表示：正文 + 空格拼接的标签。
pub fn note_text(note: &Note) -> String {
    let tags = note.tags.join(" ");
    if tags.is_empty() {
        note.content.clone()
    } else {
        format!("{} {}", note.content.trim(), tags)
    }
}

/// 文本指纹，用于判断已持久化的向量是否仍然有效。
pub fn fingerprint_of(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// 把调用方给出的条数限制到合法范围。
pub fn clamp_top_k(value: usize) -> usize {
    value.clamp(1, MAX_TOP_K)
}

fn vector_id(note_id: &str, fingerprint: u64) -> String {
    format!("{note_id}:{fingerprint:016x}")
}

fn split_vector_id(id: &str) -> Option<(&str, u64)> {
    let (note_id, fingerprint) = id.rsplit_once(':')?;
    let fingerprint = u64::from_str_radix(fingerprint, 16).ok()?;
    Some((note_id, fingerprint))
}

/// 对账：返回（需要删除的 ID，仍然有效的 note id 集合）。
fn classify_existing<'a>(
    existing: &'a [String],
    current: &HashMap<&'a str, (u64, String)>,
) -> (Vec<&'a str>, HashSet<&'a str>) {
    let mut stale = Vec::new();
    let mut valid = HashSet::new();
    for id in existing {
        match split_vector_id(id) {
            Some((note_id, fingerprint))
                if current.get(note_id).map(|(current, _)| *current) == Some(fingerprint) =>
            {
                valid.insert(note_id);
            },
            _ => stale.push(id.as_str()),
        }
    }
    (stale, valid)
}

/// 按角色隔离的笔记向量索引。
///
/// 库文件按「嵌入后端命名空间 / 角色」两级分宿，换后端等于换一整棵目录树，
/// 天然从空索引开始，不会与另一种向量空间混用。
pub struct NotesVectorIndex {
    root: PathBuf,
    stores: Mutex<HashMap<(String, String), Arc<VectorStore>>>,
}

impl NotesVectorIndex {
    /// `data_dir` 为应用数据目录，索引落在其下的 `vector_index/notes`。
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            root: data_dir.as_ref().join("vector_index").join("notes"),
            stores: Mutex::new(HashMap::new()),
        }
    }

    /// 取后端的分库目录名与向量维度。
    ///
    /// 云端后端在首次成功响应前不知道维度：用一次短探针确定并缓存，之后同一后端
    /// 的调用都是纯查表。探针失败即报错，由调用方走既有降级路径。
    fn backend_of(embedder: &dyn Embedder) -> Result<(String, usize)> {
        let namespace = embedder
            .namespace()
            .ok_or_else(|| anyhow!("嵌入后端未提供索引命名空间"))?;
        if let Some(dimension) = embedder.dimension() {
            return Ok((namespace, dimension));
        }
        embedder.embed(DIMENSION_PROBE)?;
        let dimension = embedder
            .dimension()
            .ok_or_else(|| anyhow!("嵌入后端未回报向量维度"))?;
        Ok((namespace, dimension))
    }

    fn store(&self, namespace: &str, role: &str, dimension: usize) -> Result<Arc<VectorStore>> {
        let key = (namespace.to_string(), role.to_string());
        let mut stores = self
            .stores
            .lock()
            .map_err(|_| anyhow!("笔记向量库连接锁已损坏"))?;
        if let Some(store) = stores.get(&key) {
            return Ok(store.clone());
        }
        let dir = self.root.join(sanitize_role_name(namespace));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.db", sanitize_role_name(role)));
        let store = Arc::new(VectorStore::open(path, dimension)?);
        stores.insert(key, store.clone());
        Ok(store)
    }

    /// 对账并补齐缺失向量：删除已失效条目，把当前没有向量的笔记（可连带查询本身）
    /// 分批嵌入后写库。返回（该角色的向量库，查询向量，新嵌入条数）。
    fn sync(
        &self,
        embedder: &dyn Embedder,
        role: &str,
        notes: &[Note],
        query: Option<&str>,
    ) -> Result<(Arc<VectorStore>, Option<Vec<f32>>, usize)> {
        let (namespace, dimension) = Self::backend_of(embedder)?;
        let store = self.store(&namespace, role, dimension)?;

        let current: HashMap<&str, (u64, String)> = notes
            .iter()
            .map(|note| {
                let text = note_text(note);
                (note.id.as_str(), (fingerprint_of(&text), text))
            })
            .collect();

        // 清理已删除的笔记与内容变化后残留的旧向量。
        let existing = store.ids()?;
        let (stale, valid) = classify_existing(&existing, &current);
        if !stale.is_empty() {
            store.delete_many(stale)?;
        }

        let missing: Vec<(&Note, u64, &str)> = notes
            .iter()
            .filter_map(|note| {
                let (fingerprint, text) = current.get(note.id.as_str())?;
                if valid.contains(note.id.as_str()) {
                    return None;
                }
                Some((note, *fingerprint, text.as_str()))
            })
            .collect();

        // 查询与缺失笔记一起批量嵌入：首次建索引只有一次后端往返。
        let want_query = query.is_some();
        let mut texts = Vec::with_capacity(missing.len() + 1);
        if let Some(query) = query {
            texts.push(query.to_string());
        }
        texts.extend(missing.iter().map(|(_, _, text)| (*text).to_string()));
        if texts.is_empty() {
            return Ok((store, None, 0));
        }

        let mut query_vector: Option<Vec<f32>> = None;
        let mut note_vectors: Vec<Vec<f32>> = Vec::with_capacity(missing.len());
        let mut index = 0usize;
        for chunk in texts.chunks(WARM_BATCH) {
            let vectors = embedder.embed_batch(chunk)?;
            if vectors.len() != chunk.len() {
                return Err(anyhow!(
                    "嵌入结果数量与输入不一致: 期望 {}，实际 {}",
                    chunk.len(),
                    vectors.len()
                ));
            }
            for vector in vectors {
                if index == 0 && want_query {
                    query_vector = Some(vector);
                } else {
                    note_vectors.push(vector);
                }
                index += 1;
            }
        }
        if note_vectors.len() != missing.len() {
            return Err(anyhow!(
                "嵌入结果数量与输入不一致: 期望 {}，实际 {}",
                missing.len(),
                note_vectors.len()
            ));
        }

        let ids: Vec<String> = missing
            .iter()
            .map(|(note, fingerprint, _)| vector_id(&note.id, *fingerprint))
            .collect();
        let entries: Vec<(&str, &[f32])> = ids
            .iter()
            .zip(note_vectors.iter())
            .map(|(id, vector)| (id.as_str(), vector.as_slice()))
            .collect();
        let synced = entries.len();
        if !entries.is_empty() {
            store.upsert_many(entries)?;
        }
        Ok((store, query_vector, synced))
    }

    /// 只建索引不检索（保存云端配置后的后台预热用），返回新嵌入的条数。
    pub fn warm(&self, embedder: &dyn Embedder, role: &str, notes: &[Note]) -> Result<usize> {
        if notes.is_empty() {
            return Ok(0);
        }
        self.sync(embedder, role, notes, None)
            .map(|(_, _, synced)| synced)
    }

    /// 同步索引并检索，返回按相似度降序的笔记 ID。
    ///
    /// 只返回当前笔记列表里仍然存在的 ID；空列表时直接返回空。
    pub fn search(
        &self,
        embedder: &dyn Embedder,
        role: &str,
        notes: &[Note],
        query: &str,
        top_k: usize,
    ) -> Result<Vec<String>> {
        let query = query.trim();
        if query.is_empty() {
            return Err(anyhow!("检索关键词为空"));
        }
        if notes.is_empty() {
            return Ok(Vec::new());
        }

        let (store, query_vector, _) = self.sync(embedder, role, notes, Some(query))?;
        let Some(query_vector) = query_vector else {
            return Err(anyhow!("嵌入结果为空"));
        };

        let hits = store.search(&query_vector, clamp_top_k(top_k))?;
        let live: HashSet<&str> = notes.iter().map(|note| note.id.as_str()).collect();
        let mut found = Vec::with_capacity(hits.len());
        for hit in &hits {
            let Some((note_id, _)) = split_vector_id(&hit.id) else {
                continue;
            };
            if live.contains(note_id) {
                found.push(note_id.to_string());
            }
        }
        if let Some(top) = hits.first() {
            tracing::debug!(role, top_score = 1.0 - top.distance, "笔记语义检索完成");
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{EmbeddingEngine, MODEL_DIR_NAME};

    fn note(id: &str, content: &str, tags: &[&str]) -> Note {
        Note {
            id: id.to_string(),
            content: content.to_string(),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn text_fingerprint_and_vector_id() {
        let tagged = note("n1", "对花生过敏", &["健康", "食物"]);
        assert_eq!(note_text(&tagged), "对花生过敏 健康 食物");
        assert_eq!(note_text(&note("n2", "对花生过敏", &[])), "对花生过敏");

        let before = fingerprint_of(&note_text(&tagged));
        let after = fingerprint_of(&note_text(&note("n1", "对花生过敏", &["健康"])));
        assert_ne!(before, after, "标签变化必须改变指纹");

        let id = vector_id("n1", before);
        assert_eq!(split_vector_id(&id), Some(("n1", before)));
        assert_eq!(split_vector_id("格式不对"), None);
    }

    #[test]
    fn reconcile_drops_deleted_and_changed_notes() {
        let unchanged = note("a", "喜欢猫", &["偏好"]);
        let changed = note("b", "新内容", &[]);
        let mut current = HashMap::new();
        for note in [&unchanged, &changed] {
            let text = note_text(note);
            current.insert(note.id.as_str(), (fingerprint_of(&text), text));
        }
        // b 的向量来自旧文本，c 已被删除。
        let existing = vec![
            vector_id("a", current["a"].0),
            vector_id("b", fingerprint_of("旧内容")),
            vector_id("c", 7),
        ];

        let (stale, valid) = classify_existing(&existing, &current);
        assert_eq!(valid, HashSet::from(["a"]));
        assert_eq!(stale.len(), 2);
    }

    #[test]
    fn disabled_engine_returns_error() {
        let engine = EmbeddingEngine::load(std::path::Path::new("__不存在的模型目录__"), false);
        assert!(!engine.is_enabled());
        let dir = tempfile::tempdir().unwrap();
        let index = NotesVectorIndex::new(dir.path());
        let notes = vec![note("n1", "对花生过敏", &[])];
        assert!(
            index
                .search(engine.as_ref(), "爱丽丝", &notes, "食物过敏", 8)
                .is_err()
        );
    }

    /// 假后端：不起模型就能验证「按后端分库 + 维度参数化」这条路。
    struct StubEmbedder {
        namespace: &'static str,
        dimension: Option<usize>,
        enabled: bool,
    }

    impl Embedder for StubEmbedder {
        fn is_enabled(&self) -> bool {
            self.enabled
        }

        fn dimension(&self) -> Option<usize> {
            self.dimension
        }

        fn namespace(&self) -> Option<String> {
            Some(self.namespace.to_string())
        }

        fn embed(&self, text: &str) -> Result<Vec<f32>> {
            let dimension = self
                .dimension
                .ok_or_else(|| anyhow!("stub 后端维度未定向"))?;
            // 用首字符决定轴向，保证同文本同向量、不同文本正交，便于断言排序。
            let axis = text
                .chars()
                .next()
                .map(|c| (c as usize) % dimension)
                .unwrap_or(0);
            let mut vector = vec![0.0; dimension];
            vector[axis] = 1.0;
            Ok(vector)
        }

        fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            texts.iter().map(|text| self.embed(text)).collect()
        }
    }

    #[test]
    fn undetermined_backend_dimension_degrades_to_error() {
        let dir = tempfile::tempdir().unwrap();
        let index = NotesVectorIndex::new(dir.path());
        let notes = vec![note("n1", "喜欢猫", &[])];
        let stub = StubEmbedder {
            namespace: "cloud/pending",
            dimension: None,
            enabled: true,
        };
        assert!(
            index.search(&stub, "爱丽丝", &notes, "宠物", 3).is_err()
                && index.warm(&stub, "爱丽丝", &notes).is_err()
        );
    }

    #[test]
    fn backends_are_isolated_by_namespace_and_dimension() {
        let dir = tempfile::tempdir().unwrap();
        let index = NotesVectorIndex::new(dir.path());
        let notes = vec![note("a", "喜欢猫", &[]), note("b", "对花生过敏", &[])];

        let local = StubEmbedder {
            namespace: MODEL_DIR_NAME,
            dimension: Some(512),
            enabled: true,
        };
        let cloud = StubEmbedder {
            namespace: "openai_text-embedding-3-small",
            dimension: Some(1536),
            enabled: true,
        };

        assert_eq!(index.warm(&local, "爱丽丝", &notes).unwrap(), 2);
        // 同一后端第二次预热不再调用嵌入。
        assert_eq!(index.warm(&local, "爱丽丝", &notes).unwrap(), 0);

        let local_db = dir
            .path()
            .join("vector_index/notes")
            .join(MODEL_DIR_NAME)
            .join("爱丽丝.db");
        let cloud_db = dir
            .path()
            .join("vector_index/notes")
            .join("openai_text-embedding-3-small")
            .join("爱丽丝.db");
        assert!(local_db.exists());
        assert!(!cloud_db.exists());

        // 换后端：从空索引开始，且不复用另一侧的库文件。
        assert_eq!(index.warm(&cloud, "爱丽丝", &notes).unwrap(), 2);
        assert!(cloud_db.exists());

        // 检索按当前后端的命名空间取库，命中顺序由该后端的向量决定。
        let hits = index.search(&local, "爱丽丝", &notes, "喜欢猫", 2).unwrap();
        assert_eq!(hits, vec!["a".to_string(), "b".to_string()]);
    }

    /// 端到端：真实模型 + sqlite-vec 持久化向量库。
    ///
    /// 需先下载模型：
    /// `cargo test -p ling-chat-notes --lib -- --ignored --exact retrieval::tests::retrieval_hits_semantic_notes --nocapture`
    #[test]
    #[ignore = "需要已下载的嵌入模型"]
    fn retrieval_hits_semantic_notes() {
        #[cfg(target_os = "windows")]
        {
            let dll = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("src-tauri/binaries/onnxruntime.dll");
            assert!(dll.exists(), "onnxruntime.dll 不存在: {}", dll.display());
            if let Err(e) = ort::init_from(&dll) {
                eprintln!("ort::init_from 跳过（可能已初始化）: {e}");
            }
        }

        let model_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("data/third_party/embedding_model_bge_small_zh");
        let engine = EmbeddingEngine::load(&model_dir, true);
        assert!(
            engine.is_enabled(),
            "引擎未启用，检查模型目录: {}",
            model_dir.display()
        );

        let dir = tempfile::tempdir().unwrap();
        let index = NotesVectorIndex::new(dir.path());
        let notes = vec![
            note(
                "allergy-peanut",
                "对花生过敏，不能吃花生和花生油",
                &["健康", "食物"],
            ),
            note("sleep", "睡不着的时候可以听白噪音", &["睡眠"]),
            note("cat", "喜欢猫，想养一只橘猫", &["偏好"]),
        ];

        let hits = index
            .search(engine.as_ref(), "爱丽丝", &notes, "我对什么食物过敏", 3)
            .unwrap();
        assert_eq!(hits.first().map(String::as_str), Some("allergy-peanut"));

        // 第二次应命中已持久化的向量，结果稳定。
        let again = index
            .search(engine.as_ref(), "爱丽丝", &notes, "我对什么食物过敏", 3)
            .unwrap();
        assert_eq!(hits, again);

        // 标签变化后旧向量失效，重建后仍能正确命中。
        let mut updated = notes.clone();
        updated[1].tags = vec!["失眠".to_string()];
        let hits = index
            .search(engine.as_ref(), "爱丽丝", &updated, "睡不着怎么办", 3)
            .unwrap();
        assert!(hits.contains(&"sleep".to_string()), "实际结果: {hits:?}");
    }
}
