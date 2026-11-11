//! 条件流匹配解码器：输入投影 → 残差块 → 下采样 → 中间块 → 上采样跳连 → 输出投影。

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::{Conv1d, Conv1dConfig, GroupNorm, LayerNorm, Linear, Module, VarBuilder};

/// 解码器配置。
#[derive(Debug, Clone)]
pub struct CfmConfig {
    pub n_mels: usize,
    pub text_hidden: usize,
    pub base_channels: usize,
    pub time_dim: usize,
}

impl Default for CfmConfig {
    fn default() -> Self {
        Self {
            n_mels: 80,
            text_hidden: 192,
            base_channels: 128,
            time_dim: 64,
        }
    }
}

/// 带时间条件的残差块。
struct ResBlock {
    conv1: Conv1d,
    norm1: GroupNorm,
    conv2: Conv1d,
    norm2: GroupNorm,
    time_proj: Linear,
    channels: usize,
}

impl ResBlock {
    fn new(channels: usize, time_dim: usize, vb: VarBuilder) -> Result<Self> {
        let config = Conv1dConfig {
            padding: 1,
            ..Default::default()
        };
        let groups = channels.div_ceil(32).max(1);
        Ok(Self {
            conv1: candle_nn::conv1d(channels, channels, 3, config, vb.pp("conv1"))?,
            norm1: candle_nn::group_norm(groups, channels, 1e-5, vb.pp("norm1"))?,
            conv2: candle_nn::conv1d(channels, channels, 3, config, vb.pp("conv2"))?,
            norm2: candle_nn::group_norm(groups, channels, 1e-5, vb.pp("norm2"))?,
            time_proj: candle_nn::linear(time_dim, channels, vb.pp("time_proj"))?,
            channels,
        })
    }

    /// `xs` 为 [batch, channels, frames]，`time` 为 [batch, time_dim]。
    fn forward(&self, xs: &Tensor, time: &Tensor) -> Result<Tensor> {
        let (batch, _, frames) = xs.dims3()?;
        let bias = self
            .time_proj
            .forward(time)?
            .reshape((batch, self.channels, 1))?
            .broadcast_as((batch, self.channels, frames))?;

        let hidden = self.norm1.forward(xs)?;
        let hidden = silu(&hidden)?;
        let hidden = self.conv1.forward(&hidden)?;
        let hidden = hidden.broadcast_add(&bias)?;

        let hidden = self.norm2.forward(&hidden)?;
        let hidden = silu(&hidden)?;
        let hidden = self.conv2.forward(&hidden)?;
        xs + hidden
    }
}

/// 条件流匹配解码器。
pub struct CfmDecoder {
    input_proj: Conv1d,
    block1: ResBlock,
    down: Conv1d,
    block2: ResBlock,
    mid: ResBlock,
    up_proj: Conv1d,
    block3: ResBlock,
    out_norm: LayerNorm,
    out_proj: Conv1d,
    time_mlp1: Linear,
    time_mlp2: Linear,
    time_dim: usize,
    base_channels: usize,
    n_mels: usize,
}

impl CfmDecoder {
    /// 构造解码器。
    pub fn new(config: &CfmConfig, vb: VarBuilder) -> Result<Self> {
        let base = config.base_channels;
        let double = base * 2;

        Ok(Self {
            input_proj: candle_nn::conv1d(
                config.n_mels + config.text_hidden,
                base,
                1,
                Default::default(),
                vb.pp("input_proj"),
            )?,
            block1: ResBlock::new(base, config.time_dim, vb.pp("block1"))?,
            down: candle_nn::conv1d(
                base,
                double,
                3,
                Conv1dConfig {
                    padding: 1,
                    stride: 2,
                    ..Default::default()
                },
                vb.pp("down"),
            )?,
            block2: ResBlock::new(double, config.time_dim, vb.pp("block2"))?,
            mid: ResBlock::new(double, config.time_dim, vb.pp("mid"))?,
            up_proj: candle_nn::conv1d(
                double + base,
                base,
                1,
                Default::default(),
                vb.pp("up_proj"),
            )?,
            block3: ResBlock::new(base, config.time_dim, vb.pp("block3"))?,
            out_norm: candle_nn::layer_norm(base, 1e-5, vb.pp("out_norm"))?,
            out_proj: candle_nn::conv1d(
                base,
                config.n_mels,
                1,
                Default::default(),
                vb.pp("out_proj"),
            )?,
            time_mlp1: candle_nn::linear(config.time_dim, config.time_dim * 2, vb.pp("time1"))?,
            time_mlp2: candle_nn::linear(config.time_dim * 2, config.time_dim, vb.pp("time2"))?,
            time_dim: config.time_dim,
            base_channels: base,
            n_mels: config.n_mels,
        })
    }

    /// 前向；`noisy`/`text` 为 [batch, C, frames]，`time` 取值 0 到 1。
    pub fn forward(&self, noisy: &Tensor, text: &Tensor, time: &Tensor) -> Result<Tensor> {
        let device = noisy.device();
        let embedded_time = self.embed_time(time, device)?;

        let (batch, _, frames) = noisy.dims3()?;
        let input = Tensor::cat(&[noisy.clone(), text.clone()], 1)?;
        let hidden = self.input_proj.forward(&input)?;

        let skip = self.block1.forward(&hidden, &embedded_time)?;
        let down = self.down.forward(&skip)?;
        let middle = self.block2.forward(&down, &embedded_time)?;
        let middle = self.mid.forward(&middle, &embedded_time)?;

        let upsampled = upsample_x2(&middle, frames)?;
        let merged = Tensor::cat(&[upsampled, skip], 1)?;
        let merged = self.up_proj.forward(&merged)?;
        let hidden = self.block3.forward(&merged, &embedded_time)?;

        let hidden = self
            .out_norm
            .forward(&hidden.transpose(1, 2)?.contiguous()?)?;
        let hidden = silu(&hidden)?;
        let hidden = hidden.transpose(1, 2)?.contiguous()?;
        let output = self.out_proj.forward(&hidden)?;

        debug_assert_eq!(output.dims(), &[batch, self.n_mels, frames]);
        Ok(output)
    }

    fn embed_time(&self, time: &Tensor, device: &Device) -> Result<Tensor> {
        let sinusoidal = sinusoidal_embedding(time, self.time_dim, device)?;
        let hidden = self.time_mlp1.forward(&sinusoidal)?;
        let hidden = silu(&hidden)?;
        self.time_mlp2.forward(&hidden)
    }

    /// 基础通道数。
    pub fn base_channels(&self) -> usize {
        self.base_channels
    }
}

/// 条件流匹配损失：预测速度与目标速度的均方误差。
pub fn flow_matching_loss(predicted: &Tensor, target: &Tensor) -> Result<Tensor> {
    predicted.sub(target)?.sqr()?.mean_all()
}

/// 构造训练用加噪样本：x_t = (1 - t) * x0 + t * x1，目标速度为 x1 - x0。
pub fn add_noise(data: &Tensor, noise: &Tensor, time: f64) -> Result<(Tensor, Tensor)> {
    let noisy = (data.affine(1.0 - time, 0.0)? + noise.affine(time, 0.0)?)?;
    let target = noise.sub(data)?;
    Ok((noisy, target))
}

/// 一维上采样两倍（最近邻复制）。
fn upsample_x2(xs: &Tensor, target_frames: usize) -> Result<Tensor> {
    let (batch, channels, frames) = xs.dims3()?;
    let doubled = xs
        .unsqueeze(3)?
        .broadcast_as((batch, channels, frames, 2))?
        .contiguous()?
        .reshape((batch, channels, frames * 2))?;
    if frames * 2 == target_frames {
        return Ok(doubled);
    }
    // 下采样时奇数帧会差一帧，补齐或裁到目标长度。
    if frames * 2 > target_frames {
        doubled.narrow(2, 0, target_frames)
    } else {
        let padding = target_frames - frames * 2;
        let last = doubled.narrow(2, frames * 2 - 1, 1)?;
        let repeated = last.broadcast_as((batch, channels, padding))?;
        Tensor::cat(&[doubled, repeated], 2)
    }
}

fn sinusoidal_embedding(time: &Tensor, dim: usize, device: &Device) -> Result<Tensor> {
    let half = dim / 2;
    let mut frequencies = Vec::with_capacity(half);
    for index in 0..half {
        let exponent = -(index as f64) * 10_000f64.ln() / half as f64;
        frequencies.push(exponent.exp() as f32);
    }
    let frequencies = Tensor::from_vec(frequencies, (1, half), device)?;
    let angles = time.unsqueeze(1)?.matmul(&frequencies)?;
    let sin = angles.sin()?;
    let cos = angles.cos()?;
    Tensor::cat(&[sin, cos], 1)?.to_dtype(DType::F32)
}

fn silu(xs: &Tensor) -> Result<Tensor> {
    let sigmoid = candle_nn::ops::sigmoid(xs)?;
    xs.mul(&sigmoid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};

    fn config() -> CfmConfig {
        CfmConfig {
            n_mels: 8,
            text_hidden: 16,
            base_channels: 8,
            time_dim: 8,
        }
    }

    #[test]
    fn decoder_output_matches_input_shape() -> Result<()> {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let decoder = CfmDecoder::new(&config(), vb)?;

        let noisy = Tensor::randn(0f32, 1f32, (1, 8, 20), &device)?;
        let text = Tensor::randn(0f32, 1f32, (1, 16, 20), &device)?;
        let time = Tensor::from_vec(vec![0.5f32], (1,), &device)?;
        let output = decoder.forward(&noisy, &text, &time)?;

        assert_eq!(output.dims(), &[1, 8, 20]);
        assert_eq!(decoder.base_channels(), 8);
        Ok(())
    }

    #[test]
    fn odd_frame_count_is_padded() -> Result<()> {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let decoder = CfmDecoder::new(&config(), vb)?;

        let noisy = Tensor::randn(0f32, 1f32, (1, 8, 21), &device)?;
        let text = Tensor::randn(0f32, 1f32, (1, 16, 21), &device)?;
        let time = Tensor::from_vec(vec![0.1f32], (1,), &device)?;
        let output = decoder.forward(&noisy, &text, &time)?;
        assert_eq!(output.dims(), &[1, 8, 21]);
        Ok(())
    }

    #[test]
    fn add_noise_interpolates_endpoints() -> Result<()> {
        let device = Device::Cpu;
        let data = Tensor::new(&[[[1.0f32]]], &device)?;
        let noise = Tensor::new(&[[[3.0f32]]], &device)?;

        let (at_zero, _) = add_noise(&data, &noise, 0.0)?;
        let (at_one, target) = add_noise(&data, &noise, 1.0)?;
        let (at_mid, _) = add_noise(&data, &noise, 0.5)?;

        let scalar = |tensor: &Tensor| -> Result<f32> { tensor.sum_all()?.to_scalar::<f32>() };

        assert!((scalar(&at_zero)? - 1.0).abs() < 1e-5);
        assert!((scalar(&at_one)? - 3.0).abs() < 1e-5);
        assert!((scalar(&at_mid)? - 2.0).abs() < 1e-5);
        assert!((scalar(&target)? - 2.0).abs() < 1e-5);
        Ok(())
    }

    #[test]
    fn flow_matching_step_reduces_loss() -> Result<()> {
        // 固定输入反复优化，损失应明显下降。
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let decoder = CfmDecoder::new(&config(), vb)?;

        let data = Tensor::randn(0f32, 1f32, (1, 8, 12), &device)?;
        let noise = Tensor::randn(0f32, 1f32, (1, 8, 12), &device)?;
        let text = Tensor::randn(0f32, 1f32, (1, 16, 12), &device)?;
        let time = Tensor::from_vec(vec![0.5f32], (1,), &device)?;
        let (noisy, target) = add_noise(&data, &noise, 0.5)?;

        let params = ParamsAdamW {
            lr: 0.01,
            ..Default::default()
        };
        let mut optimizer = AdamW::new(var_map.all_vars(), params)?;

        let mut first = f32::NAN;
        let mut last = f32::NAN;
        for step in 0..100 {
            let predicted = decoder.forward(&noisy, &text, &time)?;
            let loss = flow_matching_loss(&predicted, &target)?;
            let value: f32 = loss.to_scalar()?;
            if step == 0 {
                first = value;
            }
            last = value;
            optimizer.backward_step(&loss)?;
        }

        assert!(first.is_finite() && last.is_finite());
        assert!(
            last < first * 0.7,
            "flow matching loss did not drop: first {first}, last {last}"
        );
        Ok(())
    }
}
