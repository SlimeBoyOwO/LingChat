//! 基于基座模型的对齐训练（burn/Vulkan 后端）。
//!
//! 训练沿用基座的 CFM 配方：编码器只加载基座权重、不参与梯度（时长与 mel 先验同样
//! 固定用基座预测），只有解码器参数被优化。这与「五分钟数据微调音色」的目标一致。
//!
//! 张量计算全部落在 burn 的 autodiff 设备上，物理后端固定为 Vulkan（不启用 CPU
//! 回退，见 [`burn_backend`](super::burn_backend)）。检查点是 burn 原生 `.bpk` 格式。

use std::path::Path;

use anyhow::{Context, Result};
use burn::module::Module;
use burn::optim::{AdamWConfig, GradientsParams};
use burn::prelude::*;
use burn::tensor::Distribution;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::burn_backend::train_device_autodiff;
use super::burn_decoder::{
    BurnBaseDecoder, BurnBaseDecoderConfig, from_onnx_tensors as load_decoder_from_onnx,
};
use super::burn_encoder::{
    BurnBaseEncoderConfig, from_onnx_tensors as load_encoder_from_onnx,
};
use super::corpus::{Corpus, make_batches};
use super::onnx_weights::read_initializers;

/// 训练配置。
#[derive(Debug, Clone)]
pub struct BaseTrainingConfig {
    /// 训练轮数。
    pub epochs: usize,
    /// 学习率。
    pub learning_rate: f64,
    /// 打乱顺序与噪声的随机种子。
    pub seed: u64,
    /// 每多少轮存一次检查点。
    pub checkpoint_every: usize,
    /// 每多少步触发一次显式设备内存回收。
    ///
    /// burn/cubecl 的 wgpu 内存池会把训练中间张量占用的页保留在池里以便复用，
    /// 但 Adreno 这类移动 GPU 上池内已归还的空闲页若不显式交还驱动，会随步数
    /// 累积挤占设备内存，最终 OOM。这里按步周期性触发
    /// [`Device::memory_cleanup`]，把空闲页归还驱动，对冲「越跑越崩」。
    /// 0 表示不开启。
    pub cleanup_every: usize,
}

impl Default for BaseTrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 20,
            learning_rate: 1e-3,
            seed: 1,
            checkpoint_every: 1,
            cleanup_every: 50,
        }
    }
}

/// 训练进度。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BaseTrainingProgress {
    /// 当前轮。
    pub epoch: usize,
    /// 总轮数。
    pub total_epochs: usize,
    /// 已完成的步数。
    pub step: usize,
    /// 总步数。
    pub total_steps: usize,
    /// 当前损失。
    pub loss: f32,
}

/// 训练结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BaseTrainingOutcome {
    /// 实际执行的步数。
    pub steps: usize,
    /// 首步损失。
    pub initial_loss: f32,
    /// 末步损失。
    pub final_loss: f32,
    /// 是否被取消。
    pub cancelled: bool,
}

/// 生成解码器时间 MLP 的 160 维正弦嵌入，形状 `[1,160]`。
///
/// 公式来自 MatchaTTS `SinusoidalPosEmb(dim=160)`，`scale=1000`：
/// `freq[i]=exp(-ln(10000)*i/79)`，`angle=1000*t*freq`，前半 sin、后半 cos。
fn base_time_embed(t: f64, device: &Device) -> Tensor<2> {
    const HALF: usize = 80;
    let mut values = vec![0f32; HALF * 2];
    let log_10000 = 10000f32.ln();
    for i in 0..HALF {
        let freq = (-log_10000 * i as f32 / (HALF - 1) as f32).exp();
        let angle = 1000.0 * t as f32 * freq;
        values[i] = angle.sin();
        values[HALF + i] = angle.cos();
    }
    Tensor::from_data(TensorData::new(values, [1, HALF * 2]), device)
}

/// 把 mel 先验按时长展开到目标帧数。
///
/// `mel_prior` 是编码器输出的 mel 先验，形状 `[1, c, tokens]`；`durations` 是逐 token
/// 对数时长，形状 `[1, tokens]`，先缩放到目标帧数再按四舍五入帧数重复展开。
/// 返回 `[1, c, target_frames]` 的浮点值，由调用方落到训练设备。
fn expand_prior(
    mel_prior: &Tensor<3>,
    durations: &Tensor<2>,
    target_frames: usize,
) -> Result<(Vec<f32>, usize)> {
    let [batch, channels, _tokens] = mel_prior.dims();
    if batch != 1 {
        anyhow::bail!("仅支持单样本展开");
    }

    let dur: Vec<f32> = durations.to_data().try_to_vec::<f32>()?;
    let tokens = dur.len();
    let seconds: Vec<f32> = dur.iter().map(|value| value.exp()).collect();
    let sum: f32 = seconds.iter().sum();
    let scale = if sum > 1e-3 {
        target_frames as f32 / sum
    } else {
        0.0
    };

    let mut frames_per_token: Vec<usize> = seconds
        .iter()
        .map(|value| ((value * scale).round() as usize).max(1))
        .collect();
    // 多退少补，保证总帧数正好是目标帧数。
    let mut total: usize = frames_per_token.iter().sum();
    while total > target_frames {
        let Some(index) = frames_per_token
            .iter()
            .enumerate()
            .filter(|(_, value)| **value > 1)
            .max_by_key(|(_, value)| **value)
            .map(|(index, _)| index)
        else {
            break;
        };
        frames_per_token[index] -= 1;
        total -= 1;
    }
    while total < target_frames {
        let last = frames_per_token.len() - 1;
        frames_per_token[last] += 1;
        total += 1;
    }

    let frame_values: Vec<f32> = mel_prior.to_data().try_to_vec::<f32>()?;
    let mut expanded = Vec::with_capacity(channels * target_frames);
    for token in 0..tokens {
        for _ in 0..frames_per_token[token] {
            for ci in 0..channels {
                expanded.push(frame_values[token * channels + ci]);
            }
        }
    }
    debug_assert_eq!(expanded.len(), channels * target_frames);
    Ok((expanded, channels))
}

/// 构造训练用的噪声数据（matcha 源实现的 CFM 公式）。
///
/// 返回 `(noisy=y, target=u)`。`sigma_min=1e-4`：
/// `y = (1-(1-sigma)*t)*z + t*x1`，`u = x1-(1-sigma)*z`。
fn cfm_noise(mel: &Tensor<3>, noise: &Tensor<3>, time: f64, sigma_min: f64) -> (Tensor<3>, Tensor<3>) {
    let t = time as f32;
    let noisy = noise.clone() * (1.0 - (1.0 - sigma_min as f32) * t) + mel.clone() * t;
    let target = mel.clone() - noise.clone() * (1.0 - sigma_min as f32);
    (noisy, target)
}

/// 执行基座对齐训练：只更新解码器，编码器与时长固定用基座预测。
///
/// `base_model` 是基座 ONNX 路径，从中读取权重填充编码器与解码器；`map_path` 是
/// 匿名投影权重映射表（`decoder_weight_map.json`）。`device` 是物理 Vulkan 设备，
/// 训练在其 autodiff 上下文中进行。
#[allow(clippy::too_many_arguments)]
pub fn train_base(
    corpus: &Corpus,
    encoder_config: &BurnBaseEncoderConfig,
    decoder_config: &BurnBaseDecoderConfig,
    config: &BaseTrainingConfig,
    base_model: &Path,
    map_path: &Path,
    checkpoint_path: Option<&Path>,
    cancel: Option<&CancellationToken>,
    mut on_progress: impl FnMut(&BaseTrainingProgress),
) -> Result<BaseTrainingOutcome> {
    if corpus.samples.is_empty() {
        anyhow::bail!("语料为空，无法训练");
    }
    if !map_path.is_file() {
        anyhow::bail!(
            "缺少解码器权重映射表: {}（请确认它随程序数据分发）",
            map_path.display()
        );
    }

    tracing::info!("[训练] 载入基座权重: {}", base_model.display());
    let tensors = read_initializers(base_model)
        .with_context(|| format!("读取基座权重失败: {}", base_model.display()))?;

    // 训练固定走 Vulkan 的 autodiff 设备（不在参数里暴露，避免误传 CPU）。
    let train_dev = train_device_autodiff();

    // 编码器：加载基座权重后只做前向（先验会被抽取成数据、切断计算图，参数不进梯度）。
    let encoder = load_encoder_from_onnx(encoder_config, &tensors, &train_dev)
        .map_err(anyhow::Error::msg)?;
    let encoder = encoder.no_grad();

    // 解码器：加载基座权重后进入训练模式（参数要求梯度）。
    let mut decoder = load_decoder_from_onnx(decoder_config, &tensors, map_path, &train_dev)
        .map_err(anyhow::Error::msg)?;
    decoder = decoder.train();

    // 每样本预计算 mel 先验并展开到帧级：编码器冻结，整轮训练只算一次。
    let mut priors: Vec<Vec<f32>> = Vec::with_capacity(corpus.samples.len());
    for sample in &corpus.samples {
        let len = sample.token_ids.len();
        let tokens = Tensor::<2, Int>::from_data(
            TensorData::new(sample.token_ids.clone(), [1, len]),
            &train_dev,
        );
        let (mel_prior, durations) = encoder.forward(tokens);
        let (expanded, _channels) =
            expand_prior(&mel_prior, &durations, sample.frames())
                .with_context(|| format!("样本 {} 先验展开失败", sample.id))?;
        priors.push(expanded);
    }

    let total_steps = config.epochs * corpus.samples.len();
    tracing::info!(
        "[训练] 开始：样本 {} 条，{} 轮共 {total_steps} 步，lr={}",
        corpus.samples.len(),
        config.epochs,
        config.learning_rate
    );
    let mut optimizer = AdamWConfig::new()
        .with_weight_decay(0.0)
        .with_grad_clipping(Some(burn::grad_clipping::GradientClippingConfig::Norm(1.0)))
        .init();

    let mut initial_loss = f32::NAN;
    let mut final_loss = f32::NAN;
    let mut step = 0usize;
    let mut cancelled = false;

    'outer: for epoch in 0..config.epochs {
        let order = make_batches(corpus.samples.len(), usize::MAX, config.seed ^ epoch as u64);
        let order: Vec<usize> = order.into_iter().flatten().collect();

        for index in order {
            if cancel.is_some_and(|token| token.is_cancelled()) {
                cancelled = true;
                tracing::info!(
                    "[训练] 收到取消，提前结束（第 {} 轮，已完成 {step} 步）",
                    epoch + 1
                );
                break 'outer;
            }

            let sample = &corpus.samples[index];
            let mel_ch = sample.mel[0].len();
            let frames = sample.frames();
            let prior_values = &priors[index];

            // mel 展开成 [1, 80, frames]（先按帧序的行优先，再转成通道在前的布局）。
            let mel_flat: Vec<f32> = sample
                .mel
                .iter()
                .flat_map(|frame| frame.iter().copied())
                .collect();
            // mel 行优先 [frames, mel_ch] 展开成 [1, frames, mel_ch] 再转置为
            // [1, mel_ch, frames]，与 candle 版行为一致。
            let mel = Tensor::<3>::from_data(
                TensorData::new(mel_flat, [1, frames, mel_ch]),
                &train_dev,
            )
            .transpose();

            // 噪声用标准正态分布生成，落在训练设备上参与前向与反向。
            let noise = Tensor::<3>::random(
                [1, frames, mel_ch],
                Distribution::Normal(0.0, 1.0),
                &train_dev,
            )
            .transpose();

            let time_value = rand_uniform(config.seed ^ step as u64 ^ 0x9e37);
            let (noisy, target) = cfm_noise(&mel, &noise, time_value, 1e-4);
            let time = base_time_embed(time_value, &train_dev);

            let prior = Tensor::<3>::from_data(
                TensorData::new(prior_values.to_vec(), [1, mel_ch, frames]),
                &train_dev,
            );

            let predicted = decoder.forward(noisy, prior, time);
            let loss = (predicted - target).powf_scalar(2.0).mean();

            let value: f32 = loss.to_data().try_to_vec::<f32>()?[0];
            if initial_loss.is_nan() {
                initial_loss = value;
            }
            final_loss = value;

            let grads = GradientsParams::from_grads(loss.backward(), &decoder);
            decoder = optimizer.step(config.learning_rate, decoder, grads);
            step += 1;

            // 周期归还设备内存：把池里已无引用的页交还驱动，避免跨步累积导致的
            // 移动端 OOM（Adreno 等设备对长期驻留的设备内存尤其敏感）。
            if config.cleanup_every > 0 && step % config.cleanup_every == 0 {
                train_dev.memory_cleanup();
            }

            on_progress(&BaseTrainingProgress {
                epoch: epoch + 1,
                total_epochs: config.epochs,
                step,
                total_steps,
                loss: value,
            });
        }

        if config.checkpoint_every > 0 && (epoch + 1) % config.checkpoint_every == 0 {
            if let Some(path) = checkpoint_path {
                save_checkpoint(&decoder, path)?;
                tracing::info!("[训练] 第 {} 轮结束，已保存检查点: {}", epoch + 1, path.display());
            }
        }
    }

    tracing::info!(
        "[训练] 结束：共 {step} 步，初损 {initial_loss:.6}，末损 {final_loss:.6}，取消={cancelled}"
    );
    Ok(BaseTrainingOutcome {
        steps: step,
        initial_loss,
        final_loss,
        cancelled,
    })
}

/// 保存解码器检查点（burn `.bpk` 格式）。
pub fn save_checkpoint(decoder: &BurnBaseDecoder, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建检查点目录失败: {}", parent.display()))?;
    }
    decoder
        .clone()
        .save_file(path)
        .with_context(|| format!("保存检查点失败: {}", path.display()))
}

/// 读取解码器检查点，填进给定的解码器骨架并返回。
pub fn load_checkpoint(decoder: &BurnBaseDecoder, path: &Path) -> Result<BurnBaseDecoder> {
    decoder
        .clone()
        .try_load_file(path)
        .with_context(|| format!("读取检查点失败: {}", path.display()))
}

/// xorshift 伪随机，用于时间步采样（与数据打乱解耦，避免占用 make_batches 的种子）。
fn rand_uniform(seed: u64) -> f64 {
    let mut v = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    v ^= v << 13;
    v ^= v >> 7;
    v ^= v << 17;
    (v >> 40) as f64 / (1u64 << 24) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_embed_len_and_axis() {
        let device = super::super::burn_backend::train_device();
        let emb = base_time_embed(0.0, &device);
        assert_eq!(emb.dims(), [1, 160]);
        // t=0 时 sin(0)=0、cos(0)=1，前半全 0、后半全 1。
        let v: Vec<f32> = emb.clone().to_data().try_to_vec::<f32>().unwrap();
        assert!(v.iter().take(80).all(|x| x.abs() < 1e-6));
        assert!(v.iter().skip(80).all(|x| (x - 1.0).abs() < 1e-6));
    }

    /// 集成验证：真实基座权重上跑 burn 训练，损失有界且步数为正。
    #[test]
    fn train_base_loads_base_weights_and_runs() {
        use std::path::Path;

        use crate::ai_service::tts::finetune::corpus::{Corpus, CorpusStats, TrainingSample};

        let onnx = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx");
        let map = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/tts-training/decoder_weight_map.json");
        if !onnx.exists() || !map.exists() {
            eprintln!("基座权重缺失，跳过集成测试");
            return;
        }

        let mel: Vec<Vec<f32>> = (0..120)
            .map(|i| (0..80).map(|j| 0.5 * ((i as f32 * 0.05) + j as f32).sin()).collect())
            .collect();
        let sample = TrainingSample {
            id: "tiny".to_string(),
            token_ids: vec![5, 10, 20, 33],
            mel,
        };
        let corpus = Corpus {
            samples: vec![sample],
            skipped: vec![],
            stats: CorpusStats::default(),
        };

        let config = BaseTrainingConfig {
            epochs: 1,
            learning_rate: 1e-3,
            seed: 7,
            checkpoint_every: 0,
            cleanup_every: 0,
        };
        let outcome = train_base(
            &corpus,
            &BurnBaseEncoderConfig::default(),
            &BurnBaseDecoderConfig::default(),
            &config,
            &onnx,
            &map,
            None,
            None,
            |_| {},
        )
        .expect("burn 基座对齐训练失败");
        assert!(outcome.steps > 0, "训练步数应为正");
        assert!(outcome.initial_loss.is_finite(), "初损应为有限值");
        assert!(outcome.final_loss.is_finite(), "末损应为有限值");
    }
}