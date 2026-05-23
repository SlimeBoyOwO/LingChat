//! 情绪分类器（ONNX 版）。移植自 Python `core/emotion/classifier.py`。
//!
//! - 模型：BERT 字符级分词 + 线性分类头，最大 seq_len=128
//! - 词表：`vocab.txt`（每行一 token，id 按行号计）
//! - 标签映射：`label_mapping.json` -> `id2label` / `label2id`
//! - 推理后端：`ort`（ONNX Runtime，与 Python 侧一致）

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use ort::{session::Session, value::Tensor};

const MAX_SEQ_LEN: usize = 128;
const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.08;

/// 情绪预测结果。对应 Python 版 `predict()` 返回 dict。
#[derive(Debug, Clone)]
pub struct EmotionPrediction {
    pub label: String,
    pub confidence: f32,
    pub top3: Vec<(String, f32)>,
    /// 模型被禁用或未加载时置 true（此时 `label` 即为输入文本原样）
    pub disabled: bool,
    /// 置信度过低时附带的提示
    pub warning: Option<String>,
}

impl EmotionPrediction {
    fn passthrough(text: &str, disabled: bool) -> Self {
        Self {
            label: text.to_string(),
            confidence: 1.0,
            top3: vec![(text.to_string(), 1.0)],
            disabled,
            warning: None,
        }
    }
}

#[derive(serde::Deserialize)]
struct LabelMappingFile {
    id2label: HashMap<String, String>,
    label2id: HashMap<String, i64>,
}

pub struct EmotionClassifier {
    session: Option<Mutex<Session>>,
    vocab: HashMap<String, i64>,
    id2label: HashMap<i64, String>,
    label2id: HashMap<String, i64>,
    /// 若输入文本本身就是合法标签，是否直接透传（对应 ENABLE_DIRECT_EMOTION_CLASSIFIER）
    direct_return_if_label: bool,
    unk_id: i64,
    cls_id: i64,
    sep_id: i64,
    pad_id: i64,
}

impl EmotionClassifier {
    /// 禁用状态的空实例（`predict` 全部透传）。
    pub fn disabled() -> Self {
        Self {
            session: None,
            vocab: HashMap::new(),
            id2label: HashMap::new(),
            label2id: HashMap::new(),
            direct_return_if_label: false,
            unk_id: 100,
            cls_id: 101,
            sep_id: 102,
            pad_id: 0,
        }
    }

    /// 从 `model_dir` 加载模型（目录内需有 `model.onnx`、`vocab.txt`、`label_mapping.json`）。
    pub fn load<P: AsRef<Path>>(model_dir: P) -> Result<Self> {
        let dir: PathBuf = model_dir.as_ref().to_path_buf();
        let onnx_path = dir.join("model.onnx");
        let vocab_path = dir.join("vocab.txt");
        let mapping_path = dir.join("label_mapping.json");

        if !onnx_path.exists() {
            return Err(anyhow!("ONNX 模型文件不存在: {}", onnx_path.display()));
        }
        if !vocab_path.exists() {
            return Err(anyhow!("词表文件不存在: {}", vocab_path.display()));
        }
        if !mapping_path.exists() {
            return Err(anyhow!("标签映射文件不存在: {}", mapping_path.display()));
        }

        let session = Session::builder()
            .map_err(|e| anyhow!("创建 SessionBuilder 失败: {e}"))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("设置优化级别失败: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("设置线程数失败: {e}"))?
            .commit_from_file(&onnx_path)
            .map_err(|e| anyhow!("加载 ONNX 模型失败 ({}): {e}", onnx_path.display()))?;

        // 词表：每行一个 token，行号即 id
        let vocab_text = std::fs::read_to_string(&vocab_path)
            .with_context(|| format!("读取 vocab 失败: {}", vocab_path.display()))?;
        let mut vocab: HashMap<String, i64> = HashMap::with_capacity(32_000);
        for (idx, line) in vocab_text.lines().enumerate() {
            vocab.insert(line.to_string(), idx as i64);
        }

        let unk_id = *vocab.get("[UNK]").unwrap_or(&100);
        let cls_id = *vocab.get("[CLS]").unwrap_or(&101);
        let sep_id = *vocab.get("[SEP]").unwrap_or(&102);
        let pad_id = *vocab.get("[PAD]").unwrap_or(&0);

        // label_mapping.json
        let mapping_raw = std::fs::read_to_string(&mapping_path)
            .with_context(|| format!("读取 label_mapping 失败: {}", mapping_path.display()))?;
        let mapping: LabelMappingFile =
            serde_json::from_str(&mapping_raw).context("解析 label_mapping.json 失败")?;
        let id2label: HashMap<i64, String> = mapping
            .id2label
            .into_iter()
            .map(|(k, v)| {
                k.parse::<i64>()
                    .map(|id| (id, v))
                    .map_err(|e| anyhow!("id2label 键不是整数: {k} ({e})"))
            })
            .collect::<Result<_>>()?;

        tracing::info!(
            "已加载情绪分类模型: {} (标签数={})",
            onnx_path.display(),
            id2label.len()
        );

        Ok(Self {
            session: Some(Mutex::new(session)),
            vocab,
            id2label,
            label2id: mapping.label2id,
            direct_return_if_label: std::env::var("ENABLE_DIRECT_EMOTION_CLASSIFIER")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            unk_id,
            cls_id,
            sep_id,
            pad_id,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.session.is_some()
    }

    /// 预测文本情绪。`confidence_threshold` 小于此值会返回 "不确定"。
    pub fn predict(&self, text: &str, confidence_threshold: Option<f32>) -> EmotionPrediction {
        let threshold = confidence_threshold.unwrap_or(DEFAULT_CONFIDENCE_THRESHOLD);
        let Some(session) = self.session.as_ref() else {
            return EmotionPrediction::passthrough(text, true);
        };

        if self.direct_return_if_label && self.label2id.contains_key(text) {
            tracing::debug!("输入文本 '{text}' 已是合法情感标签，直接返回");
            return EmotionPrediction::passthrough(text, false);
        }

        // 与 Python 版一致的特例：含"撒娇"直接映射到"调皮"
        if text.contains("撒娇") {
            return EmotionPrediction {
                label: "调皮".to_string(),
                confidence: 1.0,
                top3: vec![("调皮".to_string(), 1.0)],
                disabled: false,
                warning: None,
            };
        }

        let mut session = match session.lock() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("无法获取 session 锁: {e}");
                return EmotionPrediction::passthrough(text, false);
            }
        };

        match self.run_inference(&mut session, text, threshold) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("情绪预测错误: {e}");
                EmotionPrediction::passthrough(text, false)
            }
        }
    }

    fn run_inference(
        &self,
        session: &mut Session,
        text: &str,
        threshold: f32,
    ) -> Result<EmotionPrediction> {
        let (input_ids, attention_mask) = self.tokenize(text);

        let mask_f32: Vec<f32> = attention_mask.iter().map(|&v| v as f32).collect();

        let ids_tensor =
            Tensor::from_array(([1i64, MAX_SEQ_LEN as i64], input_ids.into_boxed_slice()))
                .context("创建 input_ids tensor 失败")?;
        let mask_tensor =
            Tensor::from_array(([1i64, MAX_SEQ_LEN as i64], mask_f32.into_boxed_slice()))
                .context("创建 attention_mask tensor 失败")?;

        let input_names: Vec<String> = session.inputs().iter().map(|o| o.name().to_string()).collect();

        let outputs = session
            .run(ort::inputs![
                input_names[0].as_str() => ids_tensor,
                input_names[1].as_str() => mask_tensor,
            ])
            .context("ONNX 推理失败")?;

        let logits = outputs[0]
            .try_extract_array::<f32>()
            .context("输出张量类型不是 f32")?;
        let logits_slice = logits
            .as_slice()
            .ok_or_else(|| anyhow!("logits 非连续布局"))?;
        let probs = softmax(logits_slice);

        let (pred_id, pred_prob) = probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, v)| (i as i64, *v))
            .unwrap_or((0, 0.0));

        let top3 = top_k(&probs, 3, &self.id2label);

        if pred_prob < threshold {
            tracing::debug!(
                "情绪识别置信度低: {text} -> 不确定 ({:.2}%)",
                pred_prob * 100.0
            );
            return Ok(EmotionPrediction {
                label: "不确定".to_string(),
                confidence: pred_prob,
                top3,
                disabled: false,
                warning: Some(format!("置信度低于阈值({:.0}%)", threshold * 100.0)),
            });
        }

        let label = self
            .id2label
            .get(&pred_id)
            .cloned()
            .unwrap_or_else(|| String::new());
        tracing::debug!("情绪识别: {text} -> {label} ({:.2}%)", pred_prob * 100.0);
        Ok(EmotionPrediction {
            label,
            confidence: pred_prob,
            top3,
            disabled: false,
            warning: None,
        })
    }

    /// BERT 字符级分词：chars -> ids，前加 [CLS]，末加 [SEP]，Pad 到 MAX_SEQ_LEN。
    fn tokenize(&self, text: &str) -> (Vec<i64>, Vec<i64>) {
        let mut ids: Vec<i64> = Vec::with_capacity(MAX_SEQ_LEN);
        ids.push(self.cls_id);

        let chars: Vec<&str> = text.graphemes_iter();
        let max_body = MAX_SEQ_LEN - 2;
        for g in chars.into_iter().take(max_body) {
            let id = self.vocab.get(g).copied().unwrap_or(self.unk_id);
            ids.push(id);
        }
        ids.push(self.sep_id);

        let mut mask: Vec<i64> = vec![1; ids.len()];
        let pad_len = MAX_SEQ_LEN - ids.len();
        ids.extend(std::iter::repeat(self.pad_id).take(pad_len));
        mask.extend(std::iter::repeat(0i64).take(pad_len));

        (ids, mask)
    }
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = logits.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    if sum <= 0.0 {
        return vec![0.0; logits.len()];
    }
    exps.into_iter().map(|v| v / sum).collect()
}

fn top_k(probs: &[f32], k: usize, id2label: &HashMap<i64, String>) -> Vec<(String, f32)> {
    let mut idx: Vec<usize> = (0..probs.len()).collect();
    idx.sort_by(|a, b| {
        probs[*b]
            .partial_cmp(&probs[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    idx.into_iter()
        .take(k)
        .map(|i| {
            let label = id2label
                .get(&(i as i64))
                .cloned()
                .unwrap_or_else(|| String::new());
            (label, probs[i])
        })
        .collect()
}

/// 极简 grapheme 迭代器：避免引入 `unicode-segmentation`。
/// 对汉字/假名足够（每个 scalar value 一段），不处理组合字符序列。
trait GraphemeSimple {
    fn graphemes_iter(&self) -> Vec<&str>;
}

impl GraphemeSimple for str {
    fn graphemes_iter(&self) -> Vec<&str> {
        let mut out = Vec::with_capacity(self.len());
        let mut start = 0;
        for (i, _) in self.char_indices().skip(1) {
            out.push(&self[start..i]);
            start = i;
        }
        if start < self.len() {
            out.push(&self[start..]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn model_dir() -> Option<PathBuf> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let candidate = manifest_dir
            .parent()?
            .join("ling_chat_python")
            .join("third_party")
            .join("emotion_model_19emo");
        candidate.join("model.onnx").exists().then_some(candidate)
    }

    #[test]
    fn passthrough_when_disabled() {
        let clf = EmotionClassifier::disabled();
        let p = clf.predict("任何文本", None);
        assert!(p.disabled);
        assert_eq!(p.label, "任何文本");
    }

    #[test]
    fn load_and_predict_real_model() {
        let Some(dir) = model_dir() else {
            eprintln!("skip: emotion_model_19emo 未在 ling_chat_python/third_party 下");
            return;
        };
        let clf = EmotionClassifier::load(&dir).expect("load");
        assert!(clf.is_enabled());
        let p = clf.predict("开心", None);
        eprintln!("predict 开心 -> {} ({:.4})", p.label, p.confidence);
        assert!(!p.label.is_empty());
        assert!((0.0..=1.0).contains(&p.confidence));
    }
}
