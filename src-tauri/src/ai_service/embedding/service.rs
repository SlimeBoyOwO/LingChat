//! 嵌入服务管理：加载 ONNX 模型，对文本分词、ONNX 前向推理、mean pool + L2 归一化，
//! 返回向量。不再依赖 Python 子进程。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use ort::session::Session;
use ort::value::Tensor;

use super::tokenizer::{Tokenized, Tokenizer};

/// 嵌入服务配置（供 `EmbeddingManager::new` 消费）。
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// 嵌入模型目录（需含 `model.onnx` + `tokenizer.json`）。
    pub model_dir: PathBuf,
    /// 后端选择：`"auto"` / `"onnx"` / `"st"`（Rust 侧仅支持 onnx）。
    pub backend: String,
    /// 查询（query）文本前缀。仅 E5/INSTRUCTOR 系等以 `query: + 文本` 训练的模型
    /// 需要；内置 paraphrase-multilingual-MiniLM 模型无需前缀，默认为空。
    pub query_prefix: String,
    /// 语料/记忆片段（passage）文本前缀（同上，默认为空）。
    pub passage_prefix: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_dir: PathBuf::new(),
            backend: "auto".into(),
            query_prefix: String::new(),
            passage_prefix: String::new(),
        }
    }
}

impl EmbeddingConfig {
    pub fn enabled(&self) -> bool {
        let dir = &self.model_dir;
        dir.is_dir() && self.has_model_files(dir)
    }

    fn has_model_files(&self, dir: &Path) -> bool {
        // 量化模型（体积小、优先）或原始 float32 模型任一存在即可。
        let onnx_ok = ["model_quantized.onnx", "model_int8.onnx", "model.onnx"]
            .iter()
            .any(|name| dir.join(name).exists());
        if !onnx_ok {
            return false;
        }
        dir.join("tokenizer.json").exists() || dir.join("vocab.txt").exists()
    }
}

struct Runtime {
    session: Session,
    tokenizer: Tokenizer,
    dim: usize,
    model_name: String,
    /// 可复用的全零 token_type_ids 缓冲区（按 max_length 分配一次，避免每次推理重建）。
    zero_ttype: Vec<i64>,
}

impl Runtime {
    fn run(&mut self, tok: &Tokenized) -> Result<Vec<f32>> {
        run_session(&mut self.session, tok, &mut self.zero_ttype)
    }
}

/// 嵌入服务管理器。
pub struct EmbeddingManager {
    cfg: EmbeddingConfig,
    runtime: Mutex<Option<Runtime>>,
    last_error: std::sync::Mutex<Option<String>>,
}

impl EmbeddingManager {
    pub fn new(cfg: EmbeddingConfig) -> Self {
        Self {
            cfg,
            runtime: Mutex::new(None),
            last_error: std::sync::Mutex::new(None),
        }
    }

    pub async fn is_ready(&self) -> bool {
        self.runtime.lock().unwrap().is_some()
    }

    pub async fn dim(&self) -> Option<usize> {
        self.runtime.lock().unwrap().as_ref().map(|r| r.dim)
    }

    pub async fn model_name(&self) -> Option<String> {
        self.runtime.lock()
            .unwrap()
            .as_ref()
            .map(|r| r.model_name.clone())
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    pub fn configured(&self) -> bool {
        self.cfg.enabled()
    }

    pub fn shutdown(&self) {
        // runtime drop 即释放 session，无需单独关闭
    }

    /// 确保模型已加载（幂等）。供状态查询主动触发探测。
    pub async fn ensure_started(&self) -> bool {
        if !self.cfg.enabled() {
            return false;
        }
        let mut guard = self.runtime.lock().unwrap();
        if guard.is_some() {
            return true;
        }
        match self.load() {
            Ok(rt) => {
                *guard = Some(rt);
                true
            }
            Err(e) => {
                let msg = format!("{e:#}");
                *self.last_error.lock().unwrap() = Some(msg.clone());
                tracing::warn!("[embedding] 加载失败（状态查询触发）: {msg}");
                false
            }
        }
    }

    /// 无前缀 encode（供诊断/测试）。记忆库请使用 `encode_queries` / `embed_passages`。
    pub async fn encode(&self, texts: &[String]) -> Option<Vec<Vec<f32>>> {
        self.encode_prefixed(texts, "").await
    }

    /// 查询文本 encode：自动加 `query_prefix`（若配置为非空的模型会加前缀）。
    pub async fn encode_queries(&self, texts: &[String]) -> Option<Vec<Vec<f32>>> {
        self.encode_prefixed(texts, &self.cfg.query_prefix).await
    }

    /// 语料/记忆片段 encode：自动加 `passage_prefix`（同上，视模型而定）。
    /// 返回的 `Embedded.text` 仍是原始文本（不含前缀）。
    pub async fn embed_passages(&self, texts: &[String]) -> Option<Vec<Embedded>> {
        let vecs = self.encode_prefixed(texts, &self.cfg.passage_prefix).await?;
        Some(
            texts.iter()
                .zip(vecs.into_iter())
                .map(|(t, v)| Embedded { text: t.clone(), vector: v })
                .collect(),
        )
    }

    async fn encode_prefixed(&self, texts: &[String], prefix: &str) -> Option<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Some(Vec::new());
        }
        if !self.cfg.enabled() {
            return None;
        }
        let mut guard = self.runtime.lock().unwrap();
        if guard.is_none() {
            match self.load() {
                Ok(rt) => *guard = Some(rt),
                Err(e) => {
                    let msg = format!("{e:#}");
                    *self.last_error.lock().unwrap() = Some(msg.clone());
                    tracing::warn!("[embedding] 加载失败，嵌入功能禁用: {msg}");
                    return None;
                }
            }
        }
        let rt = guard.as_mut().expect("just loaded");

        let started = std::time::Instant::now();
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let clean = sanitize(text);
            let input = if prefix.is_empty() {
                clean.to_string()
            } else {
                format!("{prefix}{clean}")
            };
            let tok = rt.tokenizer.encode(&input);
            match rt.run(&tok) {
                Ok(vec) => results.push(vec),
                Err(e) => {
                    let msg = format!("{e:#}");
                    *self.last_error.lock().unwrap() = Some(msg.clone());
                    tracing::warn!("[embedding] encode 失败: {msg}");
                    return None;
                }
            }
        }
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        tracing::debug!(
            "[embedding] encode: n={} dim={} elapsed={:.2}ms (avg {:.2}ms/条)",
            texts.len(),
            rt.dim,
            elapsed_ms,
            elapsed_ms / texts.len().max(1) as f64,
        );
        Some(results)
    }

    /// 记录一次向量相似度矩阵（供诊断 embedding 检索质量）。
    #[cfg(debug_assertions)]
    fn log_similarities(&self, label: &str, a: &[String], b: &[String], matrix: &[Vec<f32>]) {
        for (i, ai) in a.iter().enumerate() {
            for (j, bj) in b.iter().enumerate() {
                tracing::debug!(
                    "[embedding] sim[{label}] {:.3} | A{i}={:?} | B{j}={:?}",
                    matrix[i][j],
                    truncate(ai),
                    truncate(bj),
                );
            }
        }
    }

    pub async fn cosine_similarity_matrix(&self, a: &[String], b: &[String]) -> Option<Vec<Vec<f32>>> {
        let va = self.encode(a).await?;
        let vb = self.encode(b).await?;
        let out: Vec<Vec<f32>> = va.iter()
            .map(|x| vb.iter().map(|y| cosine(x, y)).collect())
            .collect();
        #[cfg(debug_assertions)]
        self.log_similarities("cosine", a, b, &out);
        Some(out)
    }

    pub async fn embed_many(&self, texts: &[String]) -> Option<Vec<Embedded>> {
        let vecs = self.encode(texts).await?;
        Some(
            texts.iter()
                .zip(vecs.into_iter())
                .map(|(t, v)| Embedded { text: t.clone(), vector: v })
                .collect(),
        )
    }

    // ── 内部 ──────────────────────────────────────────────────────────────

    fn load(&self) -> Result<Runtime> {
        if self.cfg.backend == "st" {
            return Err(anyhow!("Rust 侧不支持 sentence-transformers 后端（需 torch），请使用 backend=auto/onnx"));
        }
        let model_dir = &self.cfg.model_dir;
        let onnx_path = Self::find_onnx(model_dir)
            .ok_or_else(|| anyhow!("模型目录缺少 ONNX 文件: {}", model_dir.display()))?;
        let tokenizer = Tokenizer::load(model_dir)
            .with_context(|| format!("加载 tokenizer 失败: {}", model_dir.display()))?;

        tracing::info!("[embedding] 加载 ONNX 模型: {}", onnx_path.display());
        let load_started = std::time::Instant::now();
        // 显式注册 CPU 执行提供者并关闭其 arena：ORT 默认 CPU arena 会随推理把
        // RSS 顶到 ~1GB，嵌入式小模型不值得（嵌入只有低频小批量推理）。
        let cpu = ort::ep::CPU::default()
            .with_arena_allocator(false)
            .into();
        let mut session = Session::builder()
            .map_err(|e| anyhow!("创建 SessionBuilder 失败: {e}"))?
            .with_execution_providers([cpu])
            .map_err(|e| anyhow!("注册 CPU 执行提供者失败: {e}"))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("设置优化级别失败: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("设置线程数失败: {e}"))?
            .commit_from_file(&onnx_path)
            .map_err(|e| anyhow!("加载 ONNX 模型失败 ({}): {e}", onnx_path.display()))?;

        // 探测维度：用单条 dummy 跑一次
        let dummy = tokenizer.encode("test");
        let mut zero_ttype = vec![0i64; dummy.input_ids.len()];
        let vec = run_session(&mut session, &dummy, &mut zero_ttype)?;
        let dim = vec.len();
        if dim == 0 {
            return Err(anyhow!("模型输出向量为空"));
        }
        let model_name = format!("onnx:{}", model_dir.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "embedding".into()));

        let load_ms = load_started.elapsed().as_secs_f64() * 1000.0;
        let size_mb = std::fs::metadata(&onnx_path)
            .map(|m| m.len() as f64 / (1024.0 * 1024.0))
            .unwrap_or(0.0);
        tracing::info!(
            "[embedding] 就绪: dim={dim} model={model_name} onnx={size_mb:.1}MB load={load_ms:.0}ms"
        );
        Ok(Runtime { session, tokenizer, dim, model_name, zero_ttype })
    }

    fn find_onnx(model_dir: &Path) -> Option<PathBuf> {
        // 量化模型体积小且推理更快，优先加载；无则退回原始 float32 模型。
        for name in ["model_quantized.onnx", "model_int8.onnx", "model.onnx"] {
            let p = model_dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }
}

/// 一条已编码文本。
#[derive(Debug, Clone)]
pub struct Embedded {
    pub text: String,
    pub vector: Vec<f32>,
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 { return 0.0; }
    dot / (na.sqrt() * nb.sqrt())
}

/// 截断超长文本。仅在超长时分配，短文本零拷贝复用原切片。
fn sanitize(text: &str) -> &str {
    if text.len() > 20000 {
        &text[..20000]
    } else {
        text
    }
}

/// 截断为用于日志诊断的短文本（限制长度 + 单行提示）。
#[cfg(debug_assertions)]
fn truncate(text: &str) -> &str {
    const MAX: usize = 60;
    let trimmed = text.trim();
    if trimmed.chars().count() > MAX {
        &trimmed[..trimmed.char_indices().nth(MAX).map(|(i, _)| i).unwrap_or(trimmed.len())]
    } else {
        trimmed
    }
}

/// 跑 ONNX session，对已 tokenized 的输入做 mean pooling + L2 归一化。
fn run_session(session: &mut Session, tok: &Tokenized, zero_ttype: &mut Vec<i64>) -> Result<Vec<f32>> {
    let n = tok.input_ids.len() as i64;

    let ids_tensor = Tensor::from_array(([1i64, n], tok.input_ids.clone().into_boxed_slice()))
        .context("创建 input_ids 张量失败")?;
    let mask_tensor = Tensor::from_array(([1i64, n], tok.attention_mask.clone().into_boxed_slice()))
        .context("创建 attention_mask 张量失败")?;
    if zero_ttype.len() < n as usize {
        zero_ttype.extend(std::iter::repeat(0i64).take(n as usize - zero_ttype.len()));
    }
    let ttype_tensor = Tensor::from_array(([1i64, n], zero_ttype[..n as usize].to_vec().into_boxed_slice()))
        .context("创建 token_type_ids 张量失败")?;

    let input_names: Vec<String> = session.inputs().iter().map(|o| o.name().to_string()).collect();

    // 按 session inputs 顺序：通常 [input_ids, attention_mask, token_type_ids]
    // 若模型无 token_type_ids（2 个输入）则只提供前两个。
    let outputs = match input_names.len() {
        2 => session.run(ort::inputs![
            input_names[0].as_str() => ids_tensor,
            input_names[1].as_str() => mask_tensor,
        ]).context("ONNX 推理失败（2 inputs）")?,
        _ => session.run(ort::inputs![
            input_names[0].as_str() => ids_tensor,
            input_names[1].as_str() => mask_tensor,
            input_names[2].as_str() => ttype_tensor,
        ]).context("ONNX 推理失败")?,
    };

    // 输出 last_hidden_state [1, seq, dim]
    let arr = outputs[0]
        .try_extract_array::<f32>()
        .context("输出张量类型不是 f32")?;
    let slice = arr.as_slice().ok_or_else(|| anyhow!("输出张量非连续布局"))?;
    // shape 是 [1, n, dim]
    let dim = slice.len() / (n as usize);
    if dim == 0 { return Err(anyhow!("模型输出维度为 0")); }
    let hidden = slice;

    // Mean pooling（attention mask 加权）
    let mask = &tok.attention_mask;
    let mask_sum: f32 = mask.iter().map(|&x| x as f32).sum();
    let mask_den = mask_sum.max(1e-9);
    let mut pooled = vec![0.0f32; dim];
    for t in 0..mask.len() {
        if mask[t] == 0 { continue; }
        let base = t * dim;
        for d in 0..dim {
            pooled[d] += hidden[base + d];
        }
    }
    for v in pooled.iter_mut() {
        *v /= mask_den;
    }

    // L2 归一化
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
    for v in pooled.iter_mut() {
        *v /= norm;
    }
    Ok(pooled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_of_perpendicular_vectors_is_zero() {
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
    }

    #[test]
    fn cosine_of_same_vector_is_one() {
        assert!((cosine(&[0.6, 0.8, 0.0], &[0.6, 0.8, 0.0]) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_of_opposite_vectors_is_negative_one() {
        assert!((cosine(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn mismatch_length_returns_zero() {
        assert_eq!(cosine(&[1.0, 2.0], &[1.0]), 0.0);
    }

    #[test]
    fn config_missing_model_is_disabled() {
        let cfg = EmbeddingConfig {
            model_dir: PathBuf::from("/nonexistent/embedding"),
            ..EmbeddingConfig::default()
        };
        assert!(!cfg.enabled());
    }

    /// 端到端集成测试：驱动真实 ONNX session（需 `EMBEDDING_TEST_MODEL` 指向含
    /// `model.onnx` + `tokenizer.json` 的目录；CI 无模型时跳过）。
    #[test]
    fn encodes_through_onnx_session() {
        let model = std::env::var("EMBEDDING_TEST_MODEL").unwrap_or_else(|_| "/tmp/tiny_emo".into());
        let repo_path = |p: &str| -> PathBuf {
            let b = PathBuf::from(p);
            if b.is_absolute() { b }
            else { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(&b) }
        };
        let model_dir = match std::fs::canonicalize(repo_path(&model)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("跳过：模型目录不可用 ({model}): {e}");
                return;
            }
        };
        let cfg = EmbeddingConfig { model_dir, backend: "auto".into(),
            ..EmbeddingConfig::default() };
        let mgr = EmbeddingManager::new(cfg);
        assert!(mgr.configured(), "配置应就绪");

        let mut rt = mgr.load().expect("load 应成功");
        let tok1 = rt.tokenizer.encode("你好世界");
        let v1 = rt.run(&tok1).expect("encode 应成功");
        assert_eq!(v1.len(), rt.dim);
        let tok2 = rt.tokenizer.encode("世界你好");
        let v2 = rt.run(&tok2).unwrap();
        assert!(cosine(&v1, &v2) > 0.9, "语义相近文本应高度相似");

        let tok3 = rt.tokenizer.encode("unrelated");
        let v3 = rt.run(&tok3).unwrap();
        // 语义相近的文本余弦应显著高于无关文本（不同语言/主题）。
        assert!(cosine(&v1, &v3) < cosine(&v1, &v2), "不同语言的余弦应更低");
    }

    /// 内存占用验证：关闭 CPU arena 后，多次推理的 RSS 增幅应有限（不随推理
    /// 数线性膨胀到 GB 级）。RSS 是进程级指标，须独占进程测量，故标记 `#[ignore]`：
    /// `EMBEDDING_TEST_MODEL=... cargo test --release -- --ignored memory_stays_low...`
    #[test]
    #[ignore = "RSS 需在独占进程下测量（并行测试会互相干扰）"]
    fn memory_stays_low_after_repeated_inference() {
        let model = std::env::var("EMBEDDING_TEST_MODEL").unwrap_or_default();
        if model.is_empty() {
            eprintln!("跳过（无 EMBEDDING_TEST_MODEL）");
            return;
        }
        let rss_kb = |label: &str| {
            let kb = std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| {
                    s.lines().find(|l| l.starts_with("VmRSS:")).map(|l| {
                        l.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0)
                    })
                })
                .unwrap_or(0);
            eprintln!("{label}: {:.0} MB", kb as f64 / 1024.0);
            kb
        };
        let model_dir = PathBuf::from(model);
        let mgr = EmbeddingManager::new(EmbeddingConfig {
            model_dir,
            ..EmbeddingConfig::default()
        });
        assert!(mgr.configured(), "配置应就绪");
        let before = rss_kb("加载前");
        let mut rt = mgr.load().expect("load 应成功");
        let after_load = rss_kb("加载后");
        for i in 0..6 {
            let tok = rt.tokenizer.encode(if i % 2 == 0 { "你好世界" } else { "The weather is nice today, let's go for a walk." });
            let _ = rt.run(&tok).unwrap();
        }
        let after_infer = rss_kb("6 次推理后");
        let load_growth = after_load.saturating_sub(before);
        let infer_growth = after_infer.saturating_sub(before);
        assert!(load_growth < 300 * 1024, "加载后 RSS 不应暴涨（实测 +{:.0}MB）", load_growth as f64 / 1024.0);
        assert!(infer_growth < 400 * 1024, "多次推理后 RSS 不应膨胀到 GB 级（实测 +{:.0}MB）", infer_growth as f64 / 1024.0);
    }

    /// 多语言语义检索质量测试（需本机模型，CI 无模型时跳过）。只断言命中"语义簇"
    /// 而非精确译文：paraphrase-multilingual-MiniLM 对三语"同义近重复"簇的排序
    /// 天然模棱两可，但必须命中正确的语义簇。
    #[test]
    fn multilingual_recall_hits_same_cluster() {
        if std::env::var("EMBEDDING_TEST_MODEL").is_err() {
            eprintln!("跳过（无 EMBEDDING_TEST_MODEL）");
            return;
        }
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let model_dir = std::env::var("EMBEDDING_TEST_MODEL")
                .map(PathBuf::from)
                .expect("EMBEDDING_TEST_MODEL 必须指向量化后的嵌入模型目录");
            let mgr = EmbeddingManager::new(EmbeddingConfig {
                model_dir,
                ..EmbeddingConfig::default()
            });
            assert!(mgr.ensure_started().await, "模型应加载成功");

            // 两个语义簇：天气(0..3) / 代码测试(3..6)
            let passages = [
                "今天天气晴朗，适合出门散步。",
                "The weather is sunny today, great for a walk.",
                "今日は晴れていて、散歩にぴったりです。",
                "代码需要单元测试覆盖。",
                "Code should be covered by unit tests.",
                "コードは単体テストでカバーすべきです。",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
            let pvs: Vec<Vec<f32>> = mgr
                .embed_passages(&passages)
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.vector)
                .collect();

            let queries = [
                "今天天气怎么样？",
                "How is the weather today?",
                "今日の天気はどうですか？",
                "要怎么测试这段代码？",
                "How should I test this code?",
                "このコードはどうテストすれば？",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
            let expected_cluster = [0usize, 0, 0, 1, 1, 1];
            let qvs = mgr.encode_queries(&queries).await.unwrap();
            for (i, qv) in qvs.iter().enumerate() {
                let mut best = 0usize;
                let mut best_s = -1.0f32;
                let mut row = String::new();
                for (j, pv) in pvs.iter().enumerate() {
                    let s = cosine(qv, pv);
                    row.push_str(&format!("[{j}] {s:.3} "));
                    if s > best_s {
                        best_s = s;
                        best = j;
                    }
                }
                eprintln!("查询[{i}] {:?} 分布: {row}", queries[i]);
                let got_cluster = if best < 3 { 0 } else { 1 };
                assert_eq!(
                    got_cluster, expected_cluster[i],
                    "查询[{i}] {:?} 应命中语义簇 {:?}，实际命中语料[{best}] {:?} (sim={best_s:.3})",
                    queries[i], expected_cluster[i], passages[best]
                );
            }
        });
    }
}