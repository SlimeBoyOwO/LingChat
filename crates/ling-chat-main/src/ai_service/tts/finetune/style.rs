//! 无监督风格聚类：用轻量声学特征做确定性 k-means，生成风格伪标签。

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::feature::{MelConfig, log_mel};
use super::import::{DatasetItem, DatasetManifest};
use super::normalize::read_wav_mono;
use super::resample::resample_mono;

/// 默认风格簇数量。
pub const DEFAULT_STYLE_CLUSTERS: usize = 4;
/// 最大簇数量，避免小数据集被切碎。
pub const MAX_STYLE_CLUSTERS: usize = 16;
/// k-means 最大迭代次数。
const KMEANS_ITERATIONS: usize = 30;
/// 固定种子，保证同一数据集聚类结果一致。
const KMEANS_SEED: u64 = 0x5EED_5EED_51A7_1E00;

/// 一个风格簇的摘要。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StyleClusterSummary {
    pub cluster: u32,
    pub items: usize,
    pub total_secs: f64,
}

/// 聚类结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StyleClusterReport {
    pub dataset_id: String,
    pub k: usize,
    /// 各簇摘要，按簇 id 升序。
    pub clusters: Vec<StyleClusterSummary>,
}

/// 给数据集做风格聚类并写回 `dataset.json`；特征不可用的样本保持 None，k 自动收缩。
pub fn cluster_dataset(root: &Path, requested_k: usize) -> Result<StyleClusterReport> {
    let manifest_path = root.join("dataset.json");
    let bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("读取数据集清单失败: {}", manifest_path.display()))?;
    let mut manifest: DatasetManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("解析数据集清单失败: {}", manifest_path.display()))?;

    if manifest.items.is_empty() {
        bail!("数据集为空，无法进行风格聚类");
    }

    let mel_config = MelConfig::default();
    let mut ids = Vec::new();
    let mut features = Vec::new();
    for item in &manifest.items {
        match extract_style_feature(root, item, &mel_config) {
            Ok(feature) => {
                ids.push(item.id.clone());
                features.push(feature);
            },
            Err(error) => {
                tracing::warn!("[风格聚类] 跳过 {}: {error:#}", item.id);
            },
        }
    }

    if features.len() < 2 {
        bail!("可读取的音频不足 2 条，无法聚类");
    }

    let k = requested_k.clamp(1, MAX_STYLE_CLUSTERS).min(features.len());
    let normalized = standardize(&features);
    let assignments = kmeans(&normalized, k, KMEANS_SEED);
    let by_id: HashMap<&str, u32> = ids
        .iter()
        .map(String::as_str)
        .zip(assignments.iter().map(|value| *value as u32))
        .collect();

    for item in &mut manifest.items {
        item.style_cluster = by_id.get(item.id.as_str()).copied();
    }

    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .with_context(|| format!("序列化数据集清单失败: {}", manifest_path.display()))?,
    )
    .with_context(|| format!("写回数据集清单失败: {}", manifest_path.display()))?;

    Ok(StyleClusterReport {
        dataset_id: manifest.dataset_id,
        k,
        clusters: summarize_clusters(&manifest.items),
    })
}

/// 汇总清单里已分配的簇。
pub fn summarize_clusters(items: &[DatasetItem]) -> Vec<StyleClusterSummary> {
    let mut counts: BTreeMap<u32, (usize, f64)> = BTreeMap::new();
    for item in items {
        let Some(cluster) = item.style_cluster else {
            continue;
        };
        let entry = counts.entry(cluster).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += item.duration_secs;
    }
    counts
        .into_iter()
        .map(|(cluster, (items, total_secs))| StyleClusterSummary {
            cluster,
            items,
            total_secs,
        })
        .collect()
}

/// 从一条音频提取风格特征。
fn extract_style_feature(root: &Path, item: &DatasetItem, mel: &MelConfig) -> Result<Vec<f32>> {
    let (samples, rate) = read_wav_mono(&root.join(&item.wav))?;
    let samples = if rate == mel.sample_rate {
        samples
    } else {
        resample_mono(&samples, rate, mel.sample_rate)
    };
    if samples.is_empty() {
        bail!("音频为空");
    }

    let frames = log_mel(&samples, mel);
    if frames.is_empty() || frames[0].is_empty() {
        bail!("log-mel 为空");
    }

    let bands = frames[0].len();
    let mut feature = Vec::with_capacity(bands * 2 + 4);
    for band in 0..bands {
        let values = frames.iter().map(|frame| frame[band] as f64);
        let count = frames.len() as f64;
        let mean = values.clone().sum::<f64>() / count;
        let variance = values
            .map(|value| {
                let delta = value - mean;
                delta * delta
            })
            .sum::<f64>()
            / count;
        feature.push(mean as f32);
        feature.push(variance.sqrt() as f32);
    }

    let rms =
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt();
    let zero_crossings = samples
        .windows(2)
        .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
        .count() as f32
        / samples.len().max(1) as f32;

    // 时长取对数，避免长样本在距离里权重过大；响度和过零率保留原始尺度。
    feature.push(item.duration_secs.max(1e-3).ln() as f32);
    feature.push(rms);
    feature.push(zero_crossings);
    // 用「秒/有效字符」近似语速。没有文本的样本无法训练，这里给一个中性下限。
    let chars = item
        .text
        .as_deref()
        .map(|text| text.chars().filter(|ch| !ch.is_whitespace()).count())
        .unwrap_or(0);
    let secs_per_char = if chars > 0 {
        item.duration_secs / chars as f64
    } else {
        1e-3
    };
    feature.push(secs_per_char.max(1e-3).ln() as f32);

    Ok(feature)
}

/// 逐维标准化；零方差维度按 1 处理，避免除零。
fn standardize(features: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let count = features.len();
    let dims = features[0].len();
    let mut means = vec![0.0f64; dims];
    for feature in features {
        for (index, value) in feature.iter().enumerate() {
            means[index] += *value as f64;
        }
    }
    for mean in &mut means {
        *mean /= count as f64;
    }

    let mut stds = vec![0.0f64; dims];
    for feature in features {
        for (index, value) in feature.iter().enumerate() {
            let delta = *value as f64 - means[index];
            stds[index] += delta * delta;
        }
    }
    for std in &mut stds {
        *std = (*std / count as f64).sqrt().max(1e-6);
    }

    features
        .iter()
        .map(|feature| {
            feature
                .iter()
                .enumerate()
                .map(|(index, value)| ((*value as f64 - means[index]) / stds[index]) as f32)
                .collect()
        })
        .collect()
}

/// 确定性 xorshift64，只用于 k-means 初始化。
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }
}

fn squared_distance(left: &[f32], right: &[f32]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| {
            let delta = *a as f64 - *b as f64;
            delta * delta
        })
        .sum()
}

fn nearest_center(feature: &[f32], centers: &[Vec<f32>]) -> (usize, f64) {
    let mut best = 0usize;
    let mut best_distance = f64::INFINITY;
    for (index, center) in centers.iter().enumerate() {
        let distance = squared_distance(feature, center);
        if distance < best_distance {
            best_distance = distance;
            best = index;
        }
    }
    (best, best_distance)
}

/// 小规模 k-means：k-means++ 初始化 + 空簇回填最远点。
fn kmeans(features: &[Vec<f32>], k: usize, seed: u64) -> Vec<usize> {
    let count = features.len();
    if k <= 1 || count <= 1 {
        return vec![0; count];
    }

    let mut rng = XorShift64::new(seed);
    let mut centers = Vec::with_capacity(k);
    centers.push(features[(rng.next_u64() as usize) % count].clone());

    while centers.len() < k {
        let mut farthest = 0usize;
        let mut farthest_distance = -1.0f64;
        for (index, feature) in features.iter().enumerate() {
            let (_, distance) = nearest_center(feature, &centers);
            if distance > farthest_distance {
                farthest_distance = distance;
                farthest = index;
            }
        }
        if farthest_distance <= 1e-12 {
            break;
        }
        centers.push(features[farthest].clone());
    }
    while centers.len() < k {
        centers.push(features[(rng.next_u64() as usize) % count].clone());
    }

    let dims = features[0].len();
    let mut assignments = vec![0usize; count];
    for iteration in 0..KMEANS_ITERATIONS {
        let mut changed = false;
        for (index, feature) in features.iter().enumerate() {
            let (cluster, _) = nearest_center(feature, &centers);
            if assignments[index] != cluster {
                assignments[index] = cluster;
                changed = true;
            }
        }
        if !changed && iteration > 0 {
            break;
        }

        let mut sums = vec![vec![0.0f64; dims]; k];
        let mut counts = vec![0usize; k];
        for (index, feature) in features.iter().enumerate() {
            let cluster = assignments[index];
            counts[cluster] += 1;
            for (dim, value) in feature.iter().enumerate() {
                sums[cluster][dim] += *value as f64;
            }
        }
        let mut next_centers = Vec::with_capacity(k);
        for cluster in 0..k {
            if counts[cluster] > 0 {
                next_centers.push(
                    sums[cluster]
                        .iter()
                        .map(|value| (*value / counts[cluster] as f64) as f32)
                        .collect(),
                );
            } else {
                let farthest = features
                    .iter()
                    .enumerate()
                    .max_by(|left, right| {
                        let (_, left_distance) = nearest_center(left.1, &centers);
                        let (_, right_distance) = nearest_center(right.1, &centers);
                        left_distance
                            .partial_cmp(&right_distance)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(index, _)| index)
                    .unwrap_or(0);
                next_centers.push(features[farthest].clone());
            }
        }
        centers = next_centers;
    }

    for (index, feature) in features.iter().enumerate() {
        assignments[index] = nearest_center(feature, &centers).0;
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmeans_separates_two_groups() {
        let features = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.1],
            vec![10.0, 10.0],
            vec![10.1, 10.1],
        ];
        let assignments = kmeans(&features, 2, 7);
        assert_eq!(assignments[0], assignments[1]);
        assert_eq!(assignments[2], assignments[3]);
        assert_ne!(assignments[0], assignments[2]);
    }

    #[test]
    fn standardize_handles_constant_dimension() {
        let features = vec![vec![1.0, 2.0], vec![1.0, 3.0]];
        let normalized = standardize(&features);
        assert!(normalized[0][0].abs() < 1e-6);
        assert!(normalized[1][0].abs() < 1e-6);
    }

    #[test]
    fn cluster_dataset_writes_assignments() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("wavs")).unwrap();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 22_050,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut items = Vec::new();
        for (id, frequency) in [("a", 220.0f32), ("b", 220.0), ("c", 880.0), ("d", 880.0)] {
            let path = dir.path().join(format!("wavs/{id}.wav"));
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            for index in 0..spec.sample_rate {
                let value = (2.0 * std::f32::consts::PI * frequency * index as f32
                    / spec.sample_rate as f32)
                    .sin()
                    * 8_000.0;
                writer.write_sample(value as i16).unwrap();
            }
            writer.finalize().unwrap();
            items.push(DatasetItem {
                id: id.into(),
                wav: format!("wavs/{id}.wav"),
                text: Some(id.into()),
                duration_secs: 1.0,
                style_cluster: None,
            });
        }
        let manifest = DatasetManifest {
            version: 1,
            dataset_id: "cluster-test".into(),
            sample_rate: 22_050,
            created_at_unix: 0,
            items,
            skipped: Vec::new(),
            stats: Default::default(),
        };
        fs::write(
            dir.path().join("dataset.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let report = cluster_dataset(dir.path(), 2).unwrap();
        assert_eq!(report.k, 2);
        assert!(!report.clusters.is_empty());

        let loaded: DatasetManifest =
            serde_json::from_slice(&fs::read(dir.path().join("dataset.json")).unwrap()).unwrap();
        assert!(loaded.items.iter().all(|item| item.style_cluster.is_some()));
    }

    #[test]
    fn summarizes_only_assigned_clusters() {
        let items = vec![
            DatasetItem {
                id: "a".into(),
                wav: "wavs/a.wav".into(),
                text: Some("a".into()),
                duration_secs: 2.0,
                style_cluster: Some(1),
            },
            DatasetItem {
                id: "b".into(),
                wav: "wavs/b.wav".into(),
                text: Some("b".into()),
                duration_secs: 3.0,
                style_cluster: Some(1),
            },
            DatasetItem {
                id: "c".into(),
                wav: "wavs/c.wav".into(),
                text: Some("c".into()),
                duration_secs: 4.0,
                style_cluster: None,
            },
        ];
        let summary = summarize_clusters(&items);
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].cluster, 1);
        assert_eq!(summary[0].items, 2);
        assert!((summary[0].total_secs - 5.0).abs() < 1e-6);
    }
}
