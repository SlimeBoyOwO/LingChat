//! 基于基座模型的对齐训练（burn/Vulkan 后端）。
//!
//! 训练沿用基座的 CFM 配方：编码器只加载基座权重、不参与梯度（时长与 mel 先验同样
//! 固定用基座预测），只有解码器参数被优化。这与「五分钟数据微调音色」的目标一致。
//!
//! 张量计算全部落在 burn 的 autodiff 设备上，物理后端固定为 Vulkan（不启用 CPU
//! 回退，见 [`burn_backend`](super::burn_backend)）。检查点是 burn 原生 `.bpk` 格式。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use burn::module::Module;
use burn::optim::{AdamWConfig, GradientsParams};
use burn::prelude::*;
use burn::tensor::Distribution;
use serde::{Deserialize, Serialize};
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
    /// 单个训练步允许的最大 mel 帧数，0 表示不限制。
    ///
    /// 解码器的时间轴注意力会在每个注意力块上建 [1, 头数, F, F] 矩阵，显存
    /// 随帧数平方增长。移动端 GPU 显存有限，十几秒的长音频单步就能到几百 MB
    /// 峰值而直接崩掉。这里把长样本沿时间轴切成若干不超过该帧数的片段分别
    /// 训练，既不对素材做时长裁剪，又让单步峰值封顶。
    pub max_frames_per_step: usize,
}

impl Default for BaseTrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 20,
            learning_rate: 1e-3,
            seed: 1,
            checkpoint_every: 1,
            cleanup_every: 25,
            max_frames_per_step: 192,
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
    /// 本轮验证集平均损失；训练步中间为 None，每轮末上报一次。
    pub val_loss: Option<f32>,
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
    /// 从第几轮开始续训；0 表示从头训练（本次没有续训）。
    pub resumed_from: usize,
}

/// 训练检查点的配套元数据，用来在中断后从已训轮次续训。
///
/// 只记解码器权重没法知道「训到第几轮了」，这里用 sidecar 文件补记。指纹用于
/// 在续训前校验训练配方是否与上次一致，避免用了不同的数据集或超参去续训上次的
/// 结果，那相当于让两套配方混着训。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TrainState {
    /// 已完成的轮数。
    pub epochs_done: usize,
    /// 已完成的步数。
    pub steps_done: usize,
    /// 续训配方指纹，见 [`train_fingerprint`]。
    pub fingerprint: String,
}

/// 由检查点路径派生出配套的续训状态文件路径。
///
/// 例如 `…/checkpoint.bpk` → `…/train-state.json`，与权重检查点放在同目录。
fn train_state_path(checkpoint_path: &Path) -> PathBuf {
    checkpoint_path.with_file_name("train-state.json")
}

/// 验证集占原始样本量的比例（固定 20%）。
const VAL_SPLIT_RATIO: f64 = 0.2;

/// 从样本总数里按固定种子随机抽取验证集下标（其余为训练集）。
///
/// 用 xorshift 打乱全部下标后取前若干条为验证，保证划分可复现、且与训练
/// 循环的按轮打乱解耦（训练每轮还会再打乱训练集内的顺序）。校验集合用
/// `HashSet` 以便循环内 O(1) 判断某条样本是否属于验证集。
fn split_val_indices(sample_count: usize, seed: u64) -> std::collections::HashSet<usize> {
    let mut indices: Vec<usize> = (0..sample_count).collect();
    let mut state = seed | 0x9E37_79B9;
    for i in (1..sample_count).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % (i as u64 + 1)) as usize;
        indices.swap(i, j);
    }
    let val_count = ((sample_count as f64) * VAL_SPLIT_RATIO).round() as usize;
    indices.into_iter().take(val_count).collect()
}

/// 保存续训状态。写临时文件再改名，避免中途崩溃留下半截 JSON。
pub fn save_train_state(state: &TrainState, checkpoint_path: &Path) -> Result<()> {
    let path = train_state_path(checkpoint_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建状态目录失败: {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(state)?)
        .with_context(|| format!("写入状态临时文件失败: {}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("写入状态文件失败: {}", path.display()))
}

/// 读取续训状态；不存在或读失败时返回 None（视为从零开始）。
pub fn load_train_state(checkpoint_path: &Path) -> Option<TrainState> {
    let path = train_state_path(checkpoint_path);
    let data = std::fs::read(&path).ok()?;
    serde_json::from_slice(&data).ok()
}

/// 计算「训练配方」指纹：一份检查点只对同一套数据集、基座与超参有效。
///
/// 把影响训练语义的要素取个稳定组合：样本 id 列表、基座 ONNX 文件信息、学习率、
/// 单步帧上限、种子。注意**不包含轮数**——否则想「从已训 5 轮继续到 10 轮」时会
/// 因目标轮数不同而被拒绝续训。任一非轮数字段变化，指纹就变，续训时据此拒绝
/// 不匹配的检查点。
fn train_fingerprint(corpus: &Corpus, base_model: &Path, config: &BaseTrainingConfig) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    for sample in &corpus.samples {
        sample.id.hash(&mut hasher);
    }
    let meta = std::fs::metadata(base_model).ok();
    let base_size = meta.map(|m| m.len()).unwrap_or(0);
    (config.learning_rate as u64).hash(&mut hasher);
    config.max_frames_per_step.hash(&mut hasher);
    config.seed.hash(&mut hasher);
    base_size.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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

/// 按单步帧数预算，算一条样本会被切成几个训练片段。
fn chunk_count(frames: usize, max_frames_per_step: usize) -> usize {
    if max_frames_per_step == 0 || frames <= max_frames_per_step {
        1
    } else {
        frames.div_ceil(max_frames_per_step)
    }
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

    // 续训：如果该数据集已训过一部分且配方一致，就接着上次的权重继续，而不是
    // 从基座重推。中途崩溃或主动取消后重新开始训练，已完成的轮次就保留下来。
    let mut resume_epochs = 0usize;
    let mut resume_steps = 0usize;
    if let Some(checkpoint) = checkpoint_path {
        let expected = train_fingerprint(corpus, base_model, config);
        if let Some(state) = load_train_state(checkpoint) {
            if state.fingerprint != expected {
                tracing::warn!(
                    "[训练] 检查点与当前数据集/参数不匹配，忽略它并从基座从头训练"
                );
            } else if state.epochs_done >= config.epochs {
                // 上次已跑满本次要求的轮数，说明用户是要调参或换数据集重训，从头开始。
                tracing::info!(
                    "[训练] 检查点已完成 {} 轮，不小于目标 {} 轮，从头训练",
                    state.epochs_done,
                    config.epochs
                );
            } else if let Ok(loaded) = load_checkpoint(&decoder, checkpoint) {
                decoder = loaded;
                resume_epochs = state.epochs_done;
                resume_steps = state.steps_done;
                tracing::info!(
                    "[训练] 找到检查点，从第 {resume_epochs} 轮（已完成 {resume_steps} 步）继续训练"
                );
            } else {
                tracing::warn!("[训练] 检查点存在但读取失败，改为从基座从头训练");
            }
        }
    }

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

    // 训练过程中 mel 与展开先验的内容不变（编码器冻结、二者都是不可微输入），
    // 且不参与反传。一次性搬上设备并按样本缓存，让每个 epoch 的训练与验证循环
    // 直接引用计数复用，避免每轮反复 from_data 搬移这类纯重复开销。显存占用随
    // 样本量线性（每样本存两片 80 通道张量），对分钟级微调语料规模（数十句）
    // 远小于单步峰值，可接受。
    let mel_priors: Vec<(Tensor<3>, Tensor<3>)> = corpus
        .samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let mel_ch = sample.mel[0].len();
            let frames = sample.frames();
            let mel_flat: Vec<f32> = sample
                .mel
                .iter()
                .flat_map(|frame| frame.iter().copied())
                .collect();
            let mel_full = Tensor::<3>::from_data(
                TensorData::new(mel_flat, [1, frames, mel_ch]),
                &train_dev,
            )
            .transpose();
            let prior_full = Tensor::<3>::from_data(
                TensorData::new(priors[index].clone(), [1, mel_ch, frames]),
                &train_dev,
            );
            (mel_full, prior_full)
        })
        .collect();

    // 每个样本按单步帧数预算切成若干片段，总步数按片段数累计，进度条才准。
    // 验证集不参与梯度更新，步骤数只按训练集累计。
    let val_indices = split_val_indices(corpus.samples.len(), config.seed);
    let chunk_counts: Vec<usize> = corpus
        .samples
        .iter()
        .enumerate()
        .filter(|(i, _)| !val_indices.contains(i))
        .map(|(_, sample)| chunk_count(sample.frames(), config.max_frames_per_step))
        .collect();
    let steps_per_epoch: usize = chunk_counts.iter().sum();
    let total_steps = config.epochs * steps_per_epoch;
    tracing::info!(
        "[训练] 开始：样本 {} 条，{} 轮共 {total_steps} 步，lr={}，单步上限 {} 帧",
        corpus.samples.len(),
        config.epochs,
        config.learning_rate,
        config.max_frames_per_step
    );
    let mut optimizer = AdamWConfig::new()
        .with_weight_decay(0.0)
        .with_grad_clipping(Some(burn::grad_clipping::GradientClippingConfig::Norm(1.0)))
        .init();

    // 学习率调度：前 warmup_frac 步线性升到峰值 lr，之后沿余弦衰减到 lr*min_frac。
    // 固定高 lr 会让 loss 在高位平台振荡且不收敛，衰减才能在末期精修到更低。
    let peak_lr = config.learning_rate;
    let min_frac = 0.02f64;
    let warmup_frac = 0.05f64;
    let warmup_steps = ((total_steps as f64 * warmup_frac).round() as usize).max(1);
    let anneal_steps = total_steps.saturating_sub(warmup_steps).max(1);
    let scheduler = burn::lr_scheduler::sequential::SequentialLrSchedulerConfig::new(
        vec![
            burn::lr_scheduler::LrSchedulerConfig::Linear(
                burn::lr_scheduler::linear::LinearLrSchedulerConfig::new(
                    peak_lr * min_frac,
                    peak_lr,
                    warmup_steps,
                ),
            ),
            burn::lr_scheduler::LrSchedulerConfig::Cosine(
                burn::lr_scheduler::cosine::CosineAnnealingLrSchedulerConfig::new(peak_lr, anneal_steps)
                    .with_min_lr(peak_lr * min_frac),
            ),
        ],
        vec![warmup_steps],
    )
    .init()
    .map_err(anyhow::Error::msg)?;
    let mut lr_scheduler = scheduler;
    let mut initial_loss = f32::NAN;
    let mut final_loss = f32::NAN;
    let mut step = resume_steps;
    let mut cancelled = false;
    // 每轮耗时统计：按步累加，轮末打印该轮平均每步时间与步/秒，用来校准性能。
    let mut epoch_start = std::time::Instant::now();
    let mut epoch_step_count = 0usize;

    'outer: for epoch in resume_epochs..config.epochs {
        let order = make_batches(corpus.samples.len(), usize::MAX, config.seed ^ epoch as u64);
        let order: Vec<usize> = order.into_iter().flatten().collect();

        for index in order {
            if val_indices.contains(&index) {
                continue;
            }
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
            let (mel_full, prior_full) = &mel_priors[index];

            // 沿时间轴切片：片段长度不超过预算，避免长音频的单步显存峰值爆掉。
            let chunk = if config.max_frames_per_step == 0 {
                frames
            } else {
                config.max_frames_per_step.min(frames).max(1)
            };
            let mut start = 0usize;
            while start < frames {
                if cancel.is_some_and(|token| token.is_cancelled()) {
                    cancelled = true;
                    tracing::info!(
                        "[训练] 收到取消，提前结束（第 {} 轮，已完成 {step} 步）",
                        epoch + 1
                    );
                    break 'outer;
                }

                let len = chunk.min(frames - start);
                // 整条样本不需要切分时就别走 narrow：narrow 会在设备上再拷一份，
                // 而 clone 只是引用计数，短句样本因此少一次无谓的全量拷贝。
                let (mel, prior) = if len == frames {
                    (mel_full.clone(), prior_full.clone())
                } else {
                    (
                        mel_full.clone().narrow(2, start, len),
                        prior_full.clone().narrow(2, start, len),
                    )
                };

                // 噪声只按当前片段长度生成，长样本不必先铺满整条再切。
                let noise = Tensor::<3>::random(
                    [1, len, mel_ch],
                    Distribution::Normal(0.0, 1.0),
                    &train_dev,
                )
                .transpose();

                let time_value = rand_uniform(config.seed ^ step as u64 ^ 0x9e37);
                let (noisy, target) = cfm_noise(&mel, &noise, time_value, 1e-4);
                let time = base_time_embed(time_value, &train_dev);

                let predicted = decoder.forward(noisy, prior, time);
                let loss = (predicted - target).powf_scalar(2.0).mean();

                let grads = GradientsParams::from_grads(loss.backward(), &decoder);
                let step_lr = lr_scheduler.step();
                decoder = optimizer.step(step_lr, decoder, grads);
                step += 1;
                epoch_step_count += 1;

                // 周期归还设备内存：把池里已无引用的页交还驱动，避免跨步累积导致的
                // 移动端 OOM（Adreno 等设备对长期驻留的设备内存尤其敏感）。
                if config.cleanup_every > 0 && step % config.cleanup_every == 0 {
                    train_dev.memory_cleanup();
                }

                // 只有真要上报进度时才把 loss 读回主机。每步都读等于每步强制一次
                // 设备同步，会让 GPU 流水线停顿，是训练循环里最伤速度的写法。
                if step == 1 || step % 5 == 0 || step == total_steps {
                    let value: f32 = loss.to_data().try_to_vec::<f32>()?[0];
                    if initial_loss.is_nan() {
                        initial_loss = value;
                    }
                    final_loss = value;

                    on_progress(&BaseTrainingProgress {
                        epoch: epoch + 1,
                        total_epochs: config.epochs,
                        step,
                        total_steps,
                        loss: value,
                        val_loss: None,
                    });
                }

                start += len;
            }
        }

        // 每轮末在验证集上算一次平均 CFM 损失（不反传，用于观察过拟合与泛化）。
        // 与训练同口径：每个样本随机采样时间 t，MSE 取整条（长样本逐 chunk）均摊。
        if !val_indices.is_empty() {
            let mut val_sum = 0.0f64;
            let mut val_n = 0usize;
            for vindex in &val_indices {
                if cancel.is_some_and(|token| token.is_cancelled()) {
                    break;
                }
                let vsample = &corpus.samples[*vindex];
                let vmel_ch = vsample.mel[0].len();
                let vframes = vsample.frames();
                let (vmel_full, vprior_full) = &mel_priors[*vindex];
                let vchunk = if config.max_frames_per_step == 0 {
                    vframes
                } else {
                    config.max_frames_per_step.min(vframes).max(1)
                };
                let mut vstart = 0usize;
                while vstart < vframes {
                    let vlen = vchunk.min(vframes - vstart);
                    let (vmel, vprior) = if vlen == vframes {
                        (vmel_full.clone(), vprior_full.clone())
                    } else {
                        (
                            vmel_full.clone().narrow(2, vstart, vlen),
                            vprior_full.clone().narrow(2, vstart, vlen),
                        )
                    };
                    let vnoise = Tensor::<3>::random(
                        [1, vlen, vmel_ch],
                        Distribution::Normal(0.0, 1.0),
                        &train_dev,
                    )
                    .transpose();
                    // 与训练同一随机源但用独立种子偏移，避免与训练步的时间采样重合。
                    let vtime = rand_uniform(config.seed ^ step as u64 ^ 0xC0DE);
                    let (vnoisy, vtarget) = cfm_noise(&vmel, &vnoise, vtime, 1e-4);
                    let vtime_t = base_time_embed(vtime, &train_dev);
                    let vpred = decoder.forward(vnoisy, vprior, vtime_t);
                    let vloss = (vpred - vtarget).powf_scalar(2.0).mean();
                    val_sum += vloss.to_data().try_to_vec::<f32>()?[0] as f64;
                    val_n += 1;
                    vstart += vlen;
                }
            }
            let val_loss = if val_n > 0 { val_sum / val_n as f64 } else { f32::NAN as f64 };
            on_progress(&BaseTrainingProgress {
                epoch: epoch + 1,
                total_epochs: config.epochs,
                step,
                total_steps,
                loss: final_loss,
                val_loss: Some(val_loss as f32),
            });
            tracing::info!("[训练] 第 {} 轮验证集 loss={val_loss:.6}", epoch + 1);
        }

        if config.checkpoint_every > 0 && (epoch + 1) % config.checkpoint_every == 0 {
            if let Some(path) = checkpoint_path {
                save_checkpoint(&decoder, path)?;
                save_train_state(
                    &TrainState {
                        epochs_done: epoch + 1,
                        steps_done: step,
                        fingerprint: train_fingerprint(corpus, base_model, config),
                    },
                    path,
                )?;
                tracing::info!("[训练] 第 {} 轮结束，已保存检查点: {}", epoch + 1, path.display());
            }
        }

        // 打印本轮每步平均耗时，供校准「步/秒」性能目标。
        let epoch_elapsed = epoch_start.elapsed().as_secs_f64();
        epoch_start = std::time::Instant::now();
        if epoch_step_count > 0 {
            let per_step = epoch_elapsed / epoch_step_count as f64;
            let step_ms = per_step * 1000.0;
            let per_second = 1.0 / per_step;
            tracing::info!(
                "[训练] 第 {} 轮耗时 {epoch_elapsed:.1}s，{epoch_step_count} 步，均值 {step_ms:.1}ms/步（{per_second:.2} 步/s）",
                epoch + 1
            );
        }
        epoch_step_count = 0;
    }

    tracing::info!(
        "[训练] 结束：共 {step} 步，初损 {initial_loss:.6}，末损 {final_loss:.6}，取消={cancelled}"
    );
    Ok(BaseTrainingOutcome {
        steps: step,
        initial_loss,
        final_loss,
        cancelled,
        resumed_from: resume_epochs,
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
            max_frames_per_step: 0,
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

    /// 续训集成验证：第一次跑 1 轮存检查点，第二次请求 2 轮，应只补第 2 轮。
    #[test]
    fn train_resumes_from_checkpoint() {
        use crate::ai_service::tts::finetune::corpus::{Corpus, CorpusStats, TrainingSample};
        use std::path::Path;

        let onnx = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx");
        let map = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/tts-training/decoder_weight_map.json");
        if !onnx.exists() || !map.exists() {
            eprintln!("基座权重缺失，跳过续训集成测试");
            return;
        }

        let melt = |frames: usize| {
            (0..frames)
                .map(|i| {
                    (0..80)
                        .map(|j| 0.5 * ((i as f32 * 0.05) + j as f32).sin())
                        .collect()
                })
                .collect::<Vec<Vec<f32>>>()
        };
        let corpus = Corpus {
            samples: vec![
                TrainingSample {
                    id: "a".to_string(),
                    token_ids: vec![5, 10],
                    mel: melt(120),
                },
                TrainingSample {
                    id: "b".to_string(),
                    token_ids: vec![20, 33, 7],
                    mel: melt(150),
                },
            ],
            skipped: vec![],
            stats: CorpusStats::default(),
        };

        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/resume-test");
        std::fs::create_dir_all(&dir).unwrap();
        let checkpoint = dir.join("checkpoint.bpk");

        let run = |epochs: usize, checkpoint: Option<&Path>| {
            let config = BaseTrainingConfig {
                epochs,
                learning_rate: 1e-3,
                seed: 7,
                checkpoint_every: 1,
                cleanup_every: 0,
                max_frames_per_step: 0,
            };
            train_base(
                &corpus,
                &BurnBaseEncoderConfig::default(),
                &BurnBaseDecoderConfig::default(),
                &config,
                &onnx,
                &map,
                checkpoint,
                None,
                |_| {},
            )
            .map_err(|e| format!("续训验证训练失败: {e:#}"))
            .unwrap()
        };

        // 第一次：1 轮，存检查点。两步样本对应每一步一次。
        let first = run(1, Some(&checkpoint));
        // 第二次：请求 2 轮，应从第 1 轮续，总共仍只多跑 1 轮（而非 2 轮）。
        let second = run(2, Some(&checkpoint));

        // 每轮步数 = 样本数（无切片）= 2 步。
        let steps_per_epoch = 2usize;
        assert_eq!(first.steps, steps_per_epoch, "第一次应正好训 1 轮");
        assert_eq!(first.resumed_from, 0, "第一次应从头训练");
        // 续训共 2 轮 → 共 2*steps_per_epoch 步。
        assert_eq!(
            second.steps,
            2 * steps_per_epoch,
            "续训后步数应按 2 轮累计"
        );
        // 第二次应从第 1 轮开始续训，而不是从头重训，这才是续训生效的关键证据。
        assert_eq!(second.resumed_from, 1, "第二次应从上一次结束的轮次续训");
        // 确保状态文件已生成并可读、指纹匹配。
        assert!(
            load_train_state(&checkpoint).is_some(),
            "续训状态文件应存在"
        );
        // 清理测试产物。
        let _ = std::fs::remove_dir_all(&dir);
    }
}