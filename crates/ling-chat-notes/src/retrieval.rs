//! 笔记语义检索：复用 [`VectorStore`] 持久化句向量，按内容指纹增量维护。
//!
//! 每个角色、每个嵌入模型一个独立的向量库文件
//! （`<data_dir>/vector_index/notes/<模型目录>/<角色>.db`），避免不同角色或不同
//! 模型空间的向量互相干扰。向量 ID 由 `note_id:fingerprint` 组成，正文或标签
//! 变化后指纹随之变化，旧向量会在下一次检索时被清理并重建。
//!
//! 本模块的接口是同步阻塞的（SQLite 与 ONNX 推理），调用方负责放进
//! `spawn_blocking`。

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};

use crate::embedding::{EmbeddingEngine, MODEL_DIR_NAME};
use crate::vector::VectorStore;
use crate::{Note, sanitize_role_name};

/// 默认返回条数。
pub const DEFAULT_TOP_K: usize = 8;

/// 允许返回的最大条数。
pub const MAX_TOP_K: usize = 20;

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
pub struct NotesVectorIndex {
    root: PathBuf,
    stores: Mutex<HashMap<String, Arc<VectorStore>>>,
}

impl NotesVectorIndex {
    /// `data_dir` 为应用数据目录，索引落在其下的 `vector_index/notes/<模型目录>`。
    /// 按模型目录分宿，换模型后自动从空索引开始，不会与旧模型空间混用。
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            root: data_dir
                .as_ref()
                .join("vector_index")
                .join("notes")
                .join(MODEL_DIR_NAME),
            stores: Mutex::new(HashMap::new()),
        }
    }

    fn store(&self, role: &str) -> Result<Arc<VectorStore>> {
        let mut stores = self
            .stores
            .lock()
            .map_err(|_| anyhow!("笔记向量库连接锁已损坏"))?;
        if let Some(store) = stores.get(role) {
            return Ok(store.clone());
        }
        std::fs::create_dir_all(&self.root)?;
        let path = self.root.join(format!("{}.db", sanitize_role_name(role)));
        let store = Arc::new(VectorStore::open(path)?);
        stores.insert(role.to_string(), store.clone());
        Ok(store)
    }

    /// 同步索引并检索，返回按相似度降序的笔记 ID。
    ///
    /// 只返回当前笔记列表里仍然存在的 ID；空列表时直接返回空。
    pub fn search(
        &self,
        engine: &EmbeddingEngine,
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
        let store = self.store(role)?;

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

        // 与查询一起批量嵌入缺失的笔记；首次建索引只有一次会话往返。
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

        let mut texts = Vec::with_capacity(missing.len() + 1);
        texts.push(query.to_string());
        texts.extend(missing.iter().map(|(_, _, text)| (*text).to_string()));
        let vectors = engine.embed_batch(&texts)?;
        let Some((query_vector, note_vectors)) = vectors.split_first() else {
            return Err(anyhow!("嵌入结果为空"));
        };
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
        if !entries.is_empty() {
            store.upsert_many(entries)?;
        }

        let hits = store.search(query_vector, clamp_top_k(top_k))?;
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
                .search(&engine, "爱丽丝", &notes, "食物过敏", 8)
                .is_err()
        );
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
            .search(&engine, "爱丽丝", &notes, "我对什么食物过敏", 3)
            .unwrap();
        assert_eq!(hits.first().map(String::as_str), Some("allergy-peanut"));

        // 第二次应命中已持久化的向量，结果稳定。
        let again = index
            .search(&engine, "爱丽丝", &notes, "我对什么食物过敏", 3)
            .unwrap();
        assert_eq!(hits, again);

        // 标签变化后旧向量失效，重建后仍能正确命中。
        let mut updated = notes.clone();
        updated[1].tags = vec!["失眠".to_string()];
        let hits = index
            .search(&engine, "爱丽丝", &updated, "睡不着怎么办", 3)
            .unwrap();
        assert!(hits.contains(&"sleep".to_string()), "实际结果: {hits:?}");
    }
}
