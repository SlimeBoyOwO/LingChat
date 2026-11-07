//! 训练语料组装。
//!
//! 把数据集清单、文本前端和 mel 特征串起来：每条样本产出 token id 序列和
//! log-mel 帧，供训练循环直接消费。这里同时承担长度过滤——过短或没有
//! 文本标注的素材会被跳过并记录原因，不会让训练在坏样本上崩掉。

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::feature::{MelConfig, log_mel};
use super::import::DatasetManifest;
use super::normalize::read_wav_mono;
use super::resample::resample_mono;
use super::text::TextFrontend;

/// 语料组装配置。
#[derive(Debug, Clone)]
pub struct CorpusConfig {
    /// mel 特征配置。
    pub mel: MelConfig,
    /// 最少 token 数。
    pub min_tokens: usize,
    /// 最多 token 数。
    pub max_tokens: usize,
    /// 最少 mel 帧数。
    pub min_frames: usize,
}

impl Default for CorpusConfig {
    fn default() -> Self {
        Self {
            mel: MelConfig::default(),
            min_tokens: 1,
            max_tokens: 400,
            min_frames: 8,
        }
    }
}

/// 一条训练样本。
#[derive(Debug, Clone)]
pub struct TrainingSample {
    /// 样本 id。
    pub id: String,
    /// token id 序列。
    pub token_ids: Vec<i32>,
    /// log-mel 特征，帧数乘 mel 通道数。
    pub mel: Vec<Vec<f32>>,
}

impl TrainingSample {
    /// mel 帧数。
    pub fn frames(&self) -> usize {
        self.mel.len()
    }

    /// token 数。
    pub fn tokens(&self) -> usize {
        self.token_ids.len()
    }

    /// 时长（秒）。
    pub fn duration_secs(&self, hop_length: usize, sample_rate: u32) -> f64 {
        if sample_rate == 0 {
            return 0.0;
        }
        self.frames() as f64 * hop_length as f64 / sample_rate as f64
    }
}

/// 被跳过的样本。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedSample {
    /// 样本 id。
    pub id: String,
    /// 跳过原因。
    pub reason: String,
}

/// 语料统计。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorpusStats {
    /// 有效样本数。
    pub total_samples: usize,
    /// 总帧数。
    pub total_frames: usize,
    /// 总 token 数。
    pub total_tokens: usize,
    /// 总时长（秒）。
    pub total_secs: f64,
}

/// 组装好的语料。
#[derive(Debug, Clone)]
pub struct Corpus {
    /// 有效样本。
    pub samples: Vec<TrainingSample>,
    /// 跳过列表。
    pub skipped: Vec<SkippedSample>,
    /// 统计。
    pub stats: CorpusStats,
}

/// 从数据集目录组装语料。
pub fn build_corpus(root: &Path, frontend: &TextFrontend, config: &CorpusConfig) -> Result<Corpus> {
    let manifest_path = root.join("dataset.json");
    let bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("读取数据集清单失败: {}", manifest_path.display()))?;
    let manifest: DatasetManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("解析数据集清单失败: {}", manifest_path.display()))?;
    build_from_manifest(root, &manifest, frontend, config)
}

/// 从已解析的清单组装语料。
pub fn build_from_manifest(
    root: &Path,
    manifest: &DatasetManifest,
    frontend: &TextFrontend,
    config: &CorpusConfig,
) -> Result<Corpus> {
    let hop = config.mel.hop_length.max(1);
    let sample_rate = config.mel.sample_rate;

    let mut samples = Vec::new();
    let mut skipped = Vec::new();

    for item in &manifest.items {
        let Some(text) = item.text.as_deref() else {
            skipped.push(SkippedSample {
                id: item.id.clone(),
                reason: "缺少文本标注".to_string(),
            });
            continue;
        };

        let token_ids = frontend.text_to_ids(text);
        if token_ids.len() < config.min_tokens {
            skipped.push(SkippedSample {
                id: item.id.clone(),
                reason: format!("文本切不出有效 token: {text}"),
            });
            continue;
        }
        if token_ids.len() > config.max_tokens {
            skipped.push(SkippedSample {
                id: item.id.clone(),
                reason: format!(
                    "文本过长（{} token，上限 {}）",
                    token_ids.len(),
                    config.max_tokens
                ),
            });
            continue;
        }

        let wav_path = root.join(&item.wav);
        let (waveform, rate) = match read_wav_mono(&wav_path) {
            Ok(value) => value,
            Err(error) => {
                skipped.push(SkippedSample {
                    id: item.id.clone(),
                    reason: format!("读取 WAV 失败: {error:#}"),
                });
                continue;
            },
        };

        let waveform = if rate == sample_rate {
            waveform
        } else {
            resample_mono(&waveform, rate, sample_rate)
        };

        let mel = log_mel(&waveform, &config.mel);
        if mel.len() < config.min_frames {
            skipped.push(SkippedSample {
                id: item.id.clone(),
                reason: format!("音频过短（{} 帧，下限 {}）", mel.len(), config.min_frames),
            });
            continue;
        }

        samples.push(TrainingSample {
            id: item.id.clone(),
            token_ids,
            mel,
        });
    }

    if samples.is_empty() {
        anyhow::bail!("没有可用于训练的样本，请检查音频时长与文本标注");
    }

    let total_frames = samples.iter().map(TrainingSample::frames).sum();
    let total_tokens = samples.iter().map(TrainingSample::tokens).sum();
    let total_secs = total_frames as f64 * hop as f64 / f64::from(sample_rate);

    Ok(Corpus {
        stats: CorpusStats {
            total_samples: samples.len(),
            total_frames,
            total_tokens,
            total_secs,
        },
        samples,
        skipped,
    })
}

/// 把样本下标按批次切分。
///
/// 同一个 seed 得到同一个顺序，便于断点续训。返回的是下标而不是样本本身，
/// 调用方按需取用，避免整批复制 mel。
pub fn make_batches(sample_count: usize, batch_size: usize, seed: u64) -> Vec<Vec<usize>> {
    if sample_count == 0 || batch_size == 0 {
        return Vec::new();
    }

    let mut indices: Vec<usize> = (0..sample_count).collect();
    let mut state = seed | 1;
    for i in (1..sample_count).rev() {
        // xorshift64，够用的确定性伪随机。
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % (i as u64 + 1)) as usize;
        indices.swap(i, j);
    }

    indices.chunks(batch_size).map(<[usize]>::to_vec).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_wav(path: &Path, seconds: f32, sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        let total = (seconds * sample_rate as f32) as usize;
        for i in 0..total {
            let value =
                ((2.0 * std::f32::consts::PI * 220.0 * i as f32 / sample_rate as f32).sin()
                    * 8_000.0) as i16;
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn setup(dir: &Path, items: &[(&str, Option<&str>)]) -> PathBuf {
        std::fs::create_dir_all(dir.join("wavs")).unwrap();
        std::fs::write(dir.join("tokens.txt"), "a 0\nb 1\nc 2\n").unwrap();

        let mut manifest_items = Vec::new();
        for (id, text) in items {
            write_wav(&dir.join(format!("wavs/{id}.wav")), 1.0, 22_050);
            manifest_items.push(serde_json::json!({
                "id": id,
                "wav": format!("wavs/{id}.wav"),
                "text": text,
                "duration_secs": 1.0,
            }));
        }

        let manifest = serde_json::json!({
            "version": 1,
            "dataset_id": "test",
            "sample_rate": 22_050,
            "created_at_unix": 0,
            "items": manifest_items,
            "skipped": [],
            "stats": {
                "total_items": items.len(),
                "items_with_text": items.len(),
                "total_secs": items.len() as f64,
                "min_required_secs": 300.0,
                "meets_minimum": false
            }
        });
        std::fs::write(
            dir.join("dataset.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        dir.to_path_buf()
    }

    fn frontend(dir: &Path) -> TextFrontend {
        TextFrontend::load(&dir.join("tokens.txt"), None).unwrap()
    }

    #[test]
    fn builds_samples_with_expected_shapes() {
        let temp = tempfile::tempdir().unwrap();
        let root = setup(temp.path(), &[("a", Some("ab"))]);
        let corpus = build_corpus(&root, &frontend(temp.path()), &CorpusConfig::default()).unwrap();

        assert_eq!(corpus.samples.len(), 1);
        let sample = &corpus.samples[0];
        assert_eq!(sample.token_ids, vec![0, 1]);
        assert!(sample.frames() > 8);
        assert!(sample.mel.iter().all(|frame| frame.len() == 80));
        assert_eq!(corpus.stats.total_samples, 1);
        assert!(corpus.stats.total_secs > 0.5);
    }

    #[test]
    fn skips_items_without_text() {
        let temp = tempfile::tempdir().unwrap();
        let root = setup(temp.path(), &[("a", Some("ab")), ("b", None)]);
        let corpus = build_corpus(&root, &frontend(temp.path()), &CorpusConfig::default()).unwrap();

        assert_eq!(corpus.samples.len(), 1);
        assert_eq!(corpus.skipped.len(), 1);
        assert!(corpus.skipped[0].reason.contains("文本"));
    }

    #[test]
    fn skips_when_token_limit_exceeded() {
        let temp = tempfile::tempdir().unwrap();
        let root = setup(temp.path(), &[("a", Some("abc"))]);
        let mut config = CorpusConfig::default();
        config.max_tokens = 2;
        let corpus = build_corpus(&root, &frontend(temp.path()), &config).unwrap_err();
        assert!(corpus.to_string().contains("没有可用于训练的样本"));
    }

    #[test]
    fn batches_cover_every_sample_once() {
        let batches = make_batches(7, 3, 42);
        let mut all: Vec<usize> = batches.iter().flatten().copied().collect();
        all.sort_unstable();
        assert_eq!(all, (0..7).collect::<Vec<_>>());
        assert!(batches.iter().all(|batch| !batch.is_empty() && batch.len() <= 3));
    }

    #[test]
    fn batching_is_deterministic() {
        assert_eq!(make_batches(10, 4, 7), make_batches(10, 4, 7));
        assert_ne!(make_batches(10, 4, 7), make_batches(10, 4, 8));
    }

    #[test]
    fn empty_batches_for_zero_input() {
        assert!(make_batches(0, 4, 1).is_empty());
        assert!(make_batches(5, 0, 1).is_empty());
    }
}
