//! 与基座对齐的文本编码器。
//!
//! 结构照 `matcha-icefall-zh-baker` 的真实张量清单还原：词嵌入、三层 k=5 的
//! prenet 卷积、六层卷积投影的自注意力与 k=3 的前馈卷积，最后分出 mel 先验与
//! 时长两支。归一化没有可学习参数，按无仿射的 LayerNorm 处理。
//!
//! 张量名与基座一致，可以直接把 ONNX 里的权重按名字填进来。

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::{Conv1d, Conv1dConfig, Embedding, Module, VarBuilder, VarMap};

/// 基座编码器超参数。
#[derive(Debug, Clone)]
pub struct BaseEncoderConfig {
    /// 词表大小。
    pub n_vocab: usize,
    /// 隐层维度。
    pub hidden: usize,
    /// Transformer 层数。
    pub layers: usize,
    /// 前馈隐层维度。
    pub ffn_hidden: usize,
    /// mel 通道数。
    pub n_mels: usize,
    /// 时长预测器隐层维度。
    pub duration_hidden: usize,
}

impl Default for BaseEncoderConfig {
    fn default() -> Self {
        Self {
            n_vocab: 2069,
            hidden: 192,
            layers: 6,
            ffn_hidden: 768,
            n_mels: 80,
            duration_hidden: 256,
        }
    }
}

struct Attention {
    query: Conv1d,
    key: Conv1d,
    value: Conv1d,
    output: Conv1d,
}

impl Attention {
    fn new(hidden: usize, vb: VarBuilder) -> Result<Self> {
        let point = Conv1dConfig::default();
        Ok(Self {
            query: candle_nn::conv1d(hidden, hidden, 1, point, vb.pp("conv_q"))?,
            key: candle_nn::conv1d(hidden, hidden, 1, point, vb.pp("conv_k"))?,
            value: candle_nn::conv1d(hidden, hidden, 1, point, vb.pp("conv_v"))?,
            output: candle_nn::conv1d(hidden, hidden, 1, point, vb.pp("conv_o"))?,
        })
    }

    /// 输入输出都是 batch 乘 hidden 乘 frames。
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (batch, hidden, frames) = xs.dims3()?;
        let heads = 2;
        let head_dim = hidden / heads;
        let shape = (batch, heads, head_dim, frames);

        let query = self.query.forward(xs)?.reshape(shape)?;
        let key = self.key.forward(xs)?.reshape(shape)?;
        let value = self.value.forward(xs)?.reshape(shape)?;

        // 先在频域上做点积：按帧对齐后逐 head 相乘。
        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = query
            .transpose(2, 3)?
            .contiguous()?
            .matmul(&key)?
            .affine(scale, 0.0)?;
        let weights = candle_nn::ops::softmax(&scores, 3)?;
        let context = weights
            .matmul(&value.transpose(2, 3)?.contiguous()?)?
            .transpose(2, 3)?
            .contiguous()?
            .reshape((batch, hidden, frames))?;
        self.output.forward(&context)
    }
}

struct FeedForward {
    conv1: Conv1d,
    conv2: Conv1d,
}

impl FeedForward {
    fn new(hidden: usize, ffn_hidden: usize, vb: VarBuilder) -> Result<Self> {
        let padded = Conv1dConfig {
            padding: 1,
            ..Default::default()
        };
        Ok(Self {
            conv1: candle_nn::conv1d(hidden, ffn_hidden, 3, padded, vb.pp("conv_1"))?,
            conv2: candle_nn::conv1d(ffn_hidden, hidden, 3, padded, vb.pp("conv_2"))?,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let hidden = self.conv1.forward(xs)?.relu()?;
        self.conv2.forward(&hidden)
    }
}

/// 文本编码器。
pub struct BaseEncoder {
    embedding: Embedding,
    prenet: Vec<Conv1d>,
    prenet_proj: Conv1d,
    attentions: Vec<Attention>,
    feed_forwards: Vec<FeedForward>,
    prenet_norms: Vec<AffineNorm>,
    block_norms: Vec<(AffineNorm, AffineNorm)>,
    proj_m: Conv1d,
    duration_conv1: Conv1d,
    duration_conv2: Conv1d,
    duration_proj: Conv1d,
}

impl BaseEncoder {
    /// 构造编码器，参数名与基座一致。
    pub fn new(config: &BaseEncoderConfig, vb: VarBuilder) -> Result<Self> {
        let vb = vb.pp("model").pp("encoder");

        let mut prenet = Vec::with_capacity(3);
        let padded = Conv1dConfig {
            padding: 2,
            ..Default::default()
        };
        for index in 0..3 {
            prenet.push(candle_nn::conv1d(
                config.hidden,
                config.hidden,
                5,
                padded,
                vb.pp("prenet").pp("conv_layers").pp(index),
            )?);
        }

        let mut attentions = Vec::with_capacity(config.layers);
        let mut feed_forwards = Vec::with_capacity(config.layers);
        for index in 0..config.layers {
            attentions.push(Attention::new(
                config.hidden,
                vb.pp("encoder").pp("attn_layers").pp(index),
            )?);
            feed_forwards.push(FeedForward::new(
                config.hidden,
                config.ffn_hidden,
                vb.pp("encoder").pp("ffn_layers").pp(index),
            )?);
        }

        let mut prenet_norms = Vec::with_capacity(3);
        for index in 0..3 {
            prenet_norms.push(AffineNorm::new(
                config.hidden,
                vb.pp("prenet").pp(format!("norm{index}")),
            )?);
        }

        let mut block_norms = Vec::with_capacity(config.layers);
        for index in 0..config.layers {
            block_norms.push((
                AffineNorm::new(config.hidden, vb.pp("encoder").pp(format!("norm{index}_1")))?,
                AffineNorm::new(config.hidden, vb.pp("encoder").pp(format!("norm{index}_2")))?,
            ));
        }

        let point = Conv1dConfig::default();
        let duration_padded = Conv1dConfig {
            padding: 1,
            ..Default::default()
        };

        Ok(Self {
            embedding: candle_nn::embedding(config.n_vocab, config.hidden, vb.pp("emb"))?,
            prenet_proj: candle_nn::conv1d(
                config.hidden,
                config.hidden,
                1,
                point,
                vb.pp("prenet").pp("proj"),
            )?,
            prenet,
            prenet_norms,
            block_norms,
            attentions,
            feed_forwards,
            proj_m: candle_nn::conv1d(config.hidden, config.n_mels, 1, point, vb.pp("proj_m"))?,
            duration_conv1: candle_nn::conv1d(
                config.hidden,
                config.duration_hidden,
                3,
                duration_padded,
                vb.pp("proj_w").pp("conv_1"),
            )?,
            duration_conv2: candle_nn::conv1d(
                config.duration_hidden,
                config.duration_hidden,
                3,
                duration_padded,
                vb.pp("proj_w").pp("conv_2"),
            )?,
            duration_proj: candle_nn::conv1d(
                config.duration_hidden,
                1,
                1,
                point,
                vb.pp("proj_w").pp("proj"),
            )?,
        })
    }

    /// 前向：返回 mel 先验与对数时长。
    pub fn forward(&self, tokens: &Tensor) -> Result<(Tensor, Tensor)> {
        let mut hidden = self
            .embedding
            .forward(&tokens.to_dtype(DType::U32)?)?
            .transpose(1, 2)?
            .contiguous()?;

        for (conv, norm) in self.prenet.iter().zip(&self.prenet_norms) {
            hidden = norm.forward(&conv.forward(&hidden)?.relu()?)?;
        }
        hidden = self.prenet_proj.forward(&hidden)?;

        for ((attention, feed_forward), (norm1, norm2)) in self
            .attentions
            .iter()
            .zip(&self.feed_forwards)
            .zip(&self.block_norms)
        {
            hidden = (&hidden + attention.forward(&norm1.forward(&hidden)?))?;
            hidden = (&hidden + feed_forward.forward(&norm2.forward(&hidden)?))?;
        }

        let mel = self.proj_m.forward(&hidden)?;
        let durations = self.duration_conv1.forward(&hidden)?.relu()?;
        let durations = self.duration_conv2.forward(&durations)?.relu()?;
        let durations = self.duration_proj.forward(&durations)?.squeeze(1)?;
        Ok((mel, durations))
    }
}

/// 带仿射参数的 LayerNorm，沿通道维归一化。参数在基座里是匿名常量，按顺序还原。
struct AffineNorm {
    weight: Tensor,
    bias: Tensor,
}

impl AffineNorm {
    fn new(channels: usize, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            weight: vb.get_with_hints((1, channels, 1), "weight", candle_nn::init::ONE)?,
            bias: vb.get_with_hints((1, channels, 1), "bias", candle_nn::init::ZERO)?,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let mean = xs.mean_keepdim(1)?;
        let centered = xs.broadcast_sub(&mean)?;
        let variance = centered.sqr()?.mean_keepdim(1)?;
        let normed = centered.broadcast_div(&(variance + 1e-5)?.sqrt()?)?;
        normed
            .broadcast_mul(&self.weight)?
            .broadcast_add(&self.bias)
    }
}

impl BaseEncoder {
    /// 归一化参数的顺序，与图内导出顺序一致。
    pub fn normalization_names(layers: usize) -> Vec<String> {
        let mut names = Vec::new();
        for index in 0..3 {
            for part in ["weight", "bias"] {
                names.push(format!("model.encoder.prenet.norm{index}.{part}"));
            }
        }
        for index in 0..layers {
            for slot in ["1", "2"] {
                for part in ["weight", "bias"] {
                    names.push(format!("model.encoder.encoder.norm{index}_{slot}.{part}"));
                }
            }
        }
        names
    }
}

/// 把匿名归一化常量按图内顺序填进参数表。
pub fn bind_normalizations(
    var_map: &VarMap,
    tensors: &[super::onnx_weights::OnnxTensor],
    hidden: usize,
    layers: usize,
    device: &Device,
) -> anyhow::Result<usize> {
    let mut anonymous: Vec<&super::onnx_weights::OnnxTensor> = tensors
        .iter()
        .filter(|tensor| tensor.name.starts_with("onnx::") && tensor.dims == vec![1, hidden, 1])
        .collect();
    anonymous.sort_by_key(|tensor| {
        tensor
            .name
            .rsplit('_')
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let names = BaseEncoder::normalization_names(layers);
    if anonymous.len() != names.len() {
        anyhow::bail!(
            "归一化参数数量不符：图里 {} 个，模型需要 {} 个",
            anonymous.len(),
            names.len()
        );
    }

    let guard = var_map.data();
    let guard = guard.lock().unwrap();
    for (tensor, name) in anonymous.iter().zip(names.iter()) {
        let Some(var) = guard.get(name) else {
            anyhow::bail!("参数表缺少归一化项: {name}");
        };
        let value = Tensor::from_vec(tensor.data.clone(), tensor.dims.clone(), device)?;
        var.set(&value)?;
    }

    Ok(names.len())
}

/// 把 ONNX 里的权重按名字填进参数表。
pub fn bind_weights(
    var_map: &VarMap,
    tensors: &[super::onnx_weights::OnnxTensor],
    device: &Device,
) -> Result<usize> {
    let data = var_map.data();
    let guard = data.lock().unwrap();
    let mut bound = 0;

    for tensor in tensors {
        let Some(var) = guard.get(&tensor.name) else {
            continue;
        };
        let value = Tensor::from_vec(tensor.data.clone(), tensor.dims.clone(), device)?;
        var.set(&value)?;
        bound += 1;
    }

    Ok(bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::tts::finetune::onnx_weights::read_initializers;
    use std::path::Path;

    fn base_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx")
    }

    #[test]
    fn every_encoder_tensor_has_a_slot() {
        let path = base_path();
        if !path.exists() {
            return;
        }

        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        BaseEncoder::new(&BaseEncoderConfig::default(), vb).unwrap();

        let tensors = read_initializers(&path).unwrap();
        let names: std::collections::HashSet<String> =
            var_map.data().lock().unwrap().keys().cloned().collect();

        let encoder_tensors: Vec<_> = tensors
            .iter()
            .filter(|tensor| tensor.name.starts_with("model.encoder."))
            .collect();
        assert!(!encoder_tensors.is_empty());

        for tensor in encoder_tensors {
            let var = var_map
                .data()
                .lock()
                .unwrap()
                .get(&tensor.name)
                .map(|var| var.dims().to_vec());
            assert_eq!(
                var,
                Some(tensor.dims.clone()),
                "缺失或形状不符: {}",
                tensor.name
            );
        }

        assert!(names.iter().any(|name| name.ends_with("proj_m.weight")));
    }

    #[test]
    fn binds_anonymous_normalization_weights() {
        let path = base_path();
        if !path.exists() {
            return;
        }

        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        BaseEncoder::new(&BaseEncoderConfig::default(), vb).unwrap();

        let tensors = read_initializers(&path).unwrap();
        let bound = bind_normalizations(&var_map, &tensors, 192, 6, &device).unwrap();
        assert_eq!(bound, 30);

        let weight = var_map
            .data()
            .lock()
            .unwrap()
            .get("model.encoder.prenet.norm0.weight")
            .unwrap()
            .as_tensor()
            .dims()
            .to_vec();
        assert_eq!(weight, vec![1, 192, 1]);
    }

    #[test]
    fn forward_produces_mel_and_durations() {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let encoder = BaseEncoder::new(&BaseEncoderConfig::default(), vb).unwrap();

        let tokens = Tensor::from_vec(vec![1u32, 2, 3, 4, 5], (1, 5), &device).unwrap();
        let (mel, durations) = encoder.forward(&tokens).unwrap();
        assert_eq!(mel.dims(), &[1, 80, 5]);
        assert_eq!(durations.dims(), &[1, 5]);
    }
}
