//! 本地 ONNX 句向量引擎（bge-small-zh-v1.5）。
//!
//! 以 [`Embedder`] 暴露给调用方。ONNX Runtime 的可用性由宿主作为参数传入，
//! 本 crate 不依赖 tauri，也不负责初始化运行时。
//!
//! 模型缺失、ONNX Runtime 不可用、分词或推理失败时一律降级为「禁用态」，
//! 由调用方回退到原有行为，绝不 panic。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Result, anyhow};
use ort::session::{Session, SessionInputValue};
use ort::value::{DynValue, Tensor};
use tokenizers::{Tokenizer, TruncationParams};

use super::{Embedder, l2_normalize};

/// `data/third_party/` 下的模型目录名，同时作为本地向量索引的分库目录。
pub const MODEL_DIR_NAME: &str = "embedding_model_bge_small_zh";

/// bge-small-zh-v1.5 的句向量维度，仅用于自检与日志。
const EXPECTED_DIMENSION: usize = 512;

/// 位置编码上限，超出部分截断。
const MAX_SEQ_LEN: usize = 512;

/// 单次 `session.run` 的最大批大小。批量建索引时用它切块，避免一次占用过多内存。
const BATCH_SIZE: usize = 32;

/// 池化后取哪个输出：bge 系列的正确句向量是 `last_hidden_state` 的 CLS 位，
/// 不走 pooler 的线性层。部分导出直接给出 `sentence_embedding`。
const PREFERRED_OUTPUTS: [&str; 3] = ["last_hidden_state", "sentence_embedding", "pooler_output"];

pub struct EmbeddingEngine {
    onnx_path: PathBuf,
    tokenizer: Option<Tokenizer>,
    /// 惰性创建的推理会话；`None` 表示创建失败且不再重试。
    session: OnceLock<Option<Mutex<Session>>>,
    /// 模型文件与分词器是否齐备（静态检查结果）。
    ready: bool,
    /// 运行期失败标记：一旦置位，本进程内不再使用嵌入（调用方据此回退）。
    failed: AtomicBool,
    disabled_reason: Option<String>,
}

impl EmbeddingEngine {
    /// 从模型目录加载。
    ///
    /// `onnx_ready` 由宿主告知 ONNX Runtime 是否已初始化（Windows 需要先加载
    /// onnxruntime.dll）。永不失败：任何前置条件不满足都返回禁用态实例。此处只做
    /// 廉价检查与分词器加载（几毫秒），真正的 ONNX 会话推迟到首次
    /// [`Self::embed`]，避免为从不使用语义检索的用户付启动成本。
    pub fn load(model_dir: &Path, onnx_ready: bool) -> Arc<Self> {
        let onnx_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        let mut engine = Self {
            onnx_path: onnx_path.clone(),
            tokenizer: None,
            session: OnceLock::new(),
            ready: false,
            failed: AtomicBool::new(false),
            disabled_reason: None,
        };

        // ONNX Runtime 不可用时绝不能建 Session：ort 在 dll 缺失时首次创建
        // 会话会 panic 并崩掉整个应用（见宿主 utils/onnx.rs 顶部说明）。
        if !onnx_ready {
            engine.disabled_reason = Some("ONNX Runtime 不可用（onnxruntime.dll 缺失）".into());
        } else if !onnx_path.exists() {
            engine.disabled_reason = Some(format!("模型文件不存在: {}", onnx_path.display()));
        } else if !tokenizer_path.exists() {
            engine.disabled_reason = Some(format!("分词文件不存在: {}", tokenizer_path.display()));
        } else {
            match Self::load_tokenizer(&tokenizer_path) {
                Ok(tokenizer) => {
                    engine.tokenizer = Some(tokenizer);
                    engine.ready = true;
                },
                Err(e) => engine.disabled_reason = Some(format!("加载分词器失败: {e}")),
            }
        }

        if engine.ready {
            tracing::info!("[embedding] 文本嵌入引擎就绪: {}", model_dir.display());
        } else {
            tracing::warn!(
                "[embedding] 文本嵌入引擎未启用: {}",
                engine.disabled_reason.as_deref().unwrap_or("未知原因")
            );
        }

        Arc::new(engine)
    }

    fn load_tokenizer(path: &Path) -> Result<Tokenizer> {
        let mut tokenizer =
            Tokenizer::from_file(path).map_err(|e| anyhow!("解析 {} 失败: {e}", path.display()))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_SEQ_LEN,
                ..Default::default()
            }))
            .map_err(|e| anyhow!("设置截断长度失败: {e}"))?;
        Ok(tokenizer)
    }

    /// 引擎当前是否可用。模型齐备且未发生过运行期失败时为 true。
    pub fn is_enabled(&self) -> bool {
        self.ready && !self.failed.load(Ordering::Acquire)
    }

    /// 计算文本的 L2 归一化句向量。
    ///
    /// 同步阻塞（ort 的 `Session::run`），调用方负责放进 `spawn_blocking`。
    /// 返回的向量已归一化，检索侧只需点积。
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if !self.is_enabled() {
            return Err(anyhow!(
                "文本嵌入引擎未启用: {}",
                self.disabled_reason.as_deref().unwrap_or("运行期已失败")
            ));
        }
        let Some(tokenizer) = self.tokenizer.as_ref() else {
            return Err(anyhow!("分词器未加载"));
        };

        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| anyhow!("分词失败: {e}"))?;
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&v| v as i64).collect();
        if ids.is_empty() {
            return Err(anyhow!("分词结果为空"));
        }
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&v| v as i64)
            .collect();
        let types: Vec<i64> = encoding.get_type_ids().iter().map(|&v| v as i64).collect();

        let seq = ids.len() as i64;
        let mut ids_tensor = Some(
            Tensor::from_array(([1i64, seq], ids.into_boxed_slice()))
                .map_err(|e| anyhow!("创建 input_ids 张量失败: {e}"))?,
        );
        let mut mask_tensor = Some(
            Tensor::from_array(([1i64, seq], mask.into_boxed_slice()))
                .map_err(|e| anyhow!("创建 attention_mask 张量失败: {e}"))?,
        );
        let mut types_tensor = Some(
            Tensor::from_array(([1i64, seq], types.into_boxed_slice()))
                .map_err(|e| anyhow!("创建 token_type_ids 张量失败: {e}"))?,
        );

        let Some(session_mutex) = self.session() else {
            // 会话建不出来是永久性的，置位后调用方本轮即回退，不必每轮重试。
            self.failed.store(true, Ordering::Release);
            return Err(anyhow!("嵌入模型会话不可用"));
        };
        let mut session = session_mutex
            .lock()
            .map_err(|e| anyhow!("嵌入会话锁中毒: {e}"))?;

        // 按会话声明的输入名喂张量，避免依赖输入顺序；不同导出未必带 token_type_ids。
        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();
        if !input_names.iter().any(|name| name == "input_ids") {
            return Err(anyhow!("嵌入模型缺少 input_ids 输入: {input_names:?}"));
        }
        let mut feeds: Vec<(String, SessionInputValue<'_>)> = Vec::with_capacity(input_names.len());
        for name in &input_names {
            match name.as_str() {
                "input_ids" => {
                    if let Some(tensor) = ids_tensor.take() {
                        feeds.push(("input_ids".to_string(), tensor.into()));
                    }
                },
                "attention_mask" => {
                    if let Some(tensor) = mask_tensor.take() {
                        feeds.push(("attention_mask".to_string(), tensor.into()));
                    }
                },
                "token_type_ids" => {
                    if let Some(tensor) = types_tensor.take() {
                        feeds.push(("token_type_ids".to_string(), tensor.into()));
                    }
                },
                _ => {},
            }
        }

        let outputs = session
            .run(feeds)
            .map_err(|e| anyhow!("嵌入推理失败: {e}"))?;

        let value: &DynValue = PREFERRED_OUTPUTS
            .iter()
            .find_map(|name| outputs.get(*name))
            .unwrap_or(&outputs[0]);
        let array = value
            .try_extract_array::<f32>()
            .map_err(|e| anyhow!("嵌入输出不是 f32 张量: {e}"))?;
        let shape = array.shape().to_vec();
        let data = array
            .as_slice()
            .ok_or_else(|| anyhow!("嵌入输出张量非连续布局"))?;
        let Some(&last) = shape.last() else {
            return Err(anyhow!("嵌入输出张量没有维度"));
        };

        // batch 固定为 1：[1, seq, dim] 取 CLS（首个 token）那一行，[1, dim] 直接用。
        let take = if shape.len() >= 2 { last } else { data.len() };
        if take == 0 || take > data.len() {
            return Err(anyhow!(
                "嵌入输出维度异常: shape={shape:?} 元素数={}",
                data.len()
            ));
        }
        if last != EXPECTED_DIMENSION {
            tracing::debug!(
                "[embedding] 模型输出维度 {} 与预期 {EXPECTED_DIMENSION} 不一致（换了模型？）",
                last
            );
        }

        Ok(l2_normalize(data[..take].to_vec()))
    }

    /// 批量计算 L2 归一化句向量。
    ///
    /// 内部按 [`BATCH_SIZE`] 切块，每块一次 `session.run`；批内按最长序列补齐。
    /// 同步阻塞，调用方负责放进 `spawn_blocking`。
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if !self.is_enabled() {
            return Err(anyhow!(
                "文本嵌入引擎未启用: {}",
                self.disabled_reason.as_deref().unwrap_or("运行期已失败")
            ));
        }
        let Some(tokenizer) = self.tokenizer.as_ref() else {
            return Err(anyhow!("分词器未加载"));
        };
        // 优先用分词器配置的 pad id，其次按 BERT 系惯例取 [PAD]。
        let pad_id = tokenizer
            .get_padding()
            .map(|padding| padding.pad_id)
            .or_else(|| tokenizer.token_to_id("[PAD]"))
            .unwrap_or(0) as i64;

        let mut vectors = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH_SIZE) {
            vectors.extend(self.embed_chunk(tokenizer, chunk, pad_id)?);
        }
        Ok(vectors)
    }

    fn embed_chunk(
        &self,
        tokenizer: &Tokenizer,
        texts: &[String],
        pad_id: i64,
    ) -> Result<Vec<Vec<f32>>> {
        let encodings = tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow!("批量分词失败: {e}"))?;
        let max_len = encodings
            .iter()
            .map(|encoding| encoding.get_ids().len())
            .max()
            .unwrap_or(0);
        if max_len == 0 {
            return Err(anyhow!("分词结果为空"));
        }

        let batch = encodings.len();
        let mut ids = Vec::with_capacity(batch * max_len);
        let mut mask = Vec::with_capacity(batch * max_len);
        let mut types = Vec::with_capacity(batch * max_len);
        for encoding in &encodings {
            let len = encoding.get_ids().len();
            ids.extend(encoding.get_ids().iter().map(|&v| v as i64));
            mask.extend(encoding.get_attention_mask().iter().map(|&v| v as i64));
            types.extend(encoding.get_type_ids().iter().map(|&v| v as i64));
            for _ in len..max_len {
                ids.push(pad_id);
                mask.push(0);
                types.push(0);
            }
        }

        let shape = [batch as i64, max_len as i64];
        let mut ids_tensor = Some(
            Tensor::from_array((shape, ids.into_boxed_slice()))
                .map_err(|e| anyhow!("创建 input_ids 张量失败: {e}"))?,
        );
        let mut mask_tensor = Some(
            Tensor::from_array((shape, mask.into_boxed_slice()))
                .map_err(|e| anyhow!("创建 attention_mask 张量失败: {e}"))?,
        );
        let mut types_tensor = Some(
            Tensor::from_array((shape, types.into_boxed_slice()))
                .map_err(|e| anyhow!("创建 token_type_ids 张量失败: {e}"))?,
        );

        let Some(session_mutex) = self.session() else {
            self.failed.store(true, Ordering::Release);
            return Err(anyhow!("嵌入模型会话不可用"));
        };
        let mut session = session_mutex
            .lock()
            .map_err(|e| anyhow!("嵌入会话锁中毒: {e}"))?;

        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();
        if !input_names.iter().any(|name| name == "input_ids") {
            return Err(anyhow!("嵌入模型缺少 input_ids 输入: {input_names:?}"));
        }
        let mut feeds: Vec<(String, SessionInputValue<'_>)> = Vec::with_capacity(input_names.len());
        for name in &input_names {
            match name.as_str() {
                "input_ids" => {
                    if let Some(tensor) = ids_tensor.take() {
                        feeds.push(("input_ids".to_string(), tensor.into()));
                    }
                },
                "attention_mask" => {
                    if let Some(tensor) = mask_tensor.take() {
                        feeds.push(("attention_mask".to_string(), tensor.into()));
                    }
                },
                "token_type_ids" => {
                    if let Some(tensor) = types_tensor.take() {
                        feeds.push(("token_type_ids".to_string(), tensor.into()));
                    }
                },
                _ => {},
            }
        }

        let outputs = session
            .run(feeds)
            .map_err(|e| anyhow!("嵌入推理失败: {e}"))?;
        let value: &DynValue = PREFERRED_OUTPUTS
            .iter()
            .find_map(|name| outputs.get(*name))
            .unwrap_or(&outputs[0]);
        let array = value
            .try_extract_array::<f32>()
            .map_err(|e| anyhow!("嵌入输出不是 f32 张量: {e}"))?;
        let output_shape = array.shape().to_vec();
        let data = array
            .as_slice()
            .ok_or_else(|| anyhow!("嵌入输出张量非连续布局"))?;
        let Some(&dim) = output_shape.last() else {
            return Err(anyhow!("嵌入输出张量没有维度"));
        };
        if dim != EXPECTED_DIMENSION {
            tracing::debug!(
                "[embedding] 模型输出维度 {dim} 与预期 {EXPECTED_DIMENSION} 不一致（换了模型？）"
            );
        }

        let mut result = Vec::with_capacity(batch);
        match output_shape.len() {
            // [B, seq, dim]：取每条样本的 CLS 行。
            3 => {
                let seq = output_shape[1];
                if output_shape[0] != batch || data.len() < batch * seq * dim {
                    return Err(anyhow!(
                        "嵌入输出形状异常: shape={output_shape:?} 元素数={}",
                        data.len()
                    ));
                }
                for b in 0..batch {
                    let start = b * seq * dim;
                    result.push(l2_normalize(data[start..start + dim].to_vec()));
                }
            },
            // [B, dim]：模型直接输出句向量。
            2 => {
                if output_shape[0] != batch || data.len() < batch * dim {
                    return Err(anyhow!(
                        "嵌入输出形状异常: shape={output_shape:?} 元素数={}",
                        data.len()
                    ));
                }
                for b in 0..batch {
                    let start = b * dim;
                    result.push(l2_normalize(data[start..start + dim].to_vec()));
                }
            },
            // [dim]：批大小为 1 的单条输出。
            _ if batch == 1 && data.len() >= dim => {
                result.push(l2_normalize(data[..dim].to_vec()));
            },
            _ => {
                return Err(anyhow!(
                    "不支持的嵌入输出形状: shape={output_shape:?} 批大小={batch}"
                ));
            },
        }
        Ok(result)
    }

    /// 取（必要时创建）推理会话。创建失败会缓存为 `None`，不做重复尝试。
    fn session(&self) -> Option<&Mutex<Session>> {
        self.session
            .get_or_init(|| match build_session(&self.onnx_path) {
                Ok(session) => Some(Mutex::new(session)),
                Err(e) => {
                    tracing::warn!("[embedding] 创建嵌入模型会话失败，引擎将禁用: {e}");
                    None
                },
            })
            .as_ref()
    }
}

fn build_session(onnx_path: &Path) -> Result<Session> {
    Session::builder()
        .map_err(|e| anyhow!("创建 SessionBuilder 失败: {e}"))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow!("设置优化级别失败: {e}"))?
        .with_intra_threads(1)
        .map_err(|e| anyhow!("设置线程数失败: {e}"))?
        .commit_from_file(onnx_path)
        .map_err(|e| anyhow!("加载 ONNX 模型失败 ({}): {e}", onnx_path.display()))
}

impl Embedder for EmbeddingEngine {
    fn is_enabled(&self) -> bool {
        EmbeddingEngine::is_enabled(self)
    }

    fn dimension(&self) -> Option<usize> {
        Some(EXPECTED_DIMENSION)
    }

    fn namespace(&self) -> Option<String> {
        Some(MODEL_DIR_NAME.to_string())
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        EmbeddingEngine::embed(self, text)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        EmbeddingEngine::embed_batch(self, texts)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{EXPECTED_DIMENSION, EmbeddingEngine, l2_normalize};

    #[test]
    fn l2_normalize_produces_unit_vector() {
        let v = l2_normalize(vec![3.0, 4.0]);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_keeps_zero_vector_finite() {
        let v = l2_normalize(vec![0.0, 0.0]);
        assert!(v.iter().all(|x| x.is_finite()));
    }

    fn repo_path(rel: &str) -> PathBuf {
        // 本 crate 位于 <repo>/crates/ling-chat-notes
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(rel)
    }

    /// 端到端冒烟：真加载模型并验证句向量的维度、归一化与语义区分度。
    ///
    /// 需先跑 `node scripts/download_embedding_model.mjs` 与
    /// `node scripts/ensure-onnxruntime.mjs`。**单独运行**（同一进程里
    /// `ort::init_from` 只应初始化一次）：
    /// `cargo test -p ling-chat-notes --lib --ignored embedding_smoke -- --exact --nocapture`
    #[test]
    #[ignore = "需要已下载的嵌入模型与 onnxruntime.dll"]
    fn embedding_smoke_returns_normalized_semantic_vectors() {
        // 只有 Windows 的 load-dynamic 模式暴露 ort::init_from；其余平台静态链接，
        // 无需也不能手动初始化。
        #[cfg(target_os = "windows")]
        {
            let dll = repo_path("src-tauri/binaries/onnxruntime.dll");
            assert!(dll.exists(), "onnxruntime.dll 不存在: {}", dll.display());
            // 已初始化过会返回 Err（ort 内部库句柄已被消费），此时运行时其实已就绪，
            // 继续走即可，不要重复初始化。
            if let Err(e) = ort::init_from(&dll) {
                eprintln!("ort::init_from 跳过（可能已初始化）: {e}");
            }
        }

        let model_dir = repo_path("data/third_party/embedding_model_bge_small_zh");
        let engine = EmbeddingEngine::load(&model_dir, true);
        assert!(
            engine.is_enabled(),
            "引擎未启用，检查模型目录: {}",
            model_dir.display()
        );

        let time = engine.embed("现在几点了").expect("嵌入失败");
        assert_eq!(time.len(), EXPECTED_DIMENSION);
        let norm = time.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "向量未归一化: norm={norm}");

        let cloth = engine.embed("给角色换一身衣服").expect("嵌入失败");
        let clock_desc = engine.embed("获取当前时间").expect("嵌入失败");
        let clothes_desc = engine.embed("设置角色穿着的服装").expect("嵌入失败");

        let to_clock = super::super::cosine_similarity(&time, &clock_desc);
        let to_clothes = super::super::cosine_similarity(&time, &clothes_desc);
        println!("现在几点 <-> 获取当前时间: {to_clock:.4}");
        println!("现在几点 <-> 设置角色穿着的服装: {to_clothes:.4}");
        assert!(
            to_clock > to_clothes,
            "语义区分度不足: 时间 {to_clock:.4} 未高于服装 {to_clothes:.4}"
        );
        assert!(
            super::super::cosine_similarity(&cloth, &clothes_desc)
                > super::super::cosine_similarity(&cloth, &clock_desc),
            "服装意图未匹配到服装描述"
        );

        // 批量接口与单条接口必须得到同一方向的向量。
        let batch = engine
            .embed_batch(&["现在几点了".to_string(), "给角色换一身衣服".to_string()])
            .expect("批量嵌入失败");
        assert_eq!(batch.len(), 2);
        for (single, batch_vector) in [&time, &cloth].iter().zip(batch.iter()) {
            let norm = batch_vector.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-3, "批量向量未归一化: norm={norm}");
            let similarity = super::super::cosine_similarity(single, batch_vector);
            // 批内补齐会让量化模型产生极小的数值差异，方向一致即可。
            assert!(similarity > 0.98, "批量与单条结果不一致: {similarity}");
        }
    }
}
