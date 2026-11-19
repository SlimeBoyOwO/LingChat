//! 音频处理：WAV 编解码、参考音频读取、静音裁剪、接缝淡入淡出。

use std::path::Path;

use anyhow::{Result, anyhow};

/// 首尾静音最多保留的秒数；中间停顿是句读，要留着。
pub(super) const LEAD_SILENCE_KEEP: f32 = 0.1;
pub(super) const TRAIL_SILENCE_KEEP: f32 = 0.15;

/// 拼接处淡入淡出多久（秒）。20ms 足够把波形接缝压回静音，又短到听不出音量变化。
const JOINT_FADE: f32 = 0.02;

/// 静音阈值下限/上限，约 -56 / -40 dBFS。
///
/// 固定 0.01 只适合满幅模型：零样本克隆或音量偏小的音色峰值可能只有 0.02~0.05，
/// 语音本身就在阈值附近，词腹的弱音会被当成静音剪掉，听感就是「个别字被吃掉、
/// 语速突然变快」。这里按本段峰值自适应，既不会误伤弱音，也不会把底噪留下。
const SILENCE_FLOOR: f32 = 0.0015;
const SILENCE_CEIL: f32 = 0.01;
/// 阈值取峰值的 -34 dB 左右（0.02 倍）。
const SILENCE_PEAK_RATIO: f32 = 0.02;

/// 按本段峰值自适应静音阈值，并夹在 [`SILENCE_FLOOR`]..=[`SILENCE_CEIL`]。
fn silence_threshold(samples: &[f32]) -> f32 {
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    (peak * SILENCE_PEAK_RATIO).clamp(SILENCE_FLOOR, SILENCE_CEIL)
}

/// 首尾静音的边界 `[start, end)`（整段都静音时 `start >= end`）。
fn silence_bounds(samples: &[f32]) -> (usize, usize) {
    let threshold = silence_threshold(samples);
    let mut start = 0;
    while start < samples.len() && samples[start].abs() <= threshold {
        start += 1;
    }
    let mut end = samples.len();
    while end > start && samples[end - 1].abs() <= threshold {
        end -= 1;
    }
    (start, end)
}

/// 去掉首尾全部静音；整段首尾的留白由调用处按 LEAD/TRAIL 补回。
pub(super) fn strip_edge_silence(samples: &[f32]) -> &[f32] {
    let (start, end) = silence_bounds(samples);
    &samples[start..end]
}

/// 把一段音频内部超过 `max_gap` 秒的静音压到 `max_gap`（句内逗号停顿过长时收口）。
pub(super) fn cap_inner_silence(samples: &mut Vec<f32>, sample_rate: i32, max_gap: f32) {
    if sample_rate <= 0 || max_gap <= 0.0 {
        return;
    }
    let max_len = (sample_rate as f32 * max_gap) as usize;
    let threshold = silence_threshold(samples);
    let mut out = Vec::with_capacity(samples.len());
    let mut i = 0;
    while i < samples.len() {
        if samples[i].abs() > threshold {
            out.push(samples[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < samples.len() && samples[i].abs() <= threshold {
            i += 1;
        }
        // 长静音只留首尾各一半，中间删掉。两侧都停在近零样本上，
        // 拼接点不会从语音直接跳到静音，听感更平顺。
        let run = i - start;
        if run > max_len {
            let head = max_len / 2;
            let tail = max_len - head;
            out.extend_from_slice(&samples[start..start + head]);
            out.extend_from_slice(&samples[i - tail..i]);
        } else {
            out.extend_from_slice(&samples[start..i]);
        }
    }
    *samples = out;
}

/// 首尾各加一小段线性淡入/淡出，压掉拼接处的「咔哒」声。
pub(super) fn apply_edge_fades(samples: &mut [f32], sample_rate: i32) {
    if sample_rate <= 0 {
        return;
    }
    let fade = ((sample_rate as f32 * JOINT_FADE) as usize).min(samples.len() / 2);
    for i in 0..fade {
        let gain = i as f32 / fade as f32;
        samples[i] *= gain;
        let last = samples.len() - 1 - i;
        samples[last] *= gain;
    }
}

/// 目标 RMS 约 -18 dBFS，与 GSV 的 `smooth_wav` 对齐；峰值上限约 -1 dBFS。
const LOUDNESS_TARGET_RMS: f32 = 0.125;
const LOUDNESS_PEAK_CEILING: f32 = 0.891;
/// 低于该 RMS 视为静音，不放大底噪。
const LOUDNESS_SILENCE_RMS: f32 = 0.004;
/// 放大上限 +24 dB：足够救回偏小的零样本克隆，又不至于把底噪推满。
const LOUDNESS_MAX_GAIN: f32 = 16.0;

/// 按 RMS 对齐响度，同时用峰值上限挡住削波。
///
/// 不同模型/参考音频的输出电平差很多；不归一化时小音量模型听起来「几乎没声」。
/// RMS 只统计语音段（高于自适应静音阈值的样本），避免把句间停顿算进去拉低响度。
pub(super) fn normalize_loudness(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }

    let threshold = silence_threshold(samples);
    let mut sum_sq = 0.0f64;
    let mut speech = 0usize;
    let mut peak = 0.0f32;
    for s in samples.iter().copied() {
        let amp = s.abs();
        peak = peak.max(amp);
        if amp > threshold {
            sum_sq += (s as f64) * (s as f64);
            speech += 1;
        }
    }
    if speech == 0 || peak <= 0.0 {
        return;
    }

    let rms = (sum_sq / speech as f64).sqrt() as f32;
    if rms < LOUDNESS_SILENCE_RMS {
        return;
    }

    let mut gain = (LOUDNESS_TARGET_RMS / rms).min(LOUDNESS_MAX_GAIN);
    // 峰值不得越过上限，否则会在解码/播放时削波失真。
    if peak * gain > LOUDNESS_PEAK_CEILING {
        gain = LOUDNESS_PEAK_CEILING / peak;
    }
    // 已经接近目标时不做无意义的微调。
    if (gain - 1.0).abs() < 0.01 {
        return;
    }

    for s in &mut *samples {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}

pub(super) fn f32_to_wav(samples: &[f32], sample_rate: i32) -> Vec<u8> {
    let num_samples = samples.len();
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;
    let sr = sample_rate as u32;
    let byte_rate = sr * 2;

    let mut wav = Vec::with_capacity(44 + data_size);

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(file_size as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sr.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());

    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data_size as u32).to_le_bytes());

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        wav.extend_from_slice(&i16_sample.to_le_bytes());
    }

    wav
}

pub fn load_reference_audio(path: &Path) -> Result<(Vec<f32>, i32)> {
    // Android 的 content:// URI 读不了，提示用户重新导入。
    if path.to_string_lossy().starts_with("content://") {
        return Err(anyhow!(
            "参考音频是 Android 的 content:// URI，请先在语音设置里重新导入一次（会复制到应用数据目录）: {}",
            path.display()
        ));
    }

    // sherpa-onnx 零样本克隆仅接受 WAV。
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "wav" {
        return Err(anyhow!(
            "参考音频仅支持 WAV 格式，当前文件格式: {} ({}). 请转换为 WAV 后重试",
            ext,
            path.display()
        ));
    }

    let data = std::fs::read(path)
        .map_err(|e| anyhow!("无法读取参考音频文件 {}: {}", path.display(), e))?;

    // 按块流解析：`fmt ` 前可能有 `bext` / `LIST` / `JUNK` 等块，固定偏移会读错。
    let wav = parse_wav(&data)
        .ok_or_else(|| anyhow!("WAV 文件结构不对（缺 fmt / data 块）: {}", path.display()))?;
    let format = wav.format;

    // 采样率要原样交给 sherpa 重采样；读错会让音色、语速全变。
    if !(1..=384_000).contains(&format.sample_rate) {
        return Err(anyhow!(
            "WAV 头里的采样率不对（读到 {}），请用「导入参考音频」重新导入: {}",
            format.sample_rate,
            path.display()
        ));
    }
    if !(1..=8).contains(&format.channels) {
        return Err(anyhow!(
            "WAV 头里的声道数不对（读到 {}）: {}",
            format.channels,
            path.display()
        ));
    }

    let samples = decode_wav_samples(&data[wav.offset..wav.offset + wav.len], &format)
        .map_err(|e| anyhow!("{e}: {}", path.display()))?;

    // 多声道取平均；末尾不足一帧的残字节丢掉。
    let mono_samples = if format.channels > 1 {
        samples
            .chunks_exact(format.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / format.channels as f32)
            .collect()
    } else {
        samples
    };

    if mono_samples.is_empty() {
        return Err(anyhow!("参考音频为空"));
    }

    Ok((mono_samples, format.sample_rate))
}

/// WAV 的 `fmt ` 块内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WavFormat {
    /// 1 = PCM，3 = IEEE float（`0xFFFE` = WAVE_FORMAT_EXTENSIBLE 已按子格式解开）。
    format_tag: u16,
    channels: u16,
    sample_rate: i32,
    bits_per_sample: u16,
}

/// 解析出来的 WAV：格式 + `data` 块在文件里的位置。
struct WavData {
    format: WavFormat,
    /// `data` 块载荷的起始偏移。
    offset: usize,
    /// `data` 块载荷的字节数（以文件实际长度收口）。
    len: usize,
}

/// 遍历 RIFF 块，定位 `fmt ` 与 `data`。
fn parse_wav(data: &[u8]) -> Option<WavData> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }

    let mut format = None;
    let mut audio: Option<(usize, usize)> = None;
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let id: [u8; 4] = data[pos..pos + 4].try_into().ok()?;
        let declared = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body = pos + 8;
        // 截断的文件里块长度可能超过实际剩余字节，按实际长度收口。
        let end = body.saturating_add(declared).min(data.len());
        match &id {
            b"fmt " => format = parse_fmt_chunk(&data[body..end]),
            b"data" => audio = Some((body, end - body)),
            _ => {},
        }
        // RIFF 块按偶数字节对齐。
        pos = end.saturating_add(declared & 1).max(body);
    }

    Some(WavData {
        format: format?,
        offset: audio?.0,
        len: audio?.1,
    })
}

fn parse_fmt_chunk(body: &[u8]) -> Option<WavFormat> {
    if body.len() < 16 {
        return None;
    }
    let mut format_tag = u16::from_le_bytes([body[0], body[1]]);
    let channels = u16::from_le_bytes([body[2], body[3]]);
    let sample_rate = i32::from_le_bytes([body[4], body[5], body[6], body[7]]);
    let bits_per_sample = u16::from_le_bytes([body[14], body[15]]);
    // WAVE_FORMAT_EXTENSIBLE（24 位/多声道常见）：真正的编码在 fmt 块尾部
    // SubFormat GUID 的前两个字节。
    if format_tag == 0xFFFE && body.len() >= 26 {
        format_tag = u16::from_le_bytes([body[24], body[25]]);
    }
    Some(WavFormat {
        format_tag,
        channels,
        sample_rate,
        bits_per_sample,
    })
}

fn decode_chunks<const N: usize>(raw: &[u8], f: impl Fn([u8; N]) -> f32) -> Vec<f32> {
    raw.as_chunks::<N>()
        .0
        .iter()
        .map(|&bytes| f(bytes))
        .collect()
}

/// 把 WAV 的样本数据解码成 -1.0~1.0 的 f32。
fn decode_wav_samples(raw: &[u8], format: &WavFormat) -> Result<Vec<f32>> {
    const PCM: u16 = 1;
    const FLOAT: u16 = 3;

    let samples = match (format.format_tag, format.bits_per_sample) {
        // 8 位 WAV 是无符号 PCM，中点 128。
        (PCM, 8) => raw.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect(),
        (PCM, 16) => decode_chunks::<2>(raw, |b| i16::from_le_bytes(b) as f32 / 32_768.0),
        (PCM, 24) => decode_chunks::<3>(raw, |b| {
            // 24 位小端、手工符号扩展。
            let v = ((b[2] as i8 as i32) << 16) | ((b[1] as i32) << 8) | b[0] as i32;
            v as f32 / 8_388_608.0
        }),
        (PCM, 32) => decode_chunks::<4>(raw, |b| i32::from_le_bytes(b) as f32 / 2_147_483_648.0),
        (FLOAT, 32) => decode_chunks::<4>(raw, |b| f32::from_le_bytes(b)),
        (FLOAT, 64) => decode_chunks::<8>(raw, |b| f64::from_le_bytes(b) as f32),
        (tag, bits) => {
            return Err(anyhow!(
                "不支持的 WAV 编码（format tag = {tag}、位深 = {bits} 位；支持 PCM 8/16/24/32 与 float 32/64）"
            ));
        },
    };
    Ok(samples)
}

/// 解析生成的 WAV，返回 (采样率, 单声道样本数)。
#[cfg(test)]
pub(super) fn parse_generated_wav(wav: &[u8]) -> (u32, usize) {
    let parsed = parse_wav(wav).expect("缺 RIFF/WAVE/fmt /data 头");
    assert_eq!(parsed.format.format_tag, 1, "输出应是 PCM");
    assert_eq!(parsed.format.channels, 1, "输出应是单声道");
    assert_eq!(parsed.format.bits_per_sample, 16);
    assert_eq!(wav.len(), 44 + parsed.len, "WAV 里有多余字节");
    (parsed.format.sample_rate as u32, parsed.len / 2)
}

/// 生成的 WAV 里有没有幅度超过 `threshold` 的样本（用来判断是否全静音）。
#[cfg(test)]
pub(super) fn pcm_has_signal(wav: &[u8], threshold: u16) -> bool {
    let Some(parsed) = parse_wav(wav) else {
        return false;
    };
    wav[parsed.offset..parsed.offset + parsed.len]
        .as_chunks::<2>()
        .0
        .iter()
        .any(|c| i16::from_le_bytes(*c).unsigned_abs() > threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 片内长静音压到上限，短静音与波形不动。
    #[test]
    fn caps_inner_silence_only() {
        let sr = 1000; // 1 个样本 = 1ms，方便数
        let mut samples = vec![0.0; 500];
        samples.extend(std::iter::repeat_n(0.5, 100));
        samples.extend(std::iter::repeat_n(0.0, 600));
        samples.extend(std::iter::repeat_n(0.5, 100));
        samples.extend(std::iter::repeat_n(0.0, 500));
        cap_inner_silence(&mut samples, sr, 0.2);
        // 中间 600ms 压到 200ms；首尾也不是例外（这里一视同仁，之后
        // `strip_edge_silence` 会把首尾清掉），500ms → 200ms。
        assert_eq!(samples.len(), 200 + 100 + 200 + 100 + 200);
        assert_eq!(samples.iter().filter(|s| s.abs() > 0.1).count(), 200);
    }

    /// 低电平克隆的弱音不能被固定阈值当成静音剪掉（否则就是「吃字、语速变快」）。
    #[test]
    fn keeps_quiet_speech_above_adaptive_threshold() {
        // 峰值 0.004：旧固定阈值 0.01 会把整段判成静音、裁成空。
        let quiet = vec![0.004f32; 200];
        assert_eq!(strip_edge_silence(&quiet).len(), 200);

        // 句内长静音收口时，弱音同样要保留。
        let mut samples = vec![0.0; 300];
        samples.extend(std::iter::repeat_n(0.004, 100));
        samples.extend(std::iter::repeat_n(0.0, 300));
        samples.extend(std::iter::repeat_n(0.004, 100));
        samples.extend(std::iter::repeat_n(0.0, 300));
        cap_inner_silence(&mut samples, 1000, 0.2);
        assert_eq!(samples.len(), 200 + 100 + 200 + 100 + 200);
        assert_eq!(samples.iter().filter(|s| s.abs() > 0.001).count(), 200);
    }

    /// 低电平音频按 RMS 抬到目标响度，且不越过峰值上限。
    #[test]
    fn normalizes_quiet_audio_without_clipping() {
        let mut samples = vec![0.01f32; 1000];
        normalize_loudness(&mut samples);
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let rms = sample_rms(&samples);
        assert!(rms > 0.1, "低电平应被抬高，rms={rms}");
        assert!(
            peak <= LOUDNESS_PEAK_CEILING + 1e-6,
            "不得削波，peak={peak}"
        );
    }

    /// RMS 想抬得更多时，峰值上限优先。
    #[test]
    fn loudness_gain_respects_peak_ceiling() {
        let mut samples = vec![0.02f32; 1000];
        samples[500] = 0.2;
        normalize_loudness(&mut samples);
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            (peak - LOUDNESS_PEAK_CEILING).abs() < 1e-3,
            "峰值应被压在上限，peak={peak}"
        );
    }

    /// 近乎全静音时不放大底噪。
    #[test]
    fn normalizes_skips_near_silence() {
        let mut samples = vec![0.001f32; 100];
        normalize_loudness(&mut samples);
        assert!(samples.iter().all(|s| (*s - 0.001).abs() < 1e-6));
    }

    fn sample_rms(samples: &[f32]) -> f32 {
        let sum: f64 = samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        (sum / samples.len() as f64).sqrt() as f32
    }

    /// 淡入淡出：首尾归零、中间不动。
    #[test]
    fn fades_clause_edges_and_strips_silence() {
        let sample_rate = 1000; // 20ms = 20 个样本
        let mut samples = vec![1.0f32; 100];
        apply_edge_fades(&mut samples, sample_rate);
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[99], 0.0);
        assert!(samples[10] < 1.0 && samples[19] < 1.0);
        assert_eq!(samples[20], 1.0, "20ms 之后的样本不该被动");
        assert_eq!(samples[50], 1.0);

        // 首尾静音要全部去掉（停顿由调用处按标点精确插入）。
        let mut padded = vec![0.0; 10];
        padded.extend_from_slice(&[0.5; 5]);
        padded.extend_from_slice(&[0.0; 10]);
        let edge = strip_edge_silence(&padded);
        assert_eq!(edge.len(), 5);
        assert!(edge.iter().all(|s| *s == 0.5));
    }

    /// WAV 头按块流解析：`fmt ` 前多一个块时固定偏移会读错采样率。
    #[test]
    fn parses_wav_with_chunks_before_fmt() {
        let data = build_wav(&[
            (b"bext", &[b'0'; 602]),
            (b"fmt ", &pcm16_fmt(44_100, 1)),
            (b"data", &[0u8; 8]),
        ]);
        let parsed = parse_wav(&data).expect("应能解析");
        assert_eq!(parsed.format.sample_rate, 44_100);
        assert_eq!(parsed.format.channels, 1);
        assert_eq!(parsed.format.bits_per_sample, 16);
        assert_eq!(parsed.len, 8);

        // 标准头（fmt 在最前）同样要对。
        let data = build_wav(&[(b"fmt ", &pcm16_fmt(22_050, 2)), (b"data", &[0u8; 16])]);
        let parsed = parse_wav(&data).expect("应能解析");
        assert_eq!(parsed.format.sample_rate, 22_050);
        assert_eq!(parsed.format.channels, 2);
    }

    /// 16/24/32 位 PCM 与 float 都要按正确的解释读。
    #[test]
    fn decodes_reference_audio_formats() {
        // 24 位：0x7FFFFF 是满幅正，0x800000 是满幅负。
        let raw = [0xFFu8, 0xFF, 0x7F, 0x00, 0x00, 0x80];
        let fmt = WavFormat {
            format_tag: 1,
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 24,
        };
        let samples = decode_wav_samples(&raw, &fmt).expect("24 位应能解码");
        assert!((samples[0] - 1.0).abs() < 1e-6);
        assert!((samples[1] + 1.0).abs() < 1e-6);

        // float32 的位模式按 float 读，而不是按 int32 读（按 int32 读会得到畸变波形）。
        let raw: Vec<u8> = [0.25f32, -0.5]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let fmt = WavFormat {
            format_tag: 3,
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 32,
        };
        let samples = decode_wav_samples(&raw, &fmt).expect("float32 应能解码");
        assert_eq!(samples, vec![0.25, -0.5]);

        // 不认识的编码要报错，别把 ADPCM 当 PCM 读。
        let fmt = WavFormat {
            format_tag: 2,
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 4,
        };
        assert!(decode_wav_samples(&raw, &fmt).is_err());
    }

    /// `content://` 路径要给出可操作的提示。
    #[test]
    fn rejects_content_uri_reference() {
        let err = load_reference_audio(Path::new("content://media/external/audio/1.wav"))
            .expect_err("content:// 应直接报错");
        assert!(err.to_string().contains("重新导入"), "{err}");
    }

    /// 拼一个最简 WAV（只有 RIFF 头 + 给定块序列）。
    fn build_wav(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut body = b"WAVE".to_vec();
        for (id, payload) in chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            body.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&(body.len() as u32).to_le_bytes());
        wav.extend_from_slice(&body);
        wav
    }

    /// 16 位 PCM 的 fmt 块。
    fn pcm16_fmt(sample_rate: u32, channels: u16) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes()); // PCM
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&sample_rate.to_le_bytes());
        fmt.extend_from_slice(&(sample_rate * 2 * channels as u32).to_le_bytes());
        fmt.extend_from_slice(&(2 * channels).to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        fmt
    }
}
