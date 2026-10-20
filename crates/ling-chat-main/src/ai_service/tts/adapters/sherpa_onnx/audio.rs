//! 音频处理：WAV 编解码、参考音频读取、首尾静音裁剪、接缝淡入淡出。
//!
//! 逐句合成后由 [`apply_edge_fades`] 把接缝压回静音，停顿长度则由调用处按标点精确
//! 插入——各模型自带的片首尾静音长短不一（0.1~0.4 秒），直接拼接会让停顿忽长忽短。

use std::path::Path;

use anyhow::{Result, anyhow};

/// 首尾静音最多各保留多久（秒）。中间的停顿是句读，要留着；前后的空白在多段台词的
/// 场景里只会变成「每句都要等一会儿才开口」——zipvoice 逐次推理的段首静音实测能到
/// 0.4 秒，各模型之间也不一致。
pub(super) const LEAD_SILENCE_KEEP: f32 = 0.1;
pub(super) const TRAIL_SILENCE_KEEP: f32 = 0.15;

/// 拼接处淡入淡出多久（秒）。20ms 足够把波形接缝压回静音，又短到听不出音量变化。
const JOINT_FADE: f32 = 0.02;

/// 静音判定阈值，与 sherpa 的 `ScaleSilence` 一致（0.01，约 -40 dBFS）。
const SILENCE_AMP: f32 = 0.01;

/// 首尾静音的边界 `[start, end)`（整段都静音时 `start >= end`）。
fn silence_bounds(samples: &[f32]) -> (usize, usize) {
    let mut start = 0;
    while start < samples.len() && samples[start].abs() <= SILENCE_AMP {
        start += 1;
    }
    let mut end = samples.len();
    while end > start && samples[end - 1].abs() <= SILENCE_AMP {
        end -= 1;
    }
    (start, end)
}

/// 去掉首尾**全部**静音（不留余量）。
///
/// 逐句合成时片与片之间要插我们自己算好的停顿，各片自带的静音（0.1~0.4 秒不等，
/// 模型之间还不一样）只会让停顿忽长忽短，所以这里一点不留；整段首尾的静音由
/// 调用处按 [`LEAD_SILENCE_KEEP`] / [`TRAIL_SILENCE_KEEP`] 补回来。
pub(super) fn strip_edge_silence(samples: &[f32]) -> &[f32] {
    let (start, end) = silence_bounds(samples);
    &samples[start..end]
}

/// 给一段音频的首尾各加一小段线性淡入/淡出（时长见 [`JOINT_FADE`]）。
///
/// 拼接处需要它：两截波形在接缝处的幅值一般不连续（sherpa 内部就是直接把多段
/// `insert` 到一起的），听感上就是「咔哒」一声。淡入淡出把接缝两侧压到 0，
/// 接缝就落回静音里了。
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
    // Android 的 content:// URI（早期版本直接存进设置的路径）读不了，早点说清楚
    // 该怎么办，别让用户看到下面那句「仅支持 WAV」而摸不着头脑。
    if path.to_string_lossy().starts_with("content://") {
        return Err(anyhow!(
            "参考音频是 Android 的 content:// URI，请先在语音设置里重新导入一次（会复制到应用数据目录）: {}",
            path.display()
        ));
    }

    // 仅支持 WAV 格式（sherpa-onnx 零样本克隆要求 WAV 输入）
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

    // 按块流解析，而不是按固定偏移：`fmt ` 前面可能排着 `bext`（广播 WAV）、`LIST`、
    // `JUNK` 等块（摄像/录音软件导出很常见），固定偏移在那里读到的是无关字节。
    let wav = parse_wav(&data)
        .ok_or_else(|| anyhow!("WAV 文件结构不对（缺 fmt / data 块）: {}", path.display()))?;
    let format = wav.format;

    // 采样率要原样交给 sherpa 重采样：读错一位，参考音频就等于被按错误的速度播放，
    // 合成出来音色、语速全变（实测把 22050 的样本标成 48000，时长少一半；标成
    // 16000，时长多三成，音色距离 41~55‰——接近「换了个人」的量级）。
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

    // 多声道取平均。chunks_exact：末尾不足一帧的残字节丢掉，不能让它们单独成一帧。
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

/// 把 WAV 的样本数据解码成 -1.0~1.0 的 f32。
fn decode_wav_samples(raw: &[u8], format: &WavFormat) -> Result<Vec<f32>> {
    const PCM: u16 = 1;
    const FLOAT: u16 = 3;

    let samples: Vec<f32> = match (format.format_tag, format.bits_per_sample) {
        // 8 位 WAV 是无符号 PCM，中点 128。
        (PCM, 8) => raw.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect(),
        (PCM, 16) => raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0)
            .collect(),
        (PCM, 24) => raw
            .as_chunks::<3>()
            .0
            .iter()
            .map(|b| {
                // 24 位小端、手工符号扩展。
                let v = ((b[2] as i8 as i32) << 16) | ((b[1] as i32) << 8) | b[0] as i32;
                v as f32 / 8_388_608.0
            })
            .collect(),
        (PCM, 32) => raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0)
            .collect(),
        (FLOAT, 32) => raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect(),
        (FLOAT, 64) => raw
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| f64::from_le_bytes(*b) as f32)
            .collect(),
        (tag, bits) => {
            return Err(anyhow!(
                "不支持的 WAV 编码（format tag = {tag}、位深 = {bits} 位；支持 PCM 8/16/24/32 与 float 32/64）"
            ));
        },
    };
    Ok(samples)
}

/// 解析我们生成的 WAV 字节并返回 (采样率, 单声道样本数)。
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

    /// 拼接处的淡入淡出：首尾样本归零、中间不动，20ms 以外不受影响。
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

    /// WAV 头必须按块流解析：`fmt ` 前面多一个块（广播 WAV 的 `bext`、`LIST`）时，
    /// 固定偏移读到的是无关字节（实测 44100Hz 被读成 sr=808464432）。
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

    /// 参考音频的样本格式：16/24/32 位 PCM 与 32 位 float 都要按对的解释读。
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

    /// Android 早期版本存下来的 `content://` 路径要给出可操作的提示。
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

    /// 16 位 PCM 的 fmt 块内容。
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
