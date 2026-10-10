//! 内置本地嵌入模型：BAAI/bge-small-zh-v1.5 的中文量化版（int4 transformer + int2 词嵌入表）。
//!
//! ## 产物（`data/third_party/bge_small_zh_v1_5/`，共约 11MB）
//! - `model_int4.onnx`：Transformer 的 MatMul 全部 int4（MatMulNBits），词嵌入表已从图中
//!   摘除，改为外部输入 `inputs_embeds`（图手术脚本见 `scripts/quantize_bge_embedding.py`）；
//! - `embedding_int2.bin`：21128×512 词嵌入表，每行分成 8 组 × 64 维，每组一对 fp16
//!   `(scale, zero)` 仿射量化到 2-bit，再按行主序 LSB 优先打包（魔数 `BEKKOEMB02|…`）；
//! - `tokenizer.json.gz`：BERT WordPiece（vocab 21128），gzip 压到约 0.1MB，加载时解压进内存；
//! - `config.json`：模型元信息（512 维 / CLS 池化 / 512 上下文）。
//!
//! 推理：`input_ids + attention_mask + token_type_ids + inputs_embeds` 四输入进 ONNX Runtime
//! （CPU），输出 `last_hidden_state` 取 **CLS（序列首位）** 向量 + L2 归一化（bge 官方池化），
//! 与远程 embeddings 接口的返回对齐（`MemoryService` 直接用余弦/点积检索）。
//!
//! ONNX Runtime 为 CPU 密集推理且 `ort::Session` 需独占，这里复刻本地 TTS 的
//! 「互斥锁 + spawn_blocking」模式：`embed`/`embed_batch` 内部把 Session 的
//! `run` 挪到阻塞线程池执行，不卡 Tokio worker。

use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;
use tokenizers::tokenizer::{TruncationDirection, TruncationParams, TruncationStrategy};

/// 当前使用的本地嵌入模型名（写入 `memory_embeddings.model` 的标识）。
pub const LOCAL_MODEL_NAME: &str = "bge-small-zh-v1.5";

/// 模型子目录名（挂在 `data/third_party/` 下）。
pub const MODEL_DIR: &str = "bge_small_zh_v1_5";
/// 组件文件名。
const MODEL_FILE: &str = "model_int4.onnx";
const TOKENIZER_FILE_GZ: &str = "tokenizer.json.gz";
const EMBED_FILE: &str = "embedding_int2.bin";

/// 词嵌入维度与输出维度（bge-small-zh-v1.5 隐藏层 512）。
const EMBED_DIM: usize = 512;
/// 模型的上下文长度上限（bge-small-zh-v1.5 max_position_embeddings = 512）。
const MAX_SEQ_LEN: usize = 512;

/// 推理时的 ORT 内部线程数（内核级并行交给批量维度，这里给个温和的数值，
/// 避免与情绪分类 / VAD / 本地 TTS 的多个 session 抢 CPU）。
const INTRA_THREADS: usize = 2;

// ─── 内置模型路径解析 ─────────────────────────────────────────

/// 内置模型完整目录（`data_dir/third_party/bge_small_zh_v1_5`），与 VAD / 情绪模型同策略。
pub fn bundled_dir() -> std::path::PathBuf {
    crate::data_dir::get_data_dir()
        .join("third_party")
        .join(MODEL_DIR)
}

/// 三个必需文件是否齐备（同步、极便宜的文件存在性检查；供 `MemoryService::is_configured` 使用）。
pub fn is_bundled_available() -> bool {
    let dir = bundled_dir();
    dir.join(MODEL_FILE).is_file()
        && dir.join(TOKENIZER_FILE_GZ).is_file()
        && dir.join(EMBED_FILE).is_file()
}

// ─── int2 词嵌入表 ────────────────────────────────────────────

/// 21128×512 的 2-bit 词嵌入表（含每行 8 组 × fp16(scale, zero)）。
///
/// 文件布局（与 `scripts/quantize_bge_embedding.py` 的写入方保持一致）：
/// ```text
/// BEKKOEMB02|21128|512|16\n      <- 魔数|词表|维度|每行 scale 数（组数×2）
/// [vocab × 16 × 2B  fp16 scale/zero 混合列表]
/// [vocab × 512 × 2 bits 打包数据]
/// ```
struct EmbedTable {
    bits: usize,
    vocab: usize,
    /// 每行分组数（512 维 / 64 = 8）。
    groups: usize,
    /// 每组的向量维度（64）。
    group_size: usize,
    /// 每组一对 (scale, zero)，fp16 在内存里解成 f32。
    scales: Vec<f32>,
    /// 打包后的 2-bit 数据（bits<8 时才有意义）。
    data: Vec<u8>,
}

fn f16_to_f32(value: u16) -> f32 {
    let sign = if value & 0x8000 != 0 { -1.0f32 } else { 1.0f32 };
    let exponent = (value >> 10) & 0x1f;
    let mantissa = value & 0x3ff;
    if exponent == 0 {
        sign * (mantissa as f32) * 2f32.powi(-24)
    } else if exponent == 0x1f {
        if mantissa == 0 {
            sign * f32::INFINITY
        } else {
            f32::NAN
        }
    } else {
        sign * (1.0 + (mantissa as f32) / 1024.0) * 2f32.powi(exponent as i32 - 15)
    }
}

impl EmbedTable {
    fn load(path: &Path) -> Result<Self> {
        let raw =
            std::fs::read(path).with_context(|| format!("读取嵌入表失败: {}", path.display()))?;
        let nl = raw
            .iter()
            .position(|&b| b == b'\n')
            .context("嵌入表缺少头行")?;
        let header = std::str::from_utf8(&raw[..nl]).context("嵌入表头行非法 UTF-8")?;
        let mut parts = header.split('|');
        let magic = parts.next().context("嵌入表头缺魔数")?;
        let bits: usize = magic
            .strip_prefix("BEKKOEMB")
            .context("嵌入表魔数不匹配")?
            .parse()
            .context("嵌入表位宽非法")?;
        let vocab: usize = parts
            .next()
            .context("缺词表大小")?
            .parse()
            .context("词表大小非法")?;
        let dim: usize = parts
            .next()
            .context("缺向量维度")?
            .parse()
            .context("维度非法")?;
        let scales_per_row: usize = parts
            .next()
            .context("缺每行 scale 数")?
            .parse()
            .context("每行 scale 数非法")?;
        if dim != EMBED_DIM {
            bail!("嵌入表维度 {dim} 与预期 {EMBED_DIM} 不符");
        }
        if bits != 2 {
            bail!("内置嵌入表位宽 {bits} 未实现（仅支持 2）");
        }
        // bits=2 时每行 scale 数为 组数×2（每组一组 scale+zero）
        if scales_per_row % 2 != 0 {
            bail!("每行 scale 数 {scales_per_row} 应能被 2 整除");
        }
        let groups = scales_per_row / 2;
        if dim % groups != 0 {
            bail!("维度 {dim} 不能被组数 {groups} 整除");
        }
        let group_size = dim / groups;

        let body = &raw[nl + 1..];
        let scale_bytes = vocab * scales_per_row * 2;
        if body.len() < scale_bytes {
            bail!("嵌入表文件被截断：缺 scale 数据");
        }
        let mut scales = Vec::with_capacity(vocab * scales_per_row);
        for chunk in body[..scale_bytes].chunks_exact(2) {
            let value = u16::from_le_bytes([chunk[0], chunk[1]]);
            scales.push(f16_to_f32(value));
        }
        let packed_len = vocab * dim * bits / 8;
        if body.len() < scale_bytes + packed_len {
            bail!("嵌入表文件被截断：缺打包数据");
        }
        let data = body[scale_bytes..scale_bytes + packed_len].to_vec();

        Ok(Self {
            bits,
            vocab,
            groups,
            group_size,
            scales,
            data,
        })
    }

    /// 回读元素 `elem`（行主序下的第 `elem` 个维度分量）对应的量化码。
    /// 位流按行主序、元素 LSB 优先，字节内 MSB 优先打包（与 Python 端 `pack_nbit` 一致）。
    fn unpack(&self, elem: usize) -> u32 {
        let p = elem * self.bits;
        let mut value = 0u32;
        for bit in 0..self.bits {
            let pos = p + bit;
            let flag = ((self.data[pos >> 3] >> (7 - (pos & 7))) & 1) as u32;
            value |= flag << bit;
        }
        value
    }

    /// 把若干个 token id 解出成连续浮点向量（行主序），供 `inputs_embeds` 使用。
    fn embeds_for_ids(&self, ids: &[u32], out: &mut [f32]) {
        for (n, &id) in ids.iter().enumerate() {
            let id = id as usize;
            let row = &mut out[n * EMBED_DIM..(n + 1) * EMBED_DIM];
            if id >= self.vocab {
                continue;
            }
            let row_base = id * EMBED_DIM;
            for group in 0..self.groups {
                let scale = self.scales[(id * self.groups + group) * 2];
                let zero = self.scales[(id * self.groups + group) * 2 + 1];
                let group_base = group * self.group_size;
                for offset in 0..self.group_size {
                    let code = self.unpack(row_base + group_base + offset);
                    row[group_base + offset] = code as f32 * scale + zero;
                }
            }
        }
    }
}

// ─── 本地嵌入器 ───────────────────────────────────────────────

/// 一批待推理的输入（已按最大长度补齐）。
struct BatchInputs {
    batch: usize,
    max_len: usize,
    input_ids: Vec<i64>,
    attention_mask: Vec<i64>,
    /// 单句编码，恒为 0（BERT 图仍保留该输入用于 token_type 嵌入查表）。
    token_type_ids: Vec<i64>,
    /// `[batch, max_len, EMBED_DIM]` 的行主序浮点。
    inputs_embeds: Vec<f32>,
}

/// 打完补丁后的模型图输入（固定顺序），生成工件时保证不变。
const INPUT_IDS: &str = "input_ids";
const ATTENTION_MASK: &str = "attention_mask";
const TOKEN_TYPE_IDS: &str = "token_type_ids";
const INPUTS_EMBEDS: &str = "inputs_embeds";

/// 内置本地嵌入器（线程安全：Session 用互斥锁串行化，推理在阻塞线程池执行）。
pub struct LocalEmbedder {
    tokenizer: Tokenizer,
    table: EmbedTable,
    session: Arc<Mutex<Session>>,
}

impl LocalEmbedder {
    /// 阻塞式加载（约 300ms）：读 3.4MB 嵌入表 + 解压 tokenizer + 建 ONNX Session。
    /// 调用方应放在 `spawn_blocking` 里执行。
    pub fn load(model_dir: &Path) -> Result<Self> {
        // ONNX Runtime 不可用（onnxruntime.dll 缺失）时直接降级，绝不创建 Session
        // （ort 在 dll 缺失时首次创建 Session 会 panic，导致应用崩溃）。
        if !crate::utils::onnx::onnx_available() {
            bail!("ONNX Runtime 不可用（onnxruntime.dll 缺失），内置嵌入模型停用");
        }
        let onnx_path = model_dir.join(MODEL_FILE);
        let tokenizer_gz = model_dir.join(TOKENIZER_FILE_GZ);
        let embed_path = model_dir.join(EMBED_FILE);
        if !onnx_path.is_file() {
            bail!("内置嵌入模型缺失: {}", onnx_path.display());
        }
        if !tokenizer_gz.is_file() {
            bail!("内置嵌入词表缺失: {}", tokenizer_gz.display());
        }
        if !embed_path.is_file() {
            bail!("内置嵌入词嵌入表缺失: {}", embed_path.display());
        }

        let session = Session::builder()
            .map_err(|e| anyhow!("创建 SessionBuilder 失败: {e}"))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("设置优化级别失败: {e}"))?
            .with_intra_threads(INTRA_THREADS)
            .map_err(|e| anyhow!("设置线程数失败: {e}"))?
            .commit_from_file(&onnx_path)
            .map_err(|e| anyhow!("加载内置嵌入模型失败 ({}): {e}", onnx_path.display()))?;
        let session = Arc::new(Mutex::new(session));

        // gzip 解压 tokenizer.json 再交给 tokenizers 解析（不落临时文件）。
        let gz_bytes = std::fs::read(&tokenizer_gz)
            .with_context(|| format!("读取 tokenizer 失败: {}", tokenizer_gz.display()))?;
        let mut decoder = flate2::read::GzDecoder::new(&gz_bytes[..]);
        let mut tokenizer_json = Vec::with_capacity(gz_bytes.len() * 3);
        decoder
            .read_to_end(&mut tokenizer_json)
            .context("解压 tokenizer.json.gz 失败")?;
        let mut tokenizer = Tokenizer::from_bytes(&tokenizer_json)
            .map_err(|e| anyhow!("解析 tokenizer 失败: {e}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_SEQ_LEN,
                stride: 0,
                strategy: TruncationStrategy::LongestFirst,
                direction: TruncationDirection::Right,
            }))
            .map_err(|e| anyhow!("设置分词截断上限失败: {e}"))?;

        let table = EmbedTable::load(&embed_path)?;
        tracing::info!(
            "已加载内置嵌入模型: {} (vocab={}, bits={}, groups={})",
            onnx_path.display(),
            table.vocab,
            table.bits,
            table.groups
        );
        Ok(Self {
            tokenizer,
            table,
            session,
        })
    }

    /// 本地模型的稳定标识（写入记忆表，供按模型过滤旧向量）。
    pub fn model_name(&self) -> &str {
        LOCAL_MODEL_NAME
    }

    /// 嵌入一段文本，返回 L2 归一化向量。
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let texts = vec![text.to_string()];
        let mut vectors = self.embed_batch(&texts).await?;
        vectors.pop().context("内置嵌入返回空结果")
    }

    /// 嵌入多段文本（一次 ONNX 批量推理），返回与输入同序的向量列表。
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let batch = self.prepare_batch(texts)?;
        let mut pooled = self.run_batched(batch).await?;
        pooled.truncate(texts.len());
        Ok(pooled)
    }

    /// 分词 + 词嵌入表查表，构造 ONNX 四输入。
    fn prepare_batch(&self, texts: &[String]) -> Result<BatchInputs> {
        let mut encodings = Vec::with_capacity(texts.len());
        let mut max_len = 0usize;
        for text in texts {
            let encoding = self
                .tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| anyhow!("本地分词失败: {e}"))?;
            max_len = max_len.max(encoding.get_ids().len());
            encodings.push(encoding);
        }
        if max_len == 0 {
            bail!("内置嵌入：输入文本分词后为空");
        }

        let mut input_ids = Vec::with_capacity(texts.len() * max_len);
        let mut attention_mask = Vec::with_capacity(texts.len() * max_len);
        let mut inputs_embeds = vec![0f32; texts.len() * max_len * EMBED_DIM];
        for (index, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            self.table.embeds_for_ids(
                ids,
                &mut inputs_embeds[index * max_len * EMBED_DIM..(index + 1) * max_len * EMBED_DIM],
            );
            for &id in ids.iter() {
                input_ids.push(id as i64);
                attention_mask.push(1i64);
            }
            for _ in ids.len()..max_len {
                input_ids.push(0i64);
                attention_mask.push(0i64);
            }
        }
        // 单句编码，token_type 全 0（图内 token_type 嵌入查表恒取第 0 行）。
        let token_type_ids = vec![0i64; texts.len() * max_len];

        Ok(BatchInputs {
            batch: texts.len(),
            max_len,
            input_ids,
            attention_mask,
            token_type_ids,
            inputs_embeds,
        })
    }

    /// 把批量输入丢给阻塞线程池跑 ONNX，流水线后做 CLS 池化 + L2 归一化。
    async fn run_batched(&self, batch: BatchInputs) -> Result<Vec<Vec<f32>>> {
        let session = Arc::clone(&self.session);

        tokio::task::spawn_blocking(move || {
            let mut guard = session
                .lock()
                .map_err(|_| anyhow!("内置嵌入 Session 锁被污染"))?;
            let ids_tensor = Tensor::from_array((
                [batch.batch as i64, batch.max_len as i64],
                batch.input_ids.into_boxed_slice(),
            ))
            .context("创建 input_ids 张量失败")?;
            let mask_tensor = Tensor::from_array((
                [batch.batch as i64, batch.max_len as i64],
                batch.attention_mask.into_boxed_slice(),
            ))
            .context("创建 attention_mask 张量失败")?;
            let type_tensor = Tensor::from_array((
                [batch.batch as i64, batch.max_len as i64],
                batch.token_type_ids.into_boxed_slice(),
            ))
            .context("创建 token_type_ids 张量失败")?;
            let embeds_tensor = Tensor::from_array((
                [batch.batch as i64, batch.max_len as i64, EMBED_DIM as i64],
                batch.inputs_embeds.into_boxed_slice(),
            ))
            .context("创建 inputs_embeds 张量失败")?;

            let outputs = guard
                .run(ort::inputs![
                    INPUT_IDS => ids_tensor,
                    ATTENTION_MASK => mask_tensor,
                    TOKEN_TYPE_IDS => type_tensor,
                    INPUTS_EMBEDS => embeds_tensor,
                ])
                .context("内置嵌入 ONNX 推理失败")?;

            let hidden = outputs[0]
                .try_extract_array::<f32>()
                .context("内置嵌入输出类型不是 f32")?;
            // [batch, seq, dim]
            let shape = hidden.shape().to_vec();
            if shape.len() != 3 || shape[2] != EMBED_DIM as usize {
                bail!("内置嵌入输出形状异常: {shape:?}");
            }
            let slice = hidden.as_slice().context("内置嵌入输出非连续布局")?;

            // bge-small-zh-v1.5 用 CLS 池化：取序列首位（[CLS]）向量，无需 mask 加权。
            let mut result = Vec::with_capacity(batch.batch);
            for row in 0..batch.batch {
                let base = row * batch.max_len * EMBED_DIM;
                let mut vector = slice[base..base + EMBED_DIM].to_vec();
                let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for v in &mut vector {
                        *v /= norm;
                    }
                }
                result.push(vector);
            }
            Result::<_, anyhow::Error>::Ok(result)
        })
        .await
        .context("内置嵌入推理线程失败")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("data")
            .join("third_party")
            .join("bge_small_zh_v1_5")
            .join("embedding_int2.bin")
    }

    /// 用 Python（`scripts/quantize_bge_embedding.py` 同款算法、同 fp32 链）产出的
    /// 参考值核对 Rust 的位流解码：表头 / f16 scale / LSB-first 解包 / 分组仿射。
    /// 数据来自真实产物 `data/third_party/bge_small_zh_v1_5/embedding_int2.bin`。
    #[test]
    fn decode_matches_python_reference() {
        let table = EmbedTable::load(&table_path()).expect("嵌入表加载失败");
        assert_eq!(table.bits, 2);
        assert_eq!(table.vocab, 21_128);
        assert_eq!(table.groups, 8);
        assert_eq!(table.group_size, 64);

        let ids = [0u32, 2, 1000, 4999, 12_345, 21_127];
        let dims = [0usize, 1, 63, 64, 65, 127, 128, 511];
        // 参考值（python 严格 f32 链：code * scale + zero，与 Rust 逐位一致）
        let expected: [[f32; 8]; 6] = [
            [
                0.051879883,
                -0.08874512,
                0.051879883,
                0.038269043,
                0.038269043,
                -0.022979736,
                0.017211914,
                0.010986328,
            ],
            [
                -0.053710938,
                -0.053710938,
                -0.053710938,
                0.013671875,
                0.013671875,
                0.013671875,
                0.027038574,
                0.022277832,
            ],
            [
                -0.0524292,
                0.05456543,
                0.05456543,
                -0.09118652,
                0.11608887,
                0.012451172,
                -0.15942383,
                -0.05480957,
            ],
            [
                -0.050720215,
                -0.050720215,
                -0.050720215,
                0.06628418,
                -0.07043457,
                -0.07043457,
                -0.045898438,
                0.08691406,
            ],
            [
                -0.02709961,
                -0.02709961,
                -0.02709961,
                0.037963867,
                0.12335205,
                0.037963867,
                -0.066467285,
                0.1906128,
            ],
            [
                -0.061523438,
                -0.061523438,
                -0.061523438,
                0.022949219,
                0.022949219,
                0.022949219,
                0.02178955,
                0.020385742,
            ],
        ];

        let mut out = vec![0f32; ids.len() * EMBED_DIM];
        table.embeds_for_ids(&ids, &mut out);
        for (i, _) in ids.iter().enumerate() {
            for (k, &d) in dims.iter().enumerate() {
                let got = out[i * EMBED_DIM + d];
                let want = expected[i][k];
                assert!(
                    (got - want).abs() <= 1e-6,
                    "token {} dim {}: got {got}, want {want}",
                    ids[i],
                    d
                );
            }
        }

        // 越界 id 按 0 处理（不应 panic/越界写）
        let mut garbage = vec![0.0f32; EMBED_DIM];
        table.embeds_for_ids(&[u32::MAX - 1], &mut garbage);
        assert_eq!(garbage, vec![0.0f32; EMBED_DIM]);
    }
}
