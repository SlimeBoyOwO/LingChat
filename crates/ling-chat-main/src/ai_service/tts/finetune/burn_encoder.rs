//! 用 burn 实现的基座文本编码器，与 candle 版逐层对齐。
//!
//! 网络结构：词嵌入、三层 k=5 的 prenet 卷积（各带沿通道维的 AffineNorm）、
//! 六层 transformer 块（卷积投影自注意力 + k=3 前馈卷积，各带残差与归一化），
//! 末尾分出 mel 先验与对数时长两支。权重名约定与 candle 版一致。

use burn::module::Param;
use burn::nn::conv::{Conv1d, Conv1dConfig};
use burn::nn::PaddingConfig1d;
use burn::prelude::*;
use burn::tensor::activation::{relu, softmax};

/// 编码器超参数（burn 版，字段与 candle 版对应）。
#[derive(Debug, Clone)]
pub struct BurnBaseEncoderConfig {
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

impl Default for BurnBaseEncoderConfig {
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

/// 沿通道维归一化并做逐通道仿射（等价 candle 的 AffineNorm）。
#[derive(Module, Debug)]
struct AffineNorm {
    weight: Param<Tensor<3>>,
    bias: Param<Tensor<3>>,
}

impl AffineNorm {
    fn new(channels: usize, device: &Device) -> Self {
        Self {
            weight: Param::from_tensor(Tensor::<3>::ones([1, channels, 1], device)),
            bias: Param::from_tensor(Tensor::<3>::zeros([1, channels, 1], device)),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let mean = xs.clone().mean_dims(&[1]);
        let centered = xs.clone() - mean;
        let variance = centered.clone().powf_scalar(2.0).mean_dims(&[1]);
        let normed = centered / (variance + 1e-5).sqrt();
        normed * self.weight.val() + self.bias.val()
    }
}

/// 自注意力：四个 1x1 卷积投影，分 2 头在帧维上做缩放点积。
#[derive(Module, Debug)]
struct Attention {
    query: Conv1d,
    key: Conv1d,
    value: Conv1d,
    output: Conv1d,
}

impl Attention {
    fn new(hidden: usize, device: &Device) -> Self {
        let config = Conv1dConfig::new(hidden, hidden, 1);
        Self {
            query: config.clone().init(device),
            key: config.clone().init(device),
            value: config.clone().init(device),
            output: config.init(device),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, hidden, frames] = xs.dims();
        let heads = 2;
        let head_dim = hidden / heads;

        let query = self.query.forward(xs.clone()).reshape([batch, heads, head_dim, frames]);
        let key = self.key.forward(xs.clone()).reshape([batch, heads, head_dim, frames]);
        let value = self.value.forward(xs).reshape([batch, heads, head_dim, frames]);

        // 注意力分数 [b,h,f,f]：q[b,h,f,d] @ k[b,h,f,d]^T。
        let q = query.permute([0, 1, 3, 2]);
        let k = key.permute([0, 1, 3, 2]);
        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = softmax(q.matmul(k.transpose()) * scale, 2);
        let v = value.permute([0, 1, 3, 2]);
        let context = scores.matmul(v).permute([0, 1, 3, 2]);
        let context = context.reshape([batch, hidden, frames]);
        self.output.forward(context)
    }
}

/// 前馈：两个 k=3 卷积 + relu。
#[derive(Module, Debug)]
struct FeedForward {
    conv1: Conv1d,
    conv2: Conv1d,
}

impl FeedForward {
    fn new(hidden: usize, ffn_hidden: usize, device: &Device) -> Self {
        let padded = |channels_in, channels_out| {
            Conv1dConfig::new(channels_in, channels_out, 3).with_padding(PaddingConfig1d::Explicit(1, 1))
        };
        Self {
            conv1: padded(hidden, ffn_hidden).init(device),
            conv2: padded(ffn_hidden, hidden).init(device),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let hidden = relu(self.conv1.forward(xs));
        self.conv2.forward(hidden)
    }
}

/// 文本编码器（burn 版）。
#[derive(Module, Debug)]
pub struct BurnBaseEncoder {
    embedding: burn::nn::Embedding,
    prenet: Vec<Conv1d>,
    prenet_proj: Conv1d,
    prenet_norms: Vec<AffineNorm>,
    block_norms: Vec<(AffineNorm, AffineNorm)>,
    attentions: Vec<Attention>,
    feed_forwards: Vec<FeedForward>,
    proj_m: Conv1d,
    duration_conv1: Conv1d,
    duration_conv2: Conv1d,
    duration_proj: Conv1d,
}

impl BurnBaseEncoder {
    /// 随机初始化（权重不来自基座，用于验证结构）。
    pub fn init(config: &BurnBaseEncoderConfig, device: &Device) -> Self {
        let embedding = burn::nn::EmbeddingConfig::new(config.n_vocab, config.hidden).init(device);

        let mut prenet = Vec::with_capacity(3);
        let padded = Conv1dConfig::new(config.hidden, config.hidden, 5)
            .with_padding(PaddingConfig1d::Explicit(2, 2));
        for _ in 0..3 {
            prenet.push(padded.clone().init(device));
        }
        let prenet_proj = Conv1dConfig::new(config.hidden, config.hidden, 1).init(device);

        let mut prenet_norms = Vec::with_capacity(3);
        for _ in 0..3 {
            prenet_norms.push(AffineNorm::new(config.hidden, device));
        }

        let mut attentions = Vec::with_capacity(config.layers);
        let mut feed_forwards = Vec::with_capacity(config.layers);
        let mut block_norms = Vec::with_capacity(config.layers);
        for _ in 0..config.layers {
            attentions.push(Attention::new(config.hidden, device));
            feed_forwards.push(FeedForward::new(config.hidden, config.ffn_hidden, device));
            block_norms.push((
                AffineNorm::new(config.hidden, device),
                AffineNorm::new(config.hidden, device),
            ));
        }

        Self {
            embedding,
            prenet,
            prenet_proj,
            prenet_norms,
            block_norms,
            attentions,
            feed_forwards,
            proj_m: Conv1dConfig::new(config.hidden, config.n_mels, 1).init(device),
            duration_conv1: Conv1dConfig::new(config.hidden, config.duration_hidden, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            duration_conv2: Conv1dConfig::new(config.duration_hidden, config.duration_hidden, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            duration_proj: Conv1dConfig::new(config.duration_hidden, 1, 1).init(device),
        }
    }

    /// 前向：返回 mel 先验与对数时长。
    pub fn forward(&self, tokens: Tensor<2, Int>) -> (Tensor<3>, Tensor<2>) {
        // 词嵌入输出 [b, seq, hidden]，转成 [b, hidden, seq]。
        let mut hidden = self.embedding.forward(tokens).transpose();
        let [batch, _hidden_dim, frames] = hidden.dims();

        for index in 0..self.prenet.len() {
            let conv = relu(self.prenet[index].forward(hidden.clone()));
            hidden = self.prenet_norms[index].forward(conv);
        }
        hidden = self.prenet_proj.forward(hidden);

        for index in 0..self.attentions.len() {
            let (norm1, norm2) = &self.block_norms[index];
            let attn_out = self.attentions[index].forward(norm1.forward(hidden.clone()));
            hidden = hidden + attn_out;
            let ffn_out = self.feed_forwards[index].forward(norm2.forward(hidden.clone()));
            hidden = hidden + ffn_out;
        }

        let mel = self.proj_m.forward(hidden.clone());
        let durations = relu(self.duration_conv1.forward(hidden));
        let durations = relu(self.duration_conv2.forward(durations));
        let proj_dur = self.duration_proj.forward(durations);
        let durations = proj_dur.reshape([batch, frames]);
        (mel, durations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 在 Vulkan 设备上跑一次前向，验证形状与链路可用。
    #[test]
    fn forward_produces_mel_and_durations() {
        let device = Device::vulkan(DeviceKind::DefaultDevice);
        let config = BurnBaseEncoderConfig::default();
        let encoder = BurnBaseEncoder::init(&config, &device);

        let tokens = Tensor::<2, Int>::from_data(
            TensorData::new(vec![1u32, 2, 3, 4, 5], vec![1, 5]),
            &device,
        );
        let (mel, durations) = encoder.forward(tokens);
        assert_eq!(mel.shape().dims(), [1, 80, 5]);
        assert_eq!(durations.shape().dims(), [1, 5]);
    }
}
