//! 训练音频归一化：统一为单声道 16-bit PCM WAV，并去直流、裁静音、调响度。

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::resample::resample_mono;

/// 归一化后的峰值上限（留 1 dB 余量防削顶）。
pub const TARGET_PEAK_DBFS: f32 = -1.0;
/// 归一化后的目标 RMS。
pub const TARGET_RMS_DBFS: f32 = -23.0;
/// 静音判定阈值，用于首尾裁剪。
const SILENCE_THRESHOLD_DBFS: f32 = -45.0;
/// 首尾保留的裁剪余量，避免切掉字头字尾。
const TRIM_PADDING_SECS: f32 = 0.05;

/// 单条归一化结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizeReport {
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    /// 首尾静音裁掉的时长（秒）。
    pub trimmed_secs: f64,
}

/// 把 `src` 归一化并写成 `dest`；临时转码文件在结束时删除。
pub fn normalize_to_wav(
    src: &Path,
    dest: &Path,
    target_sample_rate: u32,
) -> Result<NormalizeReport> {
    let temp = dest.with_extension("decode.wav");
    crate::utils::audio::transcode_to_wav_16bit(src, &temp)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| format!("解码音频失败: {}", src.display()))?;

    let decoded = read_wav_mono(&temp);
    let _ = std::fs::remove_file(&temp);
    let (samples, source_rate) = decoded?;

    if samples.is_empty() {
        anyhow::bail!("音频为空: {}", src.display());
    }

    let mut samples = resample_mono(&samples, source_rate, target_sample_rate);
    remove_dc_offset(&mut samples);

    let before = samples.len() as f64 / f64::from(target_sample_rate);
    samples = trim_silence(&samples, target_sample_rate);
    let after = samples.len() as f64 / f64::from(target_sample_rate);

    if samples.is_empty() {
        anyhow::bail!("裁剪静音后没有有效音频: {}", src.display());
    }

    let (peak, rms) = normalize_loudness(&mut samples);

    write_wav_mono(dest, &samples, target_sample_rate)?;

    Ok(NormalizeReport {
        duration_secs: after,
        sample_rate: target_sample_rate,
        peak_dbfs: to_dbfs(peak),
        rms_dbfs: to_dbfs(rms),
        trimmed_secs: (before - after).max(0.0),
    })
}

/// 读入 WAV，统一下混成单声道 f32（范围 -1..1）。
pub(crate) fn read_wav_mono(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("打开 WAV 失败: {}", path.display()))?;
    let spec = reader.spec();

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample.saturating_sub(1))) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<std::result::Result<Vec<_>, _>>()?
        },
    };

    let channels = spec.channels.max(1) as usize;
    let mono = if channels == 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    Ok((mono, spec.sample_rate))
}

/// 写单声道 16-bit PCM WAV。
fn write_wav_mono(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("创建 WAV 失败: {}", path.display()))?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer.write_sample(value)?;
    }
    writer.finalize()?;
    Ok(())
}

/// 去掉直流偏置，避免归一化后整体偏移。
fn remove_dc_offset(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    for sample in samples.iter_mut() {
        *sample -= mean;
    }
}

/// 裁掉首尾低于阈值的静音，保留少量余量。
fn trim_silence(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    let threshold = dbfs_to_linear(SILENCE_THRESHOLD_DBFS);
    let first = samples.iter().position(|s| s.abs() > threshold);
    let last = samples.iter().rposition(|s| s.abs() > threshold);

    let (Some(first), Some(last)) = (first, last) else {
        return Vec::new();
    };

    let padding = (TRIM_PADDING_SECS * sample_rate as f32) as usize;
    let start = first.saturating_sub(padding);
    let end = (last + padding + 1).min(samples.len());
    samples[start..end].to_vec()
}

/// 先按目标 RMS 提升，再用峰值上限兜底。返回归一化后的峰值与 RMS。
fn normalize_loudness(samples: &mut [f32]) -> (f32, f32) {
    let peak = samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
    if peak <= f32::EPSILON {
        return (0.0, 0.0);
    }

    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let rms_gain = if rms > f32::EPSILON {
        dbfs_to_linear(TARGET_RMS_DBFS) / rms
    } else {
        1.0
    };
    // 放大后峰值不能超过上限。
    let peak_gain = dbfs_to_linear(TARGET_PEAK_DBFS) / peak;
    let gain = rms_gain.min(peak_gain);

    for sample in samples.iter_mut() {
        *sample *= gain;
    }

    let peak_after = samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
    let rms_after = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    (peak_after, rms_after)
}

fn to_dbfs(value: f32) -> f32 {
    if value <= f32::EPSILON {
        f32::NEG_INFINITY
    } else {
        20.0 * value.log10()
    }
}

fn dbfs_to_linear(dbfs: f32) -> f32 {
    10f32.powf(dbfs / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_wav(path: &Path, samples: &[f32], sample_rate: u32) {
        write_wav_mono(path, samples, sample_rate).unwrap();
    }

    #[test]
    fn removes_dc_offset() {
        let mut samples = vec![0.5f32; 100];
        remove_dc_offset(&mut samples);
        assert!(samples.iter().all(|s| s.abs() < 1e-6));
    }

    #[test]
    fn trims_leading_and_trailing_silence() {
        let mut samples = vec![0.0f32; 1_000];
        for sample in samples.iter_mut().skip(400).take(200) {
            *sample = 0.5;
        }
        let trimmed = trim_silence(&samples, 1_000);
        // 200 个有效样本 + 首尾各 50 个余量。
        assert_eq!(trimmed.len(), 300);
    }

    #[test]
    fn all_silence_trims_to_empty() {
        let samples = vec![0.0f32; 1_000];
        assert!(trim_silence(&samples, 1_000).is_empty());
    }

    #[test]
    fn loudness_respects_peak_ceiling() {
        let mut samples = vec![0.0f32; 10_000];
        for (i, sample) in samples.iter_mut().enumerate() {
            *sample = ((i as f32) * 0.05).sin() * 0.02;
        }
        let (peak, _) = normalize_loudness(&mut samples);
        assert!(peak <= dbfs_to_linear(TARGET_PEAK_DBFS) + 1e-4);
    }

    #[test]
    fn normalize_roundtrip_produces_target_rate() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.wav");
        let dest = dir.path().join("out.wav");
        let samples: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        write_test_wav(&src, &samples, 48_000);

        let report = normalize_to_wav(&src, &dest, 22_050).unwrap();
        assert_eq!(report.sample_rate, 22_050);

        let (out, rate) = read_wav_mono(&dest).unwrap();
        assert_eq!(rate, 22_050);
        assert!((out.len() as f64 / 22_050.0 - report.duration_secs).abs() < 0.05);
    }
}
