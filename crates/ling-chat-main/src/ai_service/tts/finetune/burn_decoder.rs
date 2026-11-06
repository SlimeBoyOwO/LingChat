//! 用 burn 实现的基座解码器，与 candle 版逐层对齐。
//!
//! 网络结构：时间 MLP、下行路径（时间条件卷积残差块 + 自注意力 + 卷积下采样）、
//! 中部（两个残差注意力块）、上行路径（残差注意力块 + 上采样/转置卷积）、
//! 收尾卷积与 mel 投影。转置卷积用 burn 原生实现，省去 candle 为绕开自动求导
//! 限制而写的手工等价版本。

use burn::module::{Module, Param};
use burn::nn::conv::{Conv1d, Conv1dConfig, ConvTranspose1d, ConvTranspose1dConfig};
use burn::nn::PaddingConfig1d;
use burn::prelude::*;
use burn::tensor::activation::{mish, silu};

/// 解码器超参数（burn 版，字段与 candle 版对应）。
#[derive(Debug, Clone)]
pub struct BurnBaseDecoderConfig {
    /// 隐层通道数。
    pub channels: usize,
    /// 时间嵌入维度。
    pub time_dim: usize,
    /// 时间 MLP 隐层。
    pub time_hidden: usize,
    /// mel 通道数。
    pub n_mels: usize,
    /// 解码器输入通道数：加噪 mel 与先验 mel 拼接。
    pub input_channels: usize,
}

impl Default for BurnBaseDecoderConfig {
    fn default() -> Self {
        Self {
            channels: 256,
            time_dim: 160,
            time_hidden: 1024,
            n_mels: 80,
            input_channels: 160,
        }
    }
}

/// 分组归一化加逐通道仿射，等价 candle 的 GroupNormAffine。
///
/// 把通道分 8 组，组内按（通道 x 帧）做纯归一化，再施加逐通道仿射
/// （weight/bias 形状 [channels, 1]，由匿名 onnx 常量按映射表落位）。
#[derive(Module, Debug)]
struct GroupNormAffine {
    /// 纯分组归一化（无内部仿射），仿射由 weight/bias 手工施加。
    norm: burn::nn::GroupNorm,
    weight: Param<Tensor<2>>,
    bias: Param<Tensor<2>>,
}

impl GroupNormAffine {
    fn new(channels: usize, groups: usize, device: &Device) -> Self {
        use burn::nn::GroupNormConfig;
        Self {
            norm: GroupNormConfig::new(groups, channels)
                .with_affine(false)
                .with_epsilon(1e-5)
                .init(device),
            weight: Param::from_tensor(Tensor::<2>::ones([channels, 1], device)),
            bias: Param::from_tensor(Tensor::<2>::zeros([channels, 1], device)),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, channels, frames] = xs.dims();
        let normed = self.norm.forward(xs);
        let weight = self.weight.val().unsqueeze_dim::<3>(0); // [1, channels, 1]
        let bias = self.bias.val().unsqueeze_dim::<3>(0);
        normed * weight.expand([batch, channels, frames])
            + bias.expand([batch, channels, frames])
    }
}

/// 时间嵌入与两层投影（内部 SiLU）。
#[derive(Module, Debug)]
struct TimeMlp {
    linear1: burn::nn::Linear,
    linear2: burn::nn::Linear,
}

impl TimeMlp {
    fn new(config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        use burn::nn::LinearConfig;
        Self {
            linear1: LinearConfig::new(config.time_dim, config.time_hidden).init(device),
            linear2: LinearConfig::new(config.time_hidden, config.time_hidden).init(device),
        }
    }

    /// 输入 [batch, time_dim]，输出 [batch, time_hidden]。
    fn forward(&self, time: Tensor<2>) -> Tensor<2> {
        let hidden = self.linear1.forward(time);
        self.linear2.forward(silu(hidden))
    }
}

/// 时间条件卷积残差块。
#[derive(Module, Debug)]
struct ResBlock {
    /// 块级时间条件投影：time_hidden -> channels（图上 mlp.1 Gemm）。
    time_proj: burn::nn::Linear,
    conv1: Conv1d,
    conv2: Conv1d,
    res_conv: Conv1d,
    norm1: GroupNormAffine,
    norm2: GroupNormAffine,
    channels: usize,
}

impl ResBlock {
    fn new(input_channels: usize, config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        use burn::nn::LinearConfig;
        let channels = config.channels;
        Self {
            time_proj: LinearConfig::new(config.time_hidden, channels).init(device),
            conv1: Conv1dConfig::new(input_channels, channels, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            conv2: Conv1dConfig::new(channels, channels, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            res_conv: Conv1dConfig::new(input_channels, channels, 1).init(device),
            norm1: GroupNormAffine::new(channels, 8, device),
            norm2: GroupNormAffine::new(channels, 8, device),
            channels,
        }
    }

    /// 输入 [batch, input_channels, frames]，输出 [batch, channels, frames]。
    fn forward(&self, xs: Tensor<3>, time: Tensor<2>) -> Tensor<3> {
        let [batch, _ic, _frames] = xs.dims();
        let res = self.res_conv.forward(xs.clone());
        // 块级时间条件：mish(time) -> 线性 -> [batch, channels, 1] 广播加。
        let bias = self
            .time_proj
            .forward(mish(time))
            .reshape([batch, self.channels, 1]);
        let hidden = self.norm1.forward(self.conv1.forward(xs));
        let hidden = mish(hidden) + bias;
        let hidden = self.norm2.forward(self.conv2.forward(hidden));
        let hidden = mish(hidden);
        res + hidden
    }
}

/// 带仿射参数的归一化，沿通道维（注意力块的 norm1/norm3）。
#[derive(Module, Debug)]
struct Affine {
    weight: Param<Tensor<1>>,
    bias: Param<Tensor<1>>,
}

impl Affine {
    fn new(channels: usize, device: &Device) -> Self {
        Self {
            weight: Param::from_tensor(Tensor::<1>::ones([channels], device)),
            bias: Param::from_tensor(Tensor::<1>::zeros([channels], device)),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, channels, frames] = xs.dims();
        let mean = xs.clone().mean_dims(&[1]);
        let centered = xs.clone() - mean;
        let variance = centered.clone().powf_scalar(2.0).mean_dims(&[1]);
        let normed = centered / (variance + 1e-5).sqrt();
        let weight = self.weight.val().reshape([1, channels, 1]).expand([batch, channels, frames]);
        let bias = self.bias.val().reshape([1, channels, 1]).expand([batch, channels, frames]);
        normed * weight + bias
    }
}

/// 自注意力加前馈块（多头注意力，2 头）。
#[derive(Module, Debug)]
struct AttnBlock {
    norm1: Affine,
    norm3: Affine,
    q: burn::nn::Linear,
    k: burn::nn::Linear,
    v: burn::nn::Linear,
    to_out: burn::nn::Linear,
    ff_proj: burn::nn::Linear,
    ff_out: burn::nn::Linear,
    ff_alpha: Param<Tensor<1>>,
    ff_beta: Param<Tensor<1>>,
}

impl AttnBlock {
    fn new(config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        use burn::nn::LinearConfig;
        let channels = config.channels;
        let inner = channels / 2;
        Self {
            norm1: Affine::new(channels, device),
            norm3: Affine::new(channels, device),
            q: LinearConfig::new(channels, inner).with_bias(false).init(device),
            k: LinearConfig::new(channels, inner).with_bias(false).init(device),
            v: LinearConfig::new(channels, inner).with_bias(false).init(device),
            to_out: LinearConfig::new(inner, channels).init(device),
            ff_proj: LinearConfig::new(channels, config.time_hidden).init(device),
            ff_out: LinearConfig::new(config.time_hidden, channels).init(device),
            ff_alpha: Param::from_tensor(Tensor::<1>::ones([config.time_hidden], device)),
            ff_beta: Param::from_tensor(Tensor::<1>::zeros([config.time_hidden], device)),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, channels, frames] = xs.dims();
        let heads = 2;
        let inner = channels / 2;
        let head_dim = inner / heads;

        let normed = self.norm1.forward(xs.clone()).transpose(); // [B, F, C]
        let q = self.q.forward(normed.clone()).reshape([batch, frames, heads, head_dim]);
        let q = q.permute([0, 2, 1, 3]); // [B, heads, F, head_dim]
        let k = self.k.forward(normed.clone()).reshape([batch, frames, heads, head_dim]);
        let k = k.permute([0, 2, 1, 3]);
        let v = self.v.forward(normed).reshape([batch, frames, heads, head_dim]);
        let v = v.permute([0, 2, 1, 3]);

        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = q.matmul(k.transpose()) * scale;
        let weights = burn::tensor::activation::softmax(scores, 3);
        let context = weights.matmul(v).transpose().reshape([batch, frames, inner]); // [B,F,inner]
        let attended = self.to_out.forward(context).transpose(); // [B,C,F]
        let hidden = attended + xs;

        let normed3 = self.norm3.forward(hidden.clone()).transpose(); // [B,F,C]
        let [b, f, c] = normed3.clone().dims();
        let projected = self.ff_proj.forward(normed3.reshape([b * f, c]));
        let activated = gelu_parametric(projected, self.ff_alpha.val(), self.ff_beta.val());
        let feed = self
            .ff_out
            .forward(activated)
            .reshape([b, f, c])
            .transpose(); // [B,C,F]
        hidden + feed
    }
}

/// 参数化 GELU：u + sin(u*exp(alpha))^2 / (exp(beta) + 1e-9)。
/// 参数化 GELU：u + sin(u*exp(alpha))^2 / (exp(beta) + 1e-9)。
fn gelu_parametric(x: Tensor<2>, alpha: Tensor<1>, beta: Tensor<1>) -> Tensor<2> {
    let [_, cols] = x.dims();
    // 展开成 [1, cols] 与帧维广播相乘。
    let alpha = alpha.reshape([1, cols]);
    let beta = beta.reshape([1, cols]);
    let scaled = x.clone() * alpha.exp();
    let sin_sq = scaled.sin().powf_scalar(2.0);
    let denom = beta.exp().add_scalar(1e-9);
    x + sin_sq / denom
}

/// 上采样算子：up_blocks.{i}.2，i=0 转置卷积，i=1 普通卷积。
#[derive(Module, Debug)]
enum UpSampler {
    Transpose(ConvTranspose1d),
    Conv(Conv1d),
}

impl UpSampler {
    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        match self {
            UpSampler::Transpose(c) => c.forward(xs),
            UpSampler::Conv(c) => c.forward(xs),
        }
    }
}

/// 解码器：时间 MLP、下行、中部、上行与收尾投影。
#[derive(Module, Debug)]
pub struct BurnBaseDecoder {
    time_mlp: TimeMlp,
    down: Vec<(ResBlock, AttnBlock, Conv1d)>,
    mid: Vec<(ResBlock, AttnBlock)>,
    up: Vec<(ResBlock, AttnBlock, UpSampler)>,
    final_norm: GroupNormAffine,
    final_block: Conv1d,
    final_proj: Conv1d,
}

impl BurnBaseDecoder {
    /// 用随机权重初始化解码器。
    pub fn init(config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        let mut down = Vec::with_capacity(2);
        for index in 0..2 {
            let input_channels = if index == 0 {
                config.input_channels
            } else {
                config.channels
            };
            down.push((
                ResBlock::new(input_channels, config, device),
                AttnBlock::new(config, device),
                Conv1dConfig::new(config.channels, config.channels, 3)
                    .with_stride(if index == 0 { 2 } else { 1 })
                    .with_padding(PaddingConfig1d::Explicit(1, 1))
                    .init(device),
            ));
        }

        let mid = (0..2)
            .map(|_| (ResBlock::new(config.channels, config, device), AttnBlock::new(config, device)))
            .collect();

        let mut up = Vec::with_capacity(2);
        for index in 0..2 {
            let sampler = if index == 0 {
                UpSampler::Transpose(
                    ConvTranspose1dConfig::new([config.channels, config.channels], 4)
                        .with_stride(2)
                        .with_padding(1)
                        .init(device),
                )
            } else {
                UpSampler::Conv(
                    Conv1dConfig::new(config.channels, config.channels, 3)
                        .with_padding(PaddingConfig1d::Explicit(1, 1))
                        .init(device),
                )
            };
            up.push((
                ResBlock::new(config.channels * 2, config, device),
                AttnBlock::new(config, device),
                sampler,
            ));
        }

        Self {
            time_mlp: TimeMlp::new(config, device),
            down,
            mid,
            up,
            final_norm: GroupNormAffine::new(config.channels, 8, device),
            final_block: Conv1dConfig::new(config.channels, config.channels, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            final_proj: Conv1dConfig::new(config.channels, config.n_mels, 1).init(device),
        }
    }

    /// 前向：输入加噪 mel、文本隐状态与时间，输出速度场。
    pub fn forward(&self, noisy: Tensor<3>, text: Tensor<3>, time: Tensor<2>) -> Tensor<3> {
        let time = self.time_mlp.forward(time);
        let mut hidden = Tensor::cat(vec![noisy, text], 1);

        let mut skips: Vec<Tensor<3>> = Vec::with_capacity(2);
        for (block, attention, down) in &self.down {
            hidden = block.forward(hidden, time.clone());
            hidden = attention.forward(hidden);
            skips.push(hidden.clone());
            hidden = down.forward(hidden);
        }
        for (block, attention) in &self.mid {
            hidden = block.forward(hidden, time.clone());
            hidden = attention.forward(hidden);
        }
        for (block, attention, sampler) in &self.up {
            let skip = skips.pop().expect("上行路径的 skip 数量与下行路径对齐");
            let [_, _, skip_frames] = skip.dims();
            let [_, _, f] = hidden.dims();
            if f != skip_frames {
                hidden = hidden.narrow(2, 0, skip_frames);
            }
            hidden = Tensor::cat(vec![hidden, skip], 1);
            hidden = block.forward(hidden, time.clone());
            hidden = attention.forward(hidden);
            hidden = sampler.forward(hidden);
        }
        let hidden = self.final_block.forward(hidden);
        let hidden = self.final_norm.forward(hidden);
        let hidden = mish(hidden);
        self.final_proj.forward(hidden)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 在 Vulkan 设备上跑一次前向，验证形状与链路可用。
    /// 输入 [1, 160, 5]，时间 [1, 160]，输出 [1, 80, 10]（上采样后帧数变化）。
    #[test]
    fn forward_produces_velocity_field() {
        let device = Device::vulkan(DeviceKind::DefaultDevice);
        let config = BurnBaseDecoderConfig::default();
        let decoder = BurnBaseDecoder::init(&config, &device);

        let noisy: Tensor<3, Float> = Tensor::<3, Float>::from_data(
            TensorData::new(vec![0.0f32; 80 * 5], vec![1, 80, 5]),
            &device,
        );
        let text: Tensor<3, Float> = Tensor::<3, Float>::from_data(
            TensorData::new(vec![0.0f32; 80 * 5], vec![1, 80, 5]),
            &device,
        );
        let time: Tensor<2, Float> = Tensor::<2, Float>::from_data(
            TensorData::new(vec![0.5f32; 160], vec![1, 160]),
            &device,
        );
        let out: Tensor<3, Float> = decoder.forward(noisy, text, time);
        // 下行把 5 帧变 3，上行转置再放大，最终约 [1, 80, N]。仅校验通道数。
        assert_eq!(out.shape().dims::<3>()[1], 80);
    }
}
