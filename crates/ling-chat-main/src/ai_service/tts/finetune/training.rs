//! 训练循环。
//!
//! 微调沿用基座的时长预测结果做对齐：先用当前模型对每条文本预测一次 token 时长，
//! 再按帧数缩放，得到可用的逐 token 帧数。这样不需要额外的人工对齐数据，也不会
//! 让时长在训练中漂移。
//!
//! 单条样本一步，移动端也吃得消。检查点用 safetensors，中断后从最近一轮继续。

use std::path::Path;

use anyhow::{Context, Result};
use candle_core::{Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::corpus::{Corpus, make_batches};
use super::decoder::{CfmDecoder, add_noise, flow_matching_loss};
use super::model::Matcha;

/// 训练配置。
#[derive(Debug, Clone)]
pub struct TrainingConfig {
    /// 训练轮数。
    pub epochs: usize,
    /// 学习率。
    pub learning_rate: f64,
    /// 打乱顺序与噪声的随机种子。
    pub seed: u64,
    /// 每多少轮存一次检查点。
    pub checkpoint_every: usize,
    /// 每个 token 最少帧数。
    pub min_frames_per_token: usize,
    /// 每个 token 最多帧数。
    pub max_frames_per_token: usize,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 20,
            learning_rate: 1e-3,
            seed: 1,
            checkpoint_every: 1,
            min_frames_per_token: 1,
            max_frames_per_token: 50,
        }
    }
}

/// 训练进度。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TrainingProgress {
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
    /// 本轮验证集平均损失；每个 epoch 末上报一次，训练步中间为 None。
    pub val_loss: Option<f32>,
}

/// 训练结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TrainingOutcome {
    /// 实际执行的步数。
    pub steps: usize,
    /// 首步损失。
    pub initial_loss: f32,
    /// 末步损失。
    pub final_loss: f32,
    /// 是否被取消。
    pub cancelled: bool,
}

/// 执行训练。
#[allow(clippy::too_many_arguments)]
pub fn train(
    corpus: &Corpus,
    model: &Matcha,
    decoder: &CfmDecoder,
    var_map: &VarMap,
    config: &TrainingConfig,
    device: &Device,
    checkpoint_path: Option<&Path>,
    cancel: Option<&CancellationToken>,
    mut on_progress: impl FnMut(&TrainingProgress),
) -> Result<TrainingOutcome> {
    if corpus.samples.is_empty() {
        anyhow::bail!("语料为空，无法训练");
    }

    // 先用未训练的模型预测时长，作为整轮训练的固定对齐。
    let alignments = corpus
        .samples
        .iter()
        .map(|sample| {
            predict_frames(model, &sample.token_ids, sample.frames(), config, device)
                .with_context(|| format!("样本 {} 对齐失败", sample.id))
        })
        .collect::<Result<Vec<_>>>()?;

    let total_steps = config.epochs * corpus.samples.len();
    let params = ParamsAdamW {
        lr: config.learning_rate,
        ..Default::default()
    };
    let mut optimizer = AdamW::new(var_map.all_vars(), params)?;
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
                break 'outer;
            }

            let sample = &corpus.samples[index];
            let frames = sample.frames();
            let alignment = &alignments[index];

            let tokens = Tensor::from_vec(
                sample.token_ids.clone(),
                (1, sample.token_ids.len()),
                device,
            )?;
            let (hidden, _) = model.forward(&tokens)?;
            let text = expand_to_frames(&hidden, alignment)?;

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

            let time_value = 0.05 + rng.uniform() * 0.9;
            let time = Tensor::from_vec(vec![time_value], (1,), device)?;

            let (noisy, target) = add_noise(&mel, &noise, time_value as f64)?;
            let predicted = decoder.forward(&noisy, &text, &time)?;
            let loss = flow_matching_loss(&predicted, &target)?;

            let value: f32 = loss.to_scalar()?;
            if initial_loss.is_nan() {
                initial_loss = value;
            }
            final_loss = value;

            optimizer.backward_step(&loss)?;
            step += 1;

            on_progress(&TrainingProgress {
                epoch: epoch + 1,
                total_epochs: config.epochs,
                step,
                total_steps,
                loss: value,
                val_loss: None,
            });
        }

        if config.checkpoint_every > 0 && (epoch + 1) % config.checkpoint_every == 0 {
            if let Some(path) = checkpoint_path {
                save_checkpoint(var_map, path)?;
            }
        }
    }
    Ok(TrainingOutcome {
        steps: step,
        initial_loss,
        final_loss,
        cancelled,
    })
}

/// 保存检查点。
pub fn save_checkpoint(var_map: &VarMap, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建检查点目录失败: {}", parent.display()))?;
    }
    var_map
        .save(path)
        .with_context(|| format!("保存检查点失败: {}", path.display()))
}

/// 读取检查点。
pub fn load_checkpoint(var_map: &mut VarMap, path: &Path) -> Result<()> {
    var_map
        .load(path)
        .with_context(|| format!("读取检查点失败: {}", path.display()))
}

/// 用当前模型预测逐 token 帧数，并缩放到目标帧数。
fn predict_frames(
    model: &Matcha,
    token_ids: &[i32],
    frames: usize,
    config: &TrainingConfig,
    device: &Device,
) -> Result<Vec<usize>> {
    let tokens = Tensor::from_vec(token_ids.to_vec(), (1, token_ids.len()), device)?;
    let (_, log_durations) = model.forward(&tokens)?;
    let raw = log_durations.to_vec2::<f32>()?;
    let row = raw.first().context("时长预测输出为空")?;

    if frames < row.len() {
        anyhow::bail!("帧数 {frames} 少于 token 数 {}", row.len());
    }

    let seconds: Vec<f32> = row.iter().map(|value| value.exp()).collect();
    let sum: f32 = seconds.iter().sum();
    let scale = if sum > 1e-3 { frames as f32 / sum } else { 1.0 };

    let mut durations: Vec<usize> = seconds
        .iter()
        .map(|value| {
            ((value * scale).round() as usize)
                .clamp(config.min_frames_per_token, config.max_frames_per_token)
        })
        .collect();

    // 对齐总量到真实帧数，多退少补。
    let mut total: usize = durations.iter().sum();
    while total > frames {
        let Some(index) = durations
            .iter()
            .enumerate()
            .filter(|(_, value)| **value > config.min_frames_per_token)
            .max_by_key(|(_, value)| **value)
            .map(|(index, _)| index)
        else {
            break;
        };
        durations[index] -= 1;
        total -= 1;
    }
    if total < frames {
        if let Some(last) = durations.last_mut() {
            *last += frames - total;
        }
    }

    Ok(durations)
}

/// 按逐 token 帧数把文本隐状态展开到帧级。
fn expand_to_frames(hidden: &Tensor, durations: &[usize]) -> Result<Tensor> {
    let mut indices = Vec::with_capacity(durations.iter().sum());
    for (index, duration) in durations.iter().enumerate() {
        for _ in 0..*duration {
            indices.push(index as u32);
        }
    }

    let device = hidden.device();
    let index_tensor = Tensor::from_vec(indices, (durations.iter().sum::<usize>(),), device)?;
    let expanded = hidden.squeeze(0)?.index_select(&index_tensor, 0)?;
    Ok(expanded.unsqueeze(0)?.transpose(1, 2)?.contiguous()?)
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

    fn uniform(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn normal(&mut self) -> f32 {
        // Box-Muller，避免除零。
        let u1 = self.uniform().max(1e-7);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }

    fn normal_vec(&mut self, len: usize) -> Vec<f32> {
        (0..len).map(|_| self.normal()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::corpus::CorpusStats;
    use super::super::model::MatchaConfig;
    use super::*;
    use candle_core::DType;
    use candle_nn::VarBuilder;

    fn tiny_corpus() -> Corpus {
        let samples = (0..3)
            .map(|index| super::super::corpus::TrainingSample {
                id: format!("s{index}"),
                token_ids: vec![1, 2, 3, 4],
                mel: (0..40)
                    .map(|frame| {
                        (0..8)
                            .map(|channel| ((frame * 8 + channel) as f32 * 0.1).sin() * 0.5)
                            .collect()
                    })
                    .collect(),
            })
            .collect();
        Corpus {
            samples,
            skipped: Vec::new(),
            stats: CorpusStats::default(),
        }
    }

    #[test]
    fn training_reduces_loss_and_saves_checkpoint() -> Result<()> {
        let device = Device::Cpu;
        let mut var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);

        let model = Matcha::new(
            &MatchaConfig {
                n_vocab: 32,
                n_mels: 8,
                hidden: 16,
                layers: 1,
                heads: 2,
                ffn_mult: 2,
                duration_layers: 1,
            },
            vb.pp("matcha"),
        )?;
        let decoder = CfmDecoder::new(
            &super::super::decoder::CfmConfig {
                n_mels: 8,
                text_hidden: 16,
                base_channels: 8,
                time_dim: 8,
            },
            vb.pp("decoder"),
        )?;

        let dir = tempfile::tempdir().unwrap();
        let checkpoint = dir.path().join("checkpoint.safetensors");

        let config = TrainingConfig {
            epochs: 30,
            learning_rate: 0.01,
            seed: 7,
            checkpoint_every: 10,
            min_frames_per_token: 1,
            max_frames_per_token: 50,
        };

        let mut progress_count = 0;
        let outcome = train(
            &tiny_corpus(),
            &model,
            &decoder,
            &var_map,
            &config,
            &device,
            Some(&checkpoint),
            None,
            |_| progress_count += 1,
        )?;

        assert!(!outcome.cancelled);
        assert_eq!(outcome.steps, 30 * 3);
        assert_eq!(progress_count, outcome.steps);
        assert!(outcome.final_loss < outcome.initial_loss);
        assert!(checkpoint.exists());
        assert!(std::fs::metadata(&checkpoint).unwrap().len() > 0);

        // 参数表能被完整读回。
        load_checkpoint(&mut var_map, &checkpoint)?;
        assert!(!var_map.all_vars().is_empty());
        Ok(())
    }

    #[test]
    fn cancellation_stops_early() -> Result<()> {
        let device = Device::Cpu;
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let model = Matcha::new(
            &MatchaConfig {
                n_vocab: 32,
                n_mels: 8,
                hidden: 16,
                layers: 1,
                heads: 2,
                ffn_mult: 2,
                duration_layers: 1,
            },
            vb.pp("matcha"),
        )?;
        let decoder = CfmDecoder::new(
            &super::super::decoder::CfmConfig {
                n_mels: 8,
                text_hidden: 16,
                base_channels: 8,
                time_dim: 8,
            },
            vb.pp("decoder"),
        )?;

        let token = CancellationToken::new();
        token.cancel();

        let outcome = train(
            &tiny_corpus(),
            &model,
            &decoder,
            &var_map,
            &TrainingConfig::default(),
            &device,
            None,
            Some(&token),
            |_| {},
        )?;

        assert!(outcome.cancelled);
        assert_eq!(outcome.steps, 0);
        Ok(())
    }
}
