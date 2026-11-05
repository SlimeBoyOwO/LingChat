//! Matcha 声学模型的骨架。
//!
//! 只实现微调需要的那部分：文本编码器把 token 序列编成逐 token 的隐状态，
//! 时长预测器给出每个 token 占多少帧。条件流匹配解码器在后续步骤接入。
//!
//! 结构与 matcha-icefall 系列保持一致：Transformer 文本编码器 + 卷积时长预测器，
//! 激活用 ReLU、归一化用 LayerNorm、时长取对数后回归。

use candle_core::{DType, Result, Tensor};
use candle_nn::{Conv1d, Conv1dConfig, Embedding, LayerNorm, Linear, Module, VarBuilder};

/// 模型超参数。
#[derive(Debug, Clone)]
pub struct MatchaConfig {
    /// 词表大小。
    pub n_vocab: usize,
    /// mel 通道数。
    pub n_mels: usize,
    /// 隐层维度。
    pub hidden: usize,
    /// 文本编码器层数。
    pub layers: usize,
    /// 注意力头数。
    pub heads: usize,
    /// 前馈层倍数。
    pub ffn_mult: usize,
    /// 时长预测器卷积层数。
    pub duration_layers: usize,
}

impl Default for MatchaConfig {
    fn default() -> Self {
        Self {
            n_vocab: 4_000,
            n_mels: 80,
            hidden: 192,
            layers: 6,
            heads: 2,
            ffn_mult: 4,
            duration_layers: 2,
        }
    }
}

/// 多头自注意力。
struct SelfAttention {
    query: Linear,
    key: Linear,
    value: Linear,
    output: Linear,
    heads: usize,
}

impl SelfAttention {
    fn new(hidden: usize, heads: usize, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            query: candle_nn::linear(hidden, hidden, vb.pp("query"))?,
            key: candle_nn::linear(hidden, hidden, vb.pp("key"))?,
            value: candle_nn::linear(hidden, hidden, vb.pp("value"))?,
            output: candle_nn::linear(hidden, hidden, vb.pp("output"))?,
            heads,
        })
    }
}

impl Module for SelfAttention {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (batch, steps, hidden) = xs.dims3()?;
        let head_dim = hidden / self.heads;

        let query = self
            .query
            .forward(xs)?
            .reshape((batch, steps, self.heads, head_dim))?
            .transpose(1, 2)?
            .contiguous()?;
        let key = self
            .key
            .forward(xs)?
            .reshape((batch, steps, self.heads, head_dim))?
            .transpose(1, 2)?
            .contiguous()?;
        let value = self
            .value
            .forward(xs)?
            .reshape((batch, steps, self.heads, head_dim))?
            .transpose(1, 2)?
            .contiguous()?;

        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = query.matmul(&key.transpose(2, 3)?.contiguous()?)?;
        let scores = scores.affine(scale, 0.0)?;
        let weights = candle_nn::ops::softmax(&scores, 3)?;

        let context = weights
            .matmul(&value)?
            .transpose(1, 2)?
            .contiguous()?
            .reshape((batch, steps, hidden))?;
        self.output.forward(&context)
    }
}

/// 单个 Transformer 层，前置归一化加残差。
struct TransformerBlock {
    attention: SelfAttention,
    norm1: LayerNorm,
    norm2: LayerNorm,
    feed_forward1: Linear,
    feed_forward2: Linear,
}

impl TransformerBlock {
    fn new(hidden: usize, heads: usize, ffn_mult: usize, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            attention: SelfAttention::new(hidden, heads, vb.pp("attention"))?,
            norm1: candle_nn::layer_norm(hidden, 1e-5, vb.pp("norm1"))?,
            norm2: candle_nn::layer_norm(hidden, 1e-5, vb.pp("norm2"))?,
            feed_forward1: candle_nn::linear(hidden, hidden * ffn_mult, vb.pp("ff1"))?,
            feed_forward2: candle_nn::linear(hidden * ffn_mult, hidden, vb.pp("ff2"))?,
        })
    }
}

impl Module for TransformerBlock {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let attention = self.attention.forward(&self.norm1.forward(xs)?)?;
        let xs = (xs + attention)?;
        let hidden = self.feed_forward1.forward(&self.norm2.forward(&xs)?)?;
        let hidden = hidden.relu()?;
        let hidden = self.feed_forward2.forward(&hidden)?;
        &xs + hidden
    }
}

/// 文本编码器：token 序列到逐 token 隐状态。
pub struct TextEncoder {
    embedding: Embedding,
    blocks: Vec<TransformerBlock>,
    output: Linear,
    norm: LayerNorm,
}

impl TextEncoder {
    /// 构造编码器。
    pub fn new(config: &MatchaConfig, vb: VarBuilder) -> Result<Self> {
        let mut blocks = Vec::with_capacity(config.layers);
        for index in 0..config.layers {
            blocks.push(TransformerBlock::new(
                config.hidden,
                config.heads,
                config.ffn_mult,
                vb.pp(format!("block{index}")),
            )?);
        }
        Ok(Self {
            embedding: candle_nn::embedding(
                config.n_vocab,
                config.hidden,
                vb.pp("embedding"),
            )?,
            blocks,
            output: candle_nn::linear(config.hidden, config.hidden, vb.pp("output"))?,
            norm: candle_nn::layer_norm(config.hidden, 1e-5, vb.pp("norm"))?,
        })
    }

    /// 前向：输入 token id（batch 乘 steps 的 U32），输出同形状的隐状态。
    pub fn forward(&self, tokens: &Tensor) -> Result<Tensor> {
        let mut hidden = self.embedding.forward(&tokens.to_dtype(DType::U32)?)?;
        for block in &self.blocks {
            hidden = block.forward(&hidden)?;
        }
        self.norm.forward(&self.output.forward(&hidden)?)
    }
}

/// 时长预测器：逐 token 隐状态到对数帧数。
pub struct DurationPredictor {
    convolutions: Vec<(Conv1d, LayerNorm)>,
    output: Linear,
}

impl DurationPredictor {
    /// 构造时长预测器。
    pub fn new(config: &MatchaConfig, vb: VarBuilder) -> Result<Self> {
        let conv_config = Conv1dConfig {
            padding: 1,
            ..Default::default()
        };
        let mut convolutions = Vec::with_capacity(config.duration_layers);
        for index in 0..config.duration_layers {
            let conv = candle_nn::conv1d(
                config.hidden,
                config.hidden,
                3,
                conv_config,
                vb.pp(format!("conv{index}")),
            )?;
            let norm =
                candle_nn::layer_norm(config.hidden, 1e-5, vb.pp(format!("norm{index}")))?;
            convolutions.push((conv, norm));
        }
        Ok(Self {
            convolutions,
            output: candle_nn::linear(config.hidden, 1, vb.pp("output"))?,
        })
    }

    /// 前向：输入 batch 乘 steps 乘 hidden，输出 batch 乘 steps 的 log 时长。
    ///
    /// 负值会被压到很小的正数，避免后续取指数时出现 0 帧。
    pub fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (batch, steps, _hidden) = xs.dims3()?;
        let mut hidden_states = xs.transpose(1, 2)?.contiguous()?;

        for (conv, norm) in &self.convolutions {
            let convolved = conv.forward(&hidden_states)?;
            let convolved = convolved.relu()?;
            let convolved = convolved.transpose(1, 2)?.contiguous()?;
            let convolved = norm.forward(&convolved)?;
            hidden_states = convolved.transpose(1, 2)?.contiguous()?;
        }

        let hidden_states = hidden_states.transpose(1, 2)?.contiguous()?;
        let output = self.output.forward(&hidden_states)?;
        let output = output.reshape((batch, steps))?;
        output.clamp(f32::ln(0.1), f32::ln(50.0))
    }
}

/// 声学模型：文本编码器加时长预测器。
pub struct Matcha {
    text_encoder: TextEncoder,
    duration_predictor: DurationPredictor,
    hidden: usize,
}

impl Matcha {
    /// 构造模型。
    pub fn new(config: &MatchaConfig, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            text_encoder: TextEncoder::new(config, vb.pp("text_encoder"))?,
            duration_predictor: DurationPredictor::new(config, vb.pp("duration_predictor"))?,
            hidden: config.hidden,
        })
    }

    /// 前向：返回逐 token 隐状态与对数时长。
    pub fn forward(&self, tokens: &Tensor) -> Result<(Tensor, Tensor)> {
        let hidden = self.text_encoder.forward(tokens)?;
        let log_durations = self.duration_predictor.forward(&hidden)?;
        Ok((hidden, log_durations))
    }

    /// 隐层维度。
    pub fn hidden(&self) -> usize {
        self.hidden
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};

    fn config() -> MatchaConfig {
        MatchaConfig {
            n_vocab: 32,
            n_mels: 80,
            hidden: 32,
            layers: 2,
            heads: 2,
            ffn_mult: 2,
            duration_layers: 2,
        }
    }

    #[test]
    fn text_encoder_shape() -> Result<()> {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let encoder = TextEncoder::new(&config(), vb)?;

        let tokens = Tensor::from_vec(vec![1u32, 2, 3, 4, 5], (1, 5), &device)?;
        let hidden = encoder.forward(&tokens)?;
        assert_eq!(hidden.dims(), &[1, 5, 32]);
        Ok(())
    }

    #[test]
    fn duration_predictor_shape() -> Result<()> {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let predictor = DurationPredictor::new(&config(), vb)?;

        let hidden = Tensor::randn(0f32, 1f32, (2, 7, 32), &device)?;
        let durations = predictor.forward(&hidden)?;
        assert_eq!(durations.dims(), &[2, 7]);
        Ok(())
    }

    #[test]
    fn full_forward_shape() -> Result<()> {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let model = Matcha::new(&config(), vb)?;

        let tokens = Tensor::from_vec(vec![1u32, 2, 3, 4], (1, 4), &device)?;
        let (hidden, durations) = model.forward(&tokens)?;
        assert_eq!(hidden.dims(), &[1, 4, 32]);
        assert_eq!(durations.dims(), &[1, 4]);
        assert_eq!(model.hidden(), 32);
        Ok(())
    }

    #[test]
    fn duration_regression_reduces_loss() -> Result<()> {
        // 用固定的目标时长跑几十步，损失应该明显下降，说明反向传播与优化器都通了。
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let model = Matcha::new(&config(), vb)?;

        let tokens = Tensor::from_vec(vec![1u32, 2, 3, 4, 5, 6], (2, 3), &device)?;
        let target = Tensor::from_vec(
            vec![f32::ln(4.0), f32::ln(6.0), f32::ln(8.0), f32::ln(4.0), f32::ln(6.0), f32::ln(8.0)],
            (2, 3),
            &device,
        )?;

        let params = ParamsAdamW {
            lr: 0.02,
            ..Default::default()
        };
        let mut optimizer = AdamW::new(var_map.all_vars(), params)?;

        let mut first_loss = f32::NAN;
        let mut last_loss = f32::NAN;
        for step in 0..60 {
            let (_, log_durations) = model.forward(&tokens)?;
            let loss = log_durations.sub(&target)?.sqr()?.mean_all()?;
            let value: f32 = loss.to_scalar()?;
            if step == 0 {
                first_loss = value;
            }
            last_loss = value;
            optimizer.backward_step(&loss)?;
        }

        assert!(first_loss.is_finite() && last_loss.is_finite());
        assert!(
            last_loss < first_loss * 0.5,
            "loss did not drop: first {first_loss}, last {last_loss}"
        );
        Ok(())
    }
}
