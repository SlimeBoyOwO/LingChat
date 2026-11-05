//! 与基座对齐的解码器。
//!
//! 按 `matcha-icefall-zh-baker` 的真实 ONNX 图结构还原：时间 MLP、带 FiLM 调制
//! 的卷积残差块、自注意力块、上下采样与收尾投影。
//!
//! 关键点（来自图与权重绑定）：
//! - 卷积残差块是「时间条件卷积」：卷积后做 GroupNorm(8)+逐通道仿射（匿名
//!   `[256,1]` 权重/偏置），叠加块级时间偏置，残差用 1x1 卷积。
//! - 所有非线性是 Mish（softplus→tanh→乘）；全局时间 MLP 内部用 SiLU。
//! - 注意力块的 `norm1`/`norm3` 用命名张量；`qkv`/`to_out`/`ff` 用匿名投影。
//! - 匿名常量按 `decoder_weight_map.json` 的 target 落位绑定。

use std::path::Path;

use candle_core::{Device, Result, Tensor};
use candle_nn::{Conv1d, Conv1dConfig, Linear, Module, VarBuilder, VarMap};

/// 解码器配置。
#[derive(Debug, Clone)]
pub struct BaseDecoderConfig {
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

impl Default for BaseDecoderConfig {
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

/// 时间嵌入与两层投影（内部 SiLU）。
pub struct TimeMlp {
    linear1: Linear,
    linear2: Linear,
}

impl TimeMlp {
    fn new(config: &BaseDecoderConfig, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            linear1: candle_nn::linear(config.time_dim, config.time_hidden, vb.pp("linear_1"))?,
            linear2: candle_nn::linear(config.time_hidden, config.time_hidden, vb.pp("linear_2"))?,
        })
    }

    /// 输入 batch 乘 time_dim，输出 batch 乘 time_hidden。
    pub fn forward(&self, time: &Tensor) -> Result<Tensor> {
        let hidden = self.linear1.forward(time)?.contiguous()?;
        let silu = candle_nn::ops::silu(&hidden)?;
        self.linear2.forward(&silu)
    }
}

/// 卷积残差块的归一化：GroupNorm=8（把 256 通道分 8 组，每组 32 通道按通道×帧归一化），
/// 再应用逐通道可学习仿射（weight/bias 形状 `[channels,1]`）。
///
/// 对应图中 `InstanceNormalization`（分组后 scale=1/bias=0，即纯归一化）后的匿名
/// `Mul`/`Add` 仿射常量。
struct GroupNormAffine {
    weight: Tensor,
    bias: Tensor,
    groups: usize,
    channels_per_group: usize,
}

impl GroupNormAffine {
    /// `weight`/`bias` 形状 `[channels,1]`，由匿名常量按映射表落位。
    fn new(channels: usize, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            weight: vb.get_with_hints((channels, 1), "weight", candle_nn::init::ONE)?,
            bias: vb.get_with_hints((channels, 1), "bias", candle_nn::init::ZERO)?,
            groups: 8,
            channels_per_group: channels / 8,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (batch, channels, frames) = xs.dims3()?;
        // 分组视图：[batch, groups, channels_per_group, frames]，组内归一化
        let grouped = xs.reshape((batch, self.groups, self.channels_per_group, frames))?;
        let mean = grouped.mean_keepdim(2)?.mean_keepdim(3)?;
        let centered = grouped.broadcast_sub(&mean)?;
        let variance = centered.sqr()?.mean_keepdim(2)?.mean_keepdim(3)?;
        let normed = centered.broadcast_div(&(variance + 1e-5)?.sqrt()?)?;
        let normed = normed.reshape((batch, channels, frames))?;
        let weight = self
            .weight
            .reshape((1, channels, 1))?
            .broadcast_as((batch, channels, frames))?;
        let bias = self
            .bias
            .reshape((1, channels, 1))?
            .broadcast_as((batch, channels, frames))?;
        normed.broadcast_mul(&weight)?.broadcast_add(&bias)
    }
}

/// 时间条件卷积残差块。
///
/// 数据流（对应图上 `down_blocks.{i}.0` 内部）：
/// ```text
/// h = mish(conv1(x));
/// h = h + time_bias;                 // 块级时间条件（mlp.0 GELU → mlp.1 Gemm）
/// h = mish(conv2(h));
/// y = h + res_conv(x);
/// ```
/// 每段卷积后都有 GroupNorm 仿射 + Mish。图中 `x × Cast_3` 与卷积后 `× Cast_3`
/// 是 padding mask（有效帧为 1、补齐帧为 0），单步前向没有补齐帧，故可省去。
pub struct ResBlock {
    /// 块级时间条件投影：time_hidden -> channels（图上 `mlp.1` Gemm）。
    time_proj: Linear,
    conv1: Conv1d,
    conv2: Conv1d,
    res_conv: Conv1d,
    norm1: GroupNormAffine,
    norm2: GroupNormAffine,
    channels: usize,
}

impl ResBlock {
    fn new(input_channels: usize, config: &BaseDecoderConfig, vb: VarBuilder) -> Result<Self> {
        let padded = Conv1dConfig {
            padding: 1,
            ..Default::default()
        };
        let channels = config.channels;
        Ok(Self {
            time_proj: candle_nn::linear(config.time_hidden, channels, vb.pp("mlp").pp("1"))?,
            conv1: candle_nn::conv1d(
                input_channels,
                channels,
                3,
                padded,
                vb.pp("block1").pp("block").pp("0"),
            )?,
            conv2: candle_nn::conv1d(
                channels,
                channels,
                3,
                padded,
                vb.pp("block2").pp("block").pp("0"),
            )?,
            res_conv: candle_nn::conv1d(
                input_channels,
                channels,
                1,
                Default::default(),
                vb.pp("res_conv"),
            )?,
            norm1: GroupNormAffine::new(channels, vb.pp("block1").pp("block").pp("1"))?,
            norm2: GroupNormAffine::new(channels, vb.pp("block2").pp("block").pp("1"))?,
            channels,
        })
    }

    /// 输入 batch 乘 input_channels 乘 frames，输出 batch 乘 channels 乘 frames。
    /// `time` 是 batch 乘 time_hidden 的全局时间嵌入（已过 time_mlp）。
    pub fn forward(&self, xs: &Tensor, time: &Tensor) -> Result<Tensor> {
        let (batch, _input_channels, frames) = xs.dims3()?;
        let res = self.res_conv.forward(xs)?;
        // 块级时间条件：mish(time) → Gemm，展开成 [batch, self.channels, 1]
        let bias = self
            .time_proj
            .forward(&candle_nn::ops::mish(time)?)?
            .reshape((batch, self.channels, 1))?
            .broadcast_as((batch, self.channels, frames))?;

        let hidden = self.norm1.forward(&self.conv1.forward(xs)?)?;
        let hidden = candle_nn::ops::mish(&hidden)?.add(&bias)?;

        let hidden = self.norm2.forward(&self.conv2.forward(&hidden)?)?;
        let hidden = candle_nn::ops::mish(&hidden)?;
        res.add(&hidden)
    }
}

/// 带仿射参数的归一化，沿通道维（注意力块的 norm1/norm3，用命名张量）。
pub struct Affine {
    weight: Tensor,
    bias: Tensor,
}

impl Affine {
    fn new(channels: usize, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            weight: vb.get_with_hints(channels, "weight", candle_nn::init::ONE)?,
            bias: vb.get_with_hints(channels, "bias", candle_nn::init::ZERO)?,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (batch, channels, frames) = xs.dims3()?;
        let mean = xs.mean_keepdim(1)?;
        let centered = xs.broadcast_sub(&mean)?;
        let variance = centered.sqr()?.mean_keepdim(1)?;
        let normed = centered.broadcast_div(&(variance + 1e-5)?.sqrt()?)?;
        let weight = self
            .weight
            .reshape((1, channels, 1))?
            .broadcast_as((batch, channels, frames))?;
        let bias = self
            .bias
            .reshape((1, channels, 1))?
            .broadcast_as((batch, channels, frames))?;
        normed.broadcast_mul(&weight)?.broadcast_add(&bias)
    }
}

/// 自注意力加前馈块。与基座的多头注意力对齐：2 头、head_dim=64，q/k/v 各把
/// channels 投影到 channels/2，softmax 点积后 `to_out` 回到 channels，块级残差。
///
/// 投影权重均为匿名常量，按 `decoder_weight_map.json` 的 target 落位绑定。
pub struct AttnBlock {
    /// 前置 LayerNorm。
    norm1: Affine,
    /// 注意力后 LayerNorm。
    norm3: Affine,
    q: Linear,
    k: Linear,
    v: Linear,
    to_out: Linear,
    /// 前馈网络：proj -> 参数化 GELU -> inflate。
    ff_proj: Linear,
    ff_out: Linear,
    /// 前馈参数化激活的逐通道尺度/偏置。
    ff_alpha: Tensor,
    ff_beta: Tensor,
}

impl AttnBlock {
    fn new(config: &BaseDecoderConfig, vb: VarBuilder) -> Result<Self> {
        let channels = config.channels;
        let inner = channels / 2;
        Ok(Self {
            norm1: Affine::new(channels, vb.pp("norm1"))?,
            norm3: Affine::new(channels, vb.pp("norm3"))?,
            q: candle_nn::linear_no_bias(channels, inner, vb.pp("attn1").pp("to_q"))?,
            k: candle_nn::linear_no_bias(channels, inner, vb.pp("attn1").pp("to_k"))?,
            v: candle_nn::linear_no_bias(channels, inner, vb.pp("attn1").pp("to_v"))?,
            to_out: candle_nn::linear(inner, channels, vb.pp("attn1").pp("to_out").pp("0"))?,
            ff_proj: candle_nn::linear(
                channels,
                config.time_hidden,
                vb.pp("ff").pp("net").pp("0").pp("proj"),
            )?,
            ff_out: candle_nn::linear(config.time_hidden, channels, vb.pp("ff").pp("net").pp("2"))?,
            ff_alpha: vb
                .pp("ff")
                .pp("net")
                .pp("0")
                .get_with_hints(config.time_hidden, "alpha", candle_nn::init::ONE)?,
            ff_beta: vb
                .pp("ff")
                .pp("net")
                .pp("0")
                .get_with_hints(config.time_hidden, "beta", candle_nn::init::ZERO)?,
        })
    }

    /// 输入输出都是 batch 乘 channels 乘 frames。
    pub fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let (batch, channels, frames) = xs.dims3()?;
        let heads = 2;
        let inner = channels / 2;
        let head_dim = inner / heads;

        // 前置 LayerNorm，随后转成 [B,F,C] 便于 Linear。
        let normed = self.norm1.forward(xs)?.transpose(1, 2)?.contiguous()?; // [B,F,C]

        // 多头自注意力。
        let q = self
            .q
            .forward(&normed)?
            .reshape((batch, frames, heads, head_dim))?
            .transpose(1, 2)?
            .contiguous()?; // [B,heads,F,head_dim]
        let k = self
            .k
            .forward(&normed)?
            .reshape((batch, frames, heads, head_dim))?
            .transpose(1, 2)?
            .contiguous()?;
        let v = self
            .v
            .forward(&normed)?
            .reshape((batch, frames, heads, head_dim))?
            .transpose(1, 2)?
            .contiguous()?;

        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = q.matmul(&k.transpose(2, 3)?.contiguous()?)?.affine(scale, 0.0)?;
        let weights = candle_nn::ops::softmax(&scores, 3)?;
        let context = weights
            .matmul(&v)?
            .transpose(1, 2)?
            .contiguous()?
            .reshape((batch, frames, inner))?; // [B,F,inner]
        let attended = self.to_out.forward(&context)?; // [B,F,channels]
        let attended = attended.transpose(1, 2)?.contiguous()?; // [B,C,F]
        let hidden = attended.add(&xs)?; // 注意力残差

        // 前馈：LayerNorm -> 参数化 GELU -> 线性 -> 残差。
        let normed = self.norm3.forward(&hidden)?.transpose(1, 2)?.contiguous()?; // [B,F,C]
        let projected = self.ff_proj.forward(&normed)?; // [B,F,time_hidden]
        let activated = gelu_parametric(
            &projected,
            &self.ff_alpha,
            &self.ff_beta,
        )?;
        let feed = self.ff_out.forward(&activated)?; // [B,F,C]
        let feed = feed.transpose(1, 2)?.contiguous()?; // [B,C,F]
        hidden.add(&feed)
    }
}

/// 参数化 GELU：`u + sin(u·e^alpha)² / (e^beta + 1e-9)`，逐元素作用于展平后的帧。
/// 对应图上 `ff.net.0` 内部：`proj` 后经 `Exp`/`Sin`/`Pow` 的激活组合。
fn gelu_parametric(x: &Tensor, alpha: &Tensor, beta: &Tensor) -> Result<Tensor> {
    let u = x;
    let scaled = u.broadcast_mul(&alpha.exp()?)?;
    let sin_sq = scaled.sin()?.sqr()?;
    let denom = beta.exp()?.affine(1.0, 1e-9)?;
    u.add(&sin_sq.broadcast_div(&denom)?)
}

/// 用基础算子重写的转置卷积（k=4, stride=2, padding=1, dilation=1）。
///
/// candle 的 `conv_transpose1d` 在自动求导下会报 `backward is not supported for
/// conv-transpose1d`，导致此层所在路径无法训练。培训时把它拆成「1x1 卷积 +
/// 时间维位移」，数学上与转置卷积严格等价，普通基础算子都能自动求导，从而
/// 该层权重进入优化器也能正常反向。
///
/// 推导：转置卷积 out[co, n] = Σ_{ci, k} x[ci, m]·W[ci, co, k]，其中
/// n = m·stride - padding + k = 2m - 1 + k。按输出位置奇偶拆分：
///   偶数位 n=2p:  取 k=1 时 m=p，k=3 时 m=p-1；
///   奇数位 n=2p+1: 取 k=0 时 m=p+1，k=2 时 m=p。
/// 于是 4 个 1x1 卷积的分量按时间维相对位移叠加即可得到完整输出。
struct ManualConvTranspose1d {
    /// 权重 [C_in, C_out, K=4]，源码里 K=4。
    weight: Tensor,
    /// 输出偏置 [C_out]。
    bias: Option<Tensor>,
}

impl ManualConvTranspose1d {
    fn new(vb: VarBuilder, c_in: usize, c_out: usize) -> Result<Self> {
        let weight = vb.get((c_in, c_out, 4), "weight")?;
        Ok(Self {
            weight,
            bias: Some(vb.get(c_out, "bias")?),
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // 拆分权重：w_k = W[:, :, k] -> [C_in, C_out]。
        let w0 = self.weight.narrow(2, 0, 1)?.squeeze(2)?;
        let w1 = self.weight.narrow(2, 1, 1)?.squeeze(2)?;
        let w2 = self.weight.narrow(2, 2, 1)?.squeeze(2)?;
        let w3 = self.weight.narrow(2, 3, 1)?.squeeze(2)?;

        // 1x1 卷积：out[co, m] = x[·, m] @ w_k，得 [B, C_out, L]。先展平到二维再做
        // matmul，避免 candle 对「三维×二维」广播的歧义。
        let conv = |wk: &Tensor, xs: &Tensor| -> Result<Tensor> {
            let xt = xs.transpose(1, 2)?.contiguous()?; // [B, L, C_in]
            let (b_, l_, c_) = xt.dims3()?;
            let flat = xt.reshape((b_ * l_, c_))?; // [B*L, C_in]
            let r = flat.matmul(wk)?.reshape((b_, l_, wk.dims()[1]))?; // [B, L, C_out]
            r.transpose(1, 2)
        };
        let part0 = conv(&w0, x)?;
        let part1 = conv(&w1, x)?;
        let part2 = conv(&w2, x)?;
        let part3 = conv(&w3, x)?;

        // 时间维位移：right[p] = part[p-1]（p=0 处补 0），left[p] = part[p+1]
        // （p=L-1 处补 0）。
        let (b, _c, l) = part0.dims3()?;
        let right = |xs: &Tensor| -> Result<Tensor> {
            let z = Tensor::zeros((b, _c, 1), xs.dtype(), xs.device())?;
            Tensor::cat(&[z, xs.narrow(2, 0, l - 1)?], 2)
        };
        let left = |xs: &Tensor| -> Result<Tensor> {
            let z = Tensor::zeros((b, _c, 1), xs.dtype(), xs.device())?;
            Tensor::cat(&[xs.narrow(2, 1, l - 1)?, z], 2)
        };

        // even[p] = part1 + part3 右移；odd[p] = part0 左移 + part2。
        let even = part1.add(&right(&part3)?)?;
        let odd = left(&part0)?.add(&part2)?;

        // 交错到 [B, C_out, 2L]：even 放偶位，odd 放奇位。
        let c_out = self.weight.dims()[1];
        let out = Tensor::stack(&[&even, &odd], 3)?.reshape((b, c_out, 2 * l))?;
        match &self.bias {
            None => Ok(out),
            Some(bias) => Ok(out.broadcast_add(&bias.reshape((1, bias.dims1()?, 1))?)?),
        }
    }
}

/// 上采样算子：`up_blocks.{i}.2`，i=0 是转置卷积(k=4,s=2)，i=1 是 Conv(k=3,s=1)。
enum UpSampler {
    Transpose(ManualConvTranspose1d),
    Conv(Conv1d),
}

impl UpSampler {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        match self {
            UpSampler::Transpose(c) => c.forward(xs),
            UpSampler::Conv(c) => (c as &dyn Module).forward(xs),
        }
    }
}

/// 解码器：时间 MLP、两个 down 块、两个 mid 块、两个 up 块与收尾投影。
pub struct BaseDecoder {
    time_mlp: TimeMlp,
    down: Vec<(ResBlock, AttnBlock, Conv1d)>,
    mid: Vec<(ResBlock, AttnBlock)>,
    up: Vec<(ResBlock, AttnBlock, UpSampler)>,
    /// 收尾卷积 `block.0` + 分组归一化仿射 `block.1`。
    final_norm: GroupNormAffine,
    final_block: Conv1d,
    final_proj: Conv1d,
}

impl BaseDecoder {
    /// 构造解码器。
    pub fn new(config: &BaseDecoderConfig, vb: VarBuilder) -> Result<Self> {
        let vb = vb.pp("model").pp("decoder").pp("estimator");

        let mut down = Vec::with_capacity(2);
        for index in 0..2 {
            let block = vb.pp("down_blocks").pp(index);
            let input_channels = if index == 0 {
                config.input_channels
            } else {
                config.channels
            };
            down.push((
                ResBlock::new(input_channels, config, block.pp("0"))?,
                AttnBlock::new(config, block.pp("1").pp("0"))?,
                candle_nn::conv1d(
                    config.channels,
                    config.channels,
                    3,
                    Conv1dConfig {
                        stride: if index == 0 { 2 } else { 1 },
                        padding: 1,
                        ..Default::default()
                    },
                    if index == 0 {
                        block.pp("2").pp("conv")
                    } else {
                        block.pp("2")
                    },
                )?,
            ));
        }

        let mut mid = Vec::with_capacity(2);
        for index in 0..2 {
            let block = vb.pp("mid_blocks").pp(index);
            mid.push((
                ResBlock::new(config.channels, config, block.pp("0"))?,
                AttnBlock::new(config, block.pp("1").pp("0"))?,
            ));
        }

        let mut up = Vec::with_capacity(2);
        for index in 0..2 {
            let block = vb.pp("up_blocks").pp(index);
            let sampler = if index == 0 {
                UpSampler::Transpose(ManualConvTranspose1d::new(
                    block.pp("2").pp("conv"),
                    config.channels,
                    config.channels,
                )?)
            } else {
                UpSampler::Conv(candle_nn::conv1d(
                    config.channels,
                    config.channels,
                    3,
                    Conv1dConfig {
                        padding: 1,
                        ..Default::default()
                    },
                    block.pp("2"),
                )?)
            };
            up.push((
                ResBlock::new(config.channels * 2, config, block.pp("0"))?,
                AttnBlock::new(config, block.pp("1").pp("0"))?,
                sampler,
            ));
        }

        Ok(Self {
            time_mlp: TimeMlp::new(config, vb.pp("time_mlp"))?,
            down,
            mid,
            up,
            final_norm: GroupNormAffine::new(config.channels, vb.pp("final_block").pp("block").pp("1"))?,
            final_block: candle_nn::conv1d(
                config.channels,
                config.channels,
                3,
                Conv1dConfig {
                    padding: 1,
                    ..Default::default()
                },
                vb.pp("final_block").pp("block").pp("0"),
            )?,
            final_proj: candle_nn::conv1d(
                config.channels,
                config.n_mels,
                1,
                Default::default(),
                vb.pp("final_proj"),
            )?,
        })
    }

    /// 仅测试用：逐级记录上采样前各阶段的特征，便于与基座 intermediate 比对。
    /// 返回顺序：[down0_res, down0_attn, down1_res, down1_attn, mid0_res, mid0_attn,
    /// mid1_res, mid1_attn, up0_res, up0_attn, up1_res, up1_attn, 最终 mel]。
    #[cfg(test)]
    fn staged_outputs(&self, noisy: &Tensor, text: &Tensor, time: &Tensor) -> Result<Vec<Tensor>> {
        let time = self.time_mlp.forward(time)?;
        let mut hidden = Tensor::cat(&[noisy.clone(), text.clone()], 1)?;
        let mut skips: Vec<Tensor> = Vec::with_capacity(2);
        let mut out = Vec::with_capacity(13);

        for (block, attention, down) in &self.down {
            hidden = block.forward(&hidden, &time)?;
            out.push(hidden.clone());
            hidden = attention.forward(&hidden)?;
            out.push(hidden.clone());
            skips.push(hidden.clone());
            hidden = down.forward(&hidden)?;
        }
        for (block, attention) in &self.mid {
            hidden = block.forward(&hidden, &time)?;
            out.push(hidden.clone());
            hidden = attention.forward(&hidden)?;
            out.push(hidden.clone());
        }
        for (block, attention, sampler) in &self.up {
            let skip = skips.pop().expect("skip 对齐");
            hidden = Tensor::cat(&[hidden.clone(), skip], 1)?;
            hidden = block.forward(&hidden, &time)?;
            out.push(hidden.clone());
            hidden = attention.forward(&hidden)?;
            out.push(hidden.clone());
            hidden = sampler.forward(&hidden)?;
        }
        let hidden = self.final_block.forward(&hidden)?;
        let hidden = self.final_norm.forward(&hidden)?;
        let hidden = candle_nn::ops::mish(&hidden)?;
        out.push(self.final_proj.forward(&hidden)?);
        Ok(out)
    }

    /// 前向：输入加噪 mel、文本隐状态与时间，输出速度场。
    /// `time` 是 batch 乘 time_dim。
    pub fn forward(&self, noisy: &Tensor, text: &Tensor, time: &Tensor) -> Result<Tensor> {
        let time = self.time_mlp.forward(time)?;
        let mut hidden = Tensor::cat(&[noisy.clone(), text.clone()], 1)?;

        // 下采样路径：每个块输出 ResBlock->Attn 后存一次 skip，再下采样。
        let mut skips: Vec<Tensor> = Vec::with_capacity(2);
        for (block, attention, down) in &self.down {
            hidden = block.forward(&hidden, &time)?;
            hidden = attention.forward(&hidden)?;
            skips.push(hidden.clone());
            hidden = down.forward(&hidden)?;
        }

        // 中部。
        for (block, attention) in &self.mid {
            hidden = block.forward(&hidden, &time)?;
            hidden = attention.forward(&hidden)?;
        }

        // 上采样路径（栈序取 skip）。
        for (block, attention, sampler) in &self.up {
            let skip = skips.pop().expect("解码器 up 阶段的 skip 数量与 down 对齐");
            hidden = Tensor::cat(&[hidden.clone(), skip], 1)?;
            hidden = block.forward(&hidden, &time)?;
            hidden = attention.forward(&hidden)?;
            hidden = sampler.forward(&hidden)?;
        }

        let hidden = self.final_block.forward(&hidden)?;
        let hidden = self.final_norm.forward(&hidden)?;
        let hidden = candle_nn::ops::mish(&hidden)?;
        self.final_proj.forward(&hidden)
    }
}

/// 把匿名常量按 `decoder_weight_map.json` 的 target 落位写进参数表。
///
/// 解码器的命名张量（卷积核、norm、Bias 等）已由 `BaseDecoder::new` 登记参数位，
/// 剩下的匿名 `onnx::*` 常量（GroupNorm 仿射、qkv/to_out/ff 投影）靠映射表定位到
/// VarMap 里的目标张量并写入数值。`onnx::*` 里两张 `mel_scale` 也一并写。
pub fn bind_anonymous_weights(
    var_map: &VarMap,
    tensors: &[super::onnx_weights::OnnxTensor],
    map_path: &Path,
    device: &Device,
) -> anyhow::Result<usize> {
    use std::collections::HashMap;
    let bytes = std::fs::read(map_path)?;
    let map: HashMap<String, serde_json::Value> = serde_json::from_slice(&bytes)?;
    let by_name: HashMap<&str, &super::onnx_weights::OnnxTensor> =
        tensors.iter().map(|t| (t.name.as_str(), t)).collect();

    let guard = var_map.data();
    let guard = guard.lock().unwrap();
    let mut bound = 0usize;
    for (src, info) in &map {
        let Some(target) = info.get("target").and_then(|v| v.as_str()) else {
            continue;
        };
        // 只落位解码器路径；编码器匿名归一化已由 base_encoder 单独处理。
        if !target.starts_with("model.decoder.") {
            continue;
        }
        // 统一路径分隔符：映射表里 attn 路径混用 `/` 与 `.`。
        let target = target.replace('/', ".");
        let Some(tensor) = by_name.get(src.as_str()) else {
            continue;
        };
        let Some(var) = guard.get(&target) else {
            anyhow::bail!("参数表缺少映射目标: {target}");
        };
        // Linear 投影权重：candle 存 [out,in]，ONNX MatMul 存 [in,out]，需转置；
        // 归一化仿射（Mul/Add）保持原方向。
        let transposed = src.starts_with("onnx::MatMul_");
        let dims: Vec<usize> = if transposed {
            tensor.dims.iter().rev().copied().collect()
        } else {
            tensor.dims.clone()
        };
        if var.dims() != dims.as_slice() {
            anyhow::bail!(
                "映射形状不符 {src}: 参数表 {:?} 图 {:?}",
                var.dims(),
                tensor.dims
            );
        }
        let value = if transposed {
            Tensor::from_vec(tensor.data.clone(), tensor.dims.clone(), device)?
                .t()?
                .contiguous()?
        } else {
            Tensor::from_vec(tensor.data.clone(), tensor.dims.clone(), device)?
        };
        var.set(&value)?;
        bound += 1;
    }
    Ok(bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::DType;
    use crate::ai_service::tts::finetune::onnx_weights::read_initializers;
    use std::path::Path;

    fn base_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx")
    }

    #[test]
    fn named_decoder_tensors_have_slots() {
        let path = base_path();
        if !path.exists() {
            return;
        }

        let device = Device::Cpu;
        let var_map = candle_nn::VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, candle_core::DType::F32, &device);
        BaseDecoder::new(&BaseDecoderConfig::default(), vb).unwrap();

        let tensors = read_initializers(&path).unwrap();
        let slots = var_map.data();
        let slots = slots.lock().unwrap();

        for tensor in tensors
            .iter()
            .filter(|tensor| tensor.name.starts_with("model.decoder."))
        {
            let Some(var) = slots.get(&tensor.name) else {
                panic!("缺少参数位: {}", tensor.name);
            };
            assert_eq!(var.dims(), tensor.dims.as_slice(), "形状不符: {}", tensor.name);
        }
    }

    #[test]
    fn forward_shape() {
        let device = Device::Cpu;
        let var_map = candle_nn::VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, candle_core::DType::F32, &device);
        let decoder = BaseDecoder::new(&BaseDecoderConfig::default(), vb).unwrap();

        let noisy = Tensor::randn(0f32, 1f32, (1, 80, 32), &device).unwrap();
        let text = Tensor::randn(0f32, 1f32, (1, 80, 32), &device).unwrap();
        let time = Tensor::randn(0f32, 1f32, (1, 160), &device).unwrap();
        let output = decoder.forward(&noisy, &text, &time).unwrap();
        assert_eq!(output.dims()[0], 1);
        assert_eq!(output.dims()[1], 80);
        assert_eq!(output.rank(), 3);
    }

    #[test]
    fn anonymous_decoder_weights_bind() {
        let path = base_path();
        let map_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/tts-training/decoder_weight_map.json");
        if !path.exists() || !map_path.exists() {
            return;
        }

        let device = Device::Cpu;
        let var_map = candle_nn::VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, candle_core::DType::F32, &device);
        BaseDecoder::new(&BaseDecoderConfig::default(), vb).unwrap();

        let tensors = read_initializers(&path).unwrap();
        let bound = bind_anonymous_weights(&var_map, &tensors, &map_path, &device).unwrap();

        // 映射表里解码器目标应有 26 (GroupNorm 仿射) + 24 (attn) + 12 (ff) + 4 (proj_w норм)
        // + 0; 实际的解码器匿名目标数应大于 40。
        assert!(bound > 40, "解码器匿名权重绑定数量异常: {bound}");
    }

    /// 从 `tools/finetune_graph/ref/` 读取前向基准（onnxruntime 产出）并比对 Rust 解码器。
    #[test]
    fn forward_matches_onnxruntime() {
        let base = base_path();
        let map_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/tts-training/decoder_weight_map.json");
        let ref_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/finetune_graph/ref");
        let need = ["noisy.f32", "text.f32", "final.f32"];
        if !base.exists() || !map_path.exists() || !need.iter().all(|f| ref_dir.join(f).exists()) {
            eprintln!("基准数据缺失，跳过前向比对");
            return;
        }

        let device = Device::Cpu;
        let var_map = candle_nn::VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, candle_core::DType::F32, &device);
        let decoder = BaseDecoder::new(&BaseDecoderConfig::default(), vb).unwrap();

        let tensors = read_initializers(&base).unwrap();
        // 命名张量直接写入
        {
            let slots = var_map.data();
            let slots = slots.lock().unwrap();
            for t in tensors.iter().filter(|t| t.name.starts_with("model.decoder.")) {
                let var = slots.get(&t.name).expect("命名张量缺参数位");
                var.set(&Tensor::from_vec(t.data.clone(), t.dims.clone(), &device).unwrap()).unwrap();
            }
        }
        bind_anonymous_weights(&var_map, &tensors, &map_path, &device).unwrap();

        fn load_f32(path: &Path) -> Option<Vec<f32>> {
            let bytes = std::fs::read(path).ok()?;
            if bytes.len() % 4 != 0 {
                return None;
            }
            Some(bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
        }
        let noisy = load_f32(&ref_dir.join("noisy.f32")).unwrap();
        let text = load_f32(&ref_dir.join("text.f32")).unwrap();
        let reference = load_f32(&ref_dir.join("final.f32")).unwrap();

        // 参考数据形状 [1,80,48]。BaseDecoder 需要基准时间嵌入（图上 time_mlp 的常量输入）。
        let time = base_time_embedding(&device); // [1,160]
        let noise_t = Tensor::from_vec(noisy, (1, 80, 48), &device).unwrap();
        let text_t = Tensor::from_vec(text, (1, 80, 48), &device).unwrap();

        // 逐级比对：down/mid/up 各块的 ResBlock 与 Attn 输出。
        let staged = decoder.staged_outputs(&noise_t, &text_t, &time).unwrap();
        let stage_names = [
            "down0_res", "down0_attn", "down1_res", "down1_attn",
            "mid0_res", "mid0_attn", "mid1_res", "mid1_attn",
            "up0_res", "up0_attn", "up1_res", "up1_attn", "final_mel",
        ];
        let stage_files = [
            "res_down00", "attn_down00", "res_down10", "attn_down10",
            "res_mid00", "attn_mid00", "res_mid10", "attn_mid10",
            "res_up00", "attn_up00", "res_up10", "attn_up10",
            "final",
        ];
        for ((name, file), tens) in stage_names.iter().zip(stage_files.iter()).zip(staged.iter()) {
            let Some(r) = load_f32(&ref_dir.join(format!("{file}.f32"))) else {
                eprintln!("{name}: 缺少基准文件 {file}");
                continue;
            };
            let dims = tens.dims();
            let v: Vec<f32> = tens.flatten_all().unwrap().to_vec1::<f32>().unwrap();
            // candle 阶段输出统一为 [B,C,F]；注意力阶段的基座断点是 [B,F,C]，需转置成 [B,C,F] 再比。
            let ref_v = if name.contains("_attn") {
                let (b, c, f) = (dims[0], dims[1], dims[2]);
                let mut rv = Vec::with_capacity(r.len());
                for b_ in 0..b {
                    for c_ in 0..c {
                        for f_ in 0..f {
                            rv.push(r[(b_ * f + f_) * c + c_]);
                        }
                    }
                }
                rv
            } else {
                r
            };
            let d = v.iter().zip(ref_v.iter()).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
            eprintln!("{name:16} 最大差: {d:.6}");
            assert!(d < 1e-3, "阶段 {name} 与基座偏差过大: {d}");
        }

        let out = decoder.forward(&noise_t, &text_t, &time).unwrap();
        assert_eq!(out.dims(), [1, 80, 48], "前向输出形状不符");

        let out: Vec<f32> = out.to_vec3().unwrap().concat().concat();
        let max_diff = reference
            .iter()
            .zip(out.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        eprintln!("解码器前向 vs onnxruntime 最大差值: {max_diff}");
        assert!(
            max_diff < 1e-3,
            "Rust 解码器与基座前向偏差过大: {max_diff}"
        );
    }

    /// 手动转置卷积分解应严格等价于 candle 原生转置卷积前向（含偏置）。
    #[test]
    fn manual_transpose_matches_candle() {
        let device = Device::Cpu;
        let w = Tensor::randn(0f32, 1f32, (4, 4, 4), &device).unwrap();
        let bias = Tensor::randn(0f32, 0.5f32, (4,), &device).unwrap();
        let x = Tensor::randn(0f32, 1f32, (1, 4, 7), &device).unwrap();

        // 参考：原生转置卷积（纯卷积，candle 的 conv_transpose1d 只收 kernel）+ 偏置。
        let reference = x
            .conv_transpose1d(&w, 1, 0, 2, 1, 1)
            .unwrap()
            .broadcast_add(&bias.reshape((1, 4, 1)).unwrap())
            .unwrap();

        // 被测：手写分解，用同一权重与偏置覆盖登记的参数位。
        let mut var_map = candle_nn::VarMap::new();
        let vb = candle_nn::VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let manual = ManualConvTranspose1d::new(vb, 4, 4).unwrap();
        var_map.set_one("weight", &w).unwrap();
        var_map.set_one("bias", &bias).unwrap();
        let out = manual.forward(&x).unwrap();

        assert_eq!(out.dims(), reference.dims(), "输出形状应一致");
        let a: Vec<f32> = out.flatten_all().unwrap().to_vec1().unwrap();
        let b: Vec<f32> = reference.flatten_all().unwrap().to_vec1().unwrap();
        let max_diff = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .fold(0f32, f32::max);
        assert!(max_diff < 1e-5, "手动转置卷积与 candle 不匹配: {max_diff}");
    }
}

/// 基座上 `time_mlp` 的输入是内嵌常量：[160] 前半 0 后半 1（时间步的时刻）。
/// 这里按图上 `time_mlp/linear_1/Constant` 给出的值构造。
#[cfg(test)]
fn base_time_embedding(device: &Device) -> Tensor {
    let mut v = vec![0f32; 160];
    for i in 80..160 {
        v[i] = 1.0;
    }
    Tensor::from_vec(v, (1, 160), device).unwrap()
}