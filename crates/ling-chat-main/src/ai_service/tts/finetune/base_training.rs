//! 基于基座模型的对齐训练。
//!
//! 训练沿用基座的 CFM 配方：编码器只加载基座权重、不参与梯度（时长同样固定用基座预测），
//! 只有解码器参数被优化。这与「五分钟数据微调音色」的目标一致，且不依赖外部训练管线。
//!
//! 关键公式（与 MatchaTTS 源实现逐条核对过）：
//! - 时间正弦嵌入 `SinusoidalPosEmb(dim=160, scale=1000)`，
//!   `freq[i]=exp(-ln(10000)*i/79)`，`emb[i]=sin(1000*t*freq[i])`、`emb[80+i]=cos(...)`。
//! - 条件流匹配：随机 `t~U(0,1)`、`z~N(0,1)`，加噪样本 `y=(1-(1-sigma)*t)z + t*x1`，
//!   速度真值 `u=x1-(1-sigma)z`，`sigma=1e-4`，损失为速度的均方误差。

use std::path::Path;

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::base_decoder::{BaseDecoder, BaseDecoderConfig};
use super::base_encoder::{BaseEncoder, BaseEncoderConfig};
use super::corpus::{Corpus, make_batches};
use super::decoder::flow_matching_loss;
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
}

impl Default for BaseTrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 20,
            learning_rate: 1e-3,
            seed: 1,
            checkpoint_every: 1,
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

/// 把基座 ONNX 的权重按名填进编码器参数表。
fn load_encoder_weights(
    var_map: &VarMap,
    tensors: &[super::onnx_weights::OnnxTensor],
    device: &Device,
) -> Result<()> {
    super::base_encoder::bind_weights(var_map, tensors, device)
        .context("编码器命名权重填充失败")?;
    super::base_encoder::bind_normalizations(var_map, tensors, 192, 6, device)
        .context("编码器归一化权重填充失败")?;
    Ok(())
}

/// 把基座 ONNX 的解码器权重填进参数表（命名张量 + 匿名投影按映射表）。
pub fn load_decoder_weights(
    var_map: &VarMap,
    tensors: &[super::onnx_weights::OnnxTensor],
    map_path: &Path,
    device: &Device,
) -> Result<()> {
    let guard = var_map.data();
    let guard = guard.lock().unwrap();
    for tensor in tensors {
        if !tensor.name.starts_with("model.decoder.") {
            continue;
        }
        let Some(var) = guard.get(&tensor.name) else {
            continue;
        };
        let value = Tensor::from_vec(tensor.data.clone(), tensor.dims.clone(), device)?;
        var.set(&value)?;
    }
    drop(guard);
    super::base_decoder::bind_anonymous_weights(var_map, tensors, map_path, device)
        .context("解码器匿名权重绑定失败")?;
    Ok(())
}

/// 生成解码器时间 MLP 的 160 维正弦嵌入，形状 `[1,160]`。
///
/// 公式来自 MatchaTTS `SinusoidalPosEmb(dim=160)`，`scale=1000`：
/// `freq[i]=exp(-ln(10000)*i/79)`，`angle=1000*t*freq`，前半 sin、后半 cos。
fn base_time_embed(t: f64, device: &Device) -> Result<Tensor> {
    const HALF: usize = 80;
    let mut values = vec![0f32; HALF * 2];
    let log_10000 = 10000f32.ln();
    for i in 0..HALF {
        let freq = (-log_10000 * i as f32 / (HALF - 1) as f32).exp();
        let angle = 1000.0 * t as f32 * freq;
        values[i] = angle.sin();
        values[HALF + i] = angle.cos();
    }
    Ok(Tensor::from_vec(values, (1, HALF * 2), device)?)
}

/// 把 mel 先验按时长展开到目标帧数。
///
/// `mel_prior` 形状 `[1, c, tokens]`，`durations` 是编码器预测的逐 token 对数时长。
/// 先缩放到目标帧数，再按四舍五入后的帧数重复展开。
fn expand_prior(
    mel_prior: &Tensor,
    durations: &Tensor,
    target_frames: usize,
    device: &Device,
) -> Result<Tensor> {
    let (batch, channels, _tokens) = mel_prior.dims3()?;
    if batch != 1 {
        anyhow::bail!("仅支持单样本展开");
    }

    let dur: Vec<f32> = durations.flatten_all()?.to_vec1::<f32>()?;
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

    let frame_values: Vec<f32> = mel_prior.flatten_all()?.to_vec1::<f32>()?;
    let mut expanded = Vec::with_capacity(channels * target_frames);
    for token in 0..tokens {
        for _ in 0..frames_per_token[token] {
            for ci in 0..channels {
                expanded.push(frame_values[token * channels + ci]);
            }
        }
    }
    debug_assert_eq!(expanded.len(), channels * target_frames);
    Ok(Tensor::from_vec(
        expanded,
        (1, channels, target_frames),
        device,
    )?)
}

/// 构造训练用的噪声数据（matcha 源实现的 CFM 公式）。
///
/// 返回 `(noisy=y, target=u)`。`sigma=1e-4`。
fn cfm_noise(mel: &Tensor, noise: &Tensor, time: f64, sigma_min: f64) -> Result<(Tensor, Tensor)> {
    let noisy = (noise.affine(1.0 - (1.0 - sigma_min) * time, 0.0)? + mel.affine(time, 0.0)?)?;
    let target = (mel - noise.affine(1.0 - sigma_min, 0.0)?)?;
    Ok((noisy, target))
}

/// 执行基座对齐训练：只更新解码器，编码器与时长固定用基座预测。
///
/// `base_model` 是基座 ONNX 路径，从中读取权重填充编码器与解码器；
/// `map_path` 是匿名投影权重映射表（`decoder_weight_map.json`）。
#[allow(clippy::too_many_arguments)]
pub fn train_base(
    corpus: &Corpus,
    encoder_config: &BaseEncoderConfig,
    decoder_config: &BaseDecoderConfig,
    config: &BaseTrainingConfig,
    base_model: &Path,
    map_path: &Path,
    checkpoint_path: Option<&Path>,
    cancel: Option<&CancellationToken>,
    device: &Device,
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

    // 编码器：独立 VarMap，只加载基座权重，不进优化器（时长与 mel 先验固定）。
    let encoder_vm = VarMap::new();
    let encoder_vb = VarBuilder::from_varmap(&encoder_vm, DType::F32, device);
    let encoder = BaseEncoder::new(encoder_config, encoder_vb)?;
    load_encoder_weights(&encoder_vm, &tensors, device)?;

    // 解码器：独立 VarMap，加载基座权重后参与训练。
    let decoder_vm = VarMap::new();
    let decoder_vb = VarBuilder::from_varmap(&decoder_vm, DType::F32, device);
    let decoder = BaseDecoder::new(decoder_config, decoder_vb)?;
    load_decoder_weights(&decoder_vm, &tensors, map_path, device)
        .with_context(|| format!("填充解码器权重失败，映射表: {}", map_path.display()))?;

    // 每样本预计算 mel 先验并展开到帧级：编码器冻结，整轮训练只算一次。
    let mut priors = Vec::with_capacity(corpus.samples.len());
    for sample in &corpus.samples {
        let tokens = Tensor::from_vec(
            sample.token_ids.clone(),
            (1, sample.token_ids.len()),
            device,
        )?;
        let (mel_prior, durations) = encoder.forward(&tokens)?;
        let expanded = expand_prior(&mel_prior, &durations, sample.frames(), device)
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
    let params = ParamsAdamW {
        lr: config.learning_rate,
        ..Default::default()
    };
    let mut optimizer = AdamW::new(decoder_vm.all_vars(), params)?;
    let mut rng = Rng::new(config.seed);

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
                tracing::info!("[训练] 收到取消，提前结束（第 {} 轮，已完成 {step} 步）", epoch + 1);
                break 'outer;
            }

            let sample = &corpus.samples[index];
            let frames = sample.frames();
            let prior = &priors[index];

            let mel_flat: Vec<f32> = sample
                .mel
                .iter()
                .flat_map(|frame| frame.iter().copied())
                .collect();
            let mel = Tensor::from_vec(mel_flat, (1, frames, sample.mel[0].len()), device)?
                .transpose(1, 2)?
                .contiguous()?;

            let noise = Tensor::from_vec(
                rng.normal_vec(frames * sample.mel[0].len()),
                (1, frames, sample.mel[0].len()),
                device,
            )?
            .transpose(1, 2)?
            .contiguous()?;

            let time_value = rng.uniform();
            let (noisy, target) = cfm_noise(&mel, &noise, time_value, 1e-4)?;
            let time = base_time_embed(time_value, device)?;
            let predicted = decoder.forward(&noisy, prior, &time)?;
            let loss = flow_matching_loss(&predicted, &target)?;

            let value: f32 = loss.to_scalar()?;
            if initial_loss.is_nan() {
                initial_loss = value;
            }
            final_loss = value;

            optimizer.backward_step(&loss)?;
            step += 1;

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
                save_checkpoint(&decoder_vm, path)?;
                tracing::info!(
                    "[训练] 第 {} 轮结束，已保存检查点: {}",
                    epoch + 1,
                    path.display()
                );
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

/// 保存解码器检查点。
pub fn save_checkpoint(var_map: &VarMap, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建检查点目录失败: {}", parent.display()))?;
    }
    var_map
        .save(path)
        .with_context(|| format!("保存检查点失败: {}", path.display()))
}

/// 读取解码器检查点。
pub fn load_checkpoint(var_map: &mut VarMap, path: &Path) -> Result<()> {
    var_map
        .load(path)
        .with_context(|| format!("读取检查点失败: {}", path.display()))
}

/// 训练用的轻量随机数，避免额外依赖。
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 40) as f64 / (1u64 << 24) as f64
    }

    fn normal(&mut self) -> f32 {
        let u1 = self.uniform().max(1e-7) as f32;
        let u2 = self.uniform() as f32;
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }

    fn normal_vec(&mut self, len: usize) -> Vec<f32> {
        (0..len).map(|_| self.normal()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::tts::finetune::corpus::{Corpus, CorpusStats, TrainingSample};
    use std::path::Path;

    #[test]
    fn time_embed_len_and_axis() {
        let device = Device::Cpu;
        let emb = base_time_embed(0.0, &device).unwrap();
        assert_eq!(emb.dims(), &[1, 160]);
        // t=0 时 sin(0)=0、cos(0)=1，前半全 0、后半全 1。
        let v: Vec<f32> = emb.flatten_all().unwrap().to_vec1().unwrap();
        assert!(v.iter().take(80).all(|x| x.abs() < 1e-6));
        assert!(v.iter().skip(80).all(|x| (x - 1.0).abs() < 1e-6));
    }

    #[test]
    fn expand_prior_aligns_frames() {
        let device = Device::Cpu;
        // 3 token 的 mel 先验，展开到 10 帧。
        let mel_prior =
            Tensor::from_vec((0..3 * 4).map(|i| i as f32).collect(), (1, 4, 3), &device).unwrap();
        let durations = Tensor::from_vec(vec![1.0f32, 2.0, 3.0], (1, 3), &device).unwrap();
        let expanded = expand_prior(&mel_prior, &durations, 10, &device).unwrap();
        assert_eq!(expanded.dims(), &[1, 4, 10]);
    }

    #[test]
    fn cfm_noise_formula_thresholds() {
        let device = Device::Cpu;
        let mel = Tensor::ones((1, 2, 4), DType::F32, &device).unwrap();
        let noise = Tensor::zeros((1, 2, 4), DType::F32, &device).unwrap();
        // t=1 时 y = x1，t=0 时 y = (1-sigma)*z。
        let (y1, _) = cfm_noise(&mel, &noise, 1.0, 1e-4).unwrap();
        let v: Vec<f32> = y1.flatten_all().unwrap().to_vec1().unwrap();
        assert!(v.iter().all(|x| (x - 1.0).abs() < 1e-6));

        let (y0, _) = cfm_noise(&mel, &noise, 0.0, 1e-4).unwrap();
        let v: Vec<f32> = y0.flatten_all().unwrap().to_vec1().unwrap();
        assert!(v.iter().all(|x| x.abs() < 1e-6));
    }

    fn base_onnx() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx")
    }

    fn tiny_corpus(frames: usize) -> Corpus {
        let mel: Vec<Vec<f32>> = (0..frames)
            .map(|i| {
                (0..80)
                    .map(|j| 0.5 * ((i as f32 * 0.05) + j as f32).sin())
                    .collect()
            })
            .collect();
        let sample = TrainingSample {
            id: "tiny".to_string(),
            token_ids: vec![5, 10, 20, 33],
            mel,
        };
        Corpus {
            samples: vec![sample],
            skipped: vec![],
            stats: CorpusStats::default(),
        }
    }

    /// 集成验证：真实基座权重加载 + 只训解码器，损失应当有界且前缀匹配。
    #[test]
    fn train_base_loads_base_weights_and_runs() {
        let onnx = base_onnx();
        let map = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/tts-training/decoder_weight_map.json");
        if !onnx.exists() || !map.exists() {
            eprintln!("基座权重缺失，跳过集成测试");
            return;
        }

        let corpus = tiny_corpus(120);
        let config = BaseTrainingConfig {
            epochs: 3,
            learning_rate: 1e-3,
            seed: 7,
            checkpoint_every: 0,
        };
        let outcome = train_base(
            &corpus,
            &BaseEncoderConfig::default(),
            &BaseDecoderConfig::default(),
            &config,
            &onnx,
            &map,
            None,
            None,
            &Device::Cpu,
            |_| {},
        )
        .expect("基座对齐训练失败");
        assert!(outcome.steps > 0, "训练步数应为正");
        assert!(outcome.initial_loss.is_finite(), "初损应为有限值");
        assert!(outcome.final_loss.is_finite(), "末损应为有限值");
    }

    /// 验证手动转置卷积层确实通过自动微分收到非零梯度：该层参数在一次反向后被更新。
    #[test]
    fn decoder_transpose_weight_updates_via_backward() -> Result<()> {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let decoder = BaseDecoder::new(&BaseDecoderConfig::default(), vb).unwrap();

        let weight_name = "model.decoder.estimator.up_blocks.0.2.conv.weight";
        let read_w = || {
            var_map
                .data()
                .lock()
                .unwrap()
                .get(weight_name)
                .expect("应由 varmap 登记")
                .as_tensor()
                .flatten_all()
                .unwrap()
                .to_vec1::<f32>()
                .unwrap()
        };
        let before = read_w();

        let frames = 16;
        let noisy = Tensor::randn(0f32, 1.0, (1, 80, frames), &device).unwrap();
        let text = Tensor::randn(0f32, 1.0, (1, 80, frames), &device).unwrap();
        let time = base_time_embed(0.5, &device).unwrap();
        let vel = decoder.forward(&noisy, &text, &time).unwrap();

        // 以「速度接近零」为伪目标构造有界损失，驱动一次反向更新。
        let loss = vel.sqr()?.mean(2)?.mean(1)?.mean(0)?;
        let params = ParamsAdamW {
            lr: 1e-2,
            ..Default::default()
        };
        AdamW::new(var_map.all_vars(), params)
            .unwrap()
            .backward_step(&loss)
            .unwrap();

        let after = read_w();
        assert_ne!(
            before, after,
            "转置卷积权重未随反向更新，自动微分未穿透该层"
        );
        Ok(())
    }
}
