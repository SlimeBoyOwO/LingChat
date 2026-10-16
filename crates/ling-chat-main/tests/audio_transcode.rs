//! `utils::audio::transcode_to_wav_16bit` 的回归测试。
//!
//! 重点覆盖「以前会被 `load_reference_audio` 拒掉的输入」：24 位 / 浮点 WAV。
//! 后者是手工解析 RIFF 头的老实现读不了的（只认 16/32 位整型），而用户从
//! 音乐 App 或录音笔导出的参考音频经常就是这些格式。
//!
//! 测试素材尽量用 hound 现场生成；Opus 是例外（没有编码器 + OGG 封装器就造不出
//! 来），用生产依赖里的 `opus` 编码、dev 依赖 `ogg` 封装，同样不引入二进制 fixture。

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use hound::{SampleFormat, WavSpec, WavWriter};
use ling_chat_main::utils::audio::{
    MAX_REFERENCE_AUDIO_SECONDS, TranscodeError, transcode_to_wav_16bit,
};
use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use opus::{Application, Channels as OpusChannels, Encoder};

/// 写一个最小 WAV。
fn write_wav<S: hound::Sample + Copy>(path: &Path, spec: WavSpec, samples: &[S]) {
    let mut writer = WavWriter::create(path, spec).expect("创建测试 WAV");
    for &sample in samples {
        writer.write_sample(sample).expect("写入测试样本");
    }
    writer.finalize().expect("收尾测试 WAV");
}

/// 读回 WAV 的规格与 16 位样本。
fn read_wav(path: &Path) -> (WavSpec, Vec<i16>) {
    let mut reader = hound::WavReader::open(path).expect("打开结果 WAV");
    let spec = reader.spec();
    let samples = reader
        .samples::<i16>()
        .map(|s| s.expect("读取结果样本"))
        .collect();
    (spec, samples)
}

fn temp_pair() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let src = dir.path().join("source.wav");
    (dir, src)
}

/// 24 位 PCM：老的 `load_reference_audio` 会报「不支持的 WAV 位深度」。
#[test]
fn accepts_24bit_pcm_wav() {
    let (dir, src) = temp_pair();
    // 24 位满量程 ±8388608，0.5 → 4194304。
    write_wav(
        &src,
        WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 24,
            sample_format: SampleFormat::Int,
        },
        &[4_194_304i32, -4_194_304, 0, 8_388_607, -8_388_608],
    );

    let dest = dir.path().join("out.wav");
    transcode_to_wav_16bit(&src, &dest).expect("24 位 WAV 应能转码");

    let (spec, samples) = read_wav(&dest);
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 16_000);
    assert_eq!(spec.bits_per_sample, 16);
    assert_eq!(spec.sample_format, SampleFormat::Int);
    assert_eq!(samples.len(), 5);
    assert!(
        (samples[0] - 16_384).abs() <= 2,
        "0.5 应约等于 16384: {}",
        samples[0]
    );
    assert!(
        (samples[1] + 16_384).abs() <= 2,
        "-0.5 应约等于 -16384: {}",
        samples[1]
    );
    assert_eq!(samples[2], 0);
    assert!(
        samples[3] >= 32_760,
        "满量程应接近 i16 上限: {}",
        samples[3]
    );
    assert!(
        samples[4] <= -32_760,
        "满量程应接近 i16 下限: {}",
        samples[4]
    );
}

/// 32 位浮点 WAV：老实现会把它当 i32 直接读，样本值全是错的（不报错、更难查）。
#[test]
fn accepts_float_wav() {
    let (dir, src) = temp_pair();
    write_wav(
        &src,
        WavSpec {
            channels: 1,
            sample_rate: 22_050,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        },
        &[0.5f32, -0.5, 0.0, 1.0],
    );

    let dest = dir.path().join("out.wav");
    transcode_to_wav_16bit(&src, &dest).expect("浮点 WAV 应能转码");

    let (spec, samples) = read_wav(&dest);
    assert_eq!(spec.sample_rate, 22_050);
    assert_eq!(spec.bits_per_sample, 16);
    assert!((samples[0] - 16_384).abs() <= 2, "0.5 → {}", samples[0]);
    assert!((samples[1] + 16_384).abs() <= 2, "-0.5 → {}", samples[1]);
    assert_eq!(samples[2], 0);
    assert!(samples[3] >= 32_760, "1.0 → {}", samples[3]);
}

/// 立体声与帧数保持不变：`load_reference_audio` 会自己下混单声道。
#[test]
fn keeps_channels_and_frames() {
    let (dir, src) = temp_pair();
    let input = [1_000i16, -1_000, 2_000, -2_000, 3_000, -3_000];
    write_wav(
        &src,
        WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        },
        &input,
    );

    let dest = dir.path().join("out.wav");
    transcode_to_wav_16bit(&src, &dest).expect("立体声 WAV 应能转码");

    let (spec, samples) = read_wav(&dest);
    assert_eq!(spec.channels, 2);
    assert_eq!(spec.sample_rate, 48_000);
    assert_eq!(samples.len(), input.len());
    for (before, after) in input.iter().zip(samples.iter()) {
        // i16 → f32 → i16 的往返会有 ±1 的取整误差。
        assert!((i32::from(*before) - i32::from(*after)).abs() <= 1);
    }
}

/// 不是音频：报「格式不支持」，且不留下半成品文件。
#[test]
fn rejects_non_audio_and_cleans_up() {
    let (dir, src) = temp_pair();
    std::fs::write(&src, b"RIFF\x00\x00\x00\x00WEBPnot-really-audio").expect("写入伪文件");

    let dest = dir.path().join("out.wav");
    let err = transcode_to_wav_16bit(&src, &dest).expect_err("非音频应被拒");
    assert!(
        matches!(err, TranscodeError::Unsupported(_)),
        "应报格式不支持，实际: {err:?}"
    );
    assert!(!dest.exists(), "失败时不应留下半成品");
}

/// 超过时长上限：按总帧数提前拒，不写文件。
#[test]
fn rejects_overlong_audio() {
    let (dir, src) = temp_pair();
    let rate = 8_000u32;
    let frames = rate * (MAX_REFERENCE_AUDIO_SECONDS as u32 + 1);
    write_wav(
        &src,
        WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        },
        &vec![0i16; frames as usize],
    );

    let dest = dir.path().join("out.wav");
    let err = transcode_to_wav_16bit(&src, &dest).expect_err("超长音频应被拒");
    match err {
        TranscodeError::TooLong { seconds, limit } => {
            assert!(seconds > limit as f64);
            assert_eq!(limit, MAX_REFERENCE_AUDIO_SECONDS);
        },
        other => panic!("应报超长，实际: {other:?}"),
    }
    assert!(!dest.exists(), "失败时不应留下半成品");
}

/// Opus 的编码层固定按 48 kHz 工作，与文件的原始采样率无关。
const OPUS_RATE: u32 = 48_000;
/// Opus 一包 20 ms（960 帧），是最常见的打包方式。
const OPUS_FRAME: usize = 960;
/// OGG 逻辑流序号（测试里随便取一个固定值即可）。
const OPUS_SERIAL: u32 = 0x10AD_BEEF;

/// 写 OpusHead（RFC 7845 §5.1）。
///
/// 6 声道等多声道用 channel mapping family 1：这里按「每个声道一条独立流、不做
/// 声道耦合」写映射表，是合法的 family 1 头，不需要为测试专用 5.1 的排布。
fn write_opus_head<W: Write>(writer: &mut PacketWriter<'_, W>, channels: u8) {
    let mut head = Vec::with_capacity(19 + usize::from(channels));
    head.extend_from_slice(b"OpusHead");
    head.push(1); // 封装版本
    head.push(channels);
    head.extend_from_slice(&312u16.to_le_bytes()); // pre-skip
    head.extend_from_slice(&OPUS_RATE.to_le_bytes()); // 原始采样率（仅元数据）
    head.extend_from_slice(&0i16.to_le_bytes()); // output gain
    if channels <= 2 {
        head.push(0); // family 0：单/双声道
    } else {
        head.push(1); // family 1
        head.push(channels); // stream count
        head.push(0); // coupled count
        for channel in 0..channels {
            head.push(channel); // 声道映射表
        }
    }
    writer
        .write_packet(head, OPUS_SERIAL, PacketWriteEndInfo::EndPage, 0)
        .expect("写 OpusHead");
}

/// 写 OpusTags（可以没有注释，但这一页不能省）。
fn write_opus_tags<W: Write>(writer: &mut PacketWriter<'_, W>) {
    let vendor = b"ling-chat-test";
    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    writer
        .write_packet(tags, OPUS_SERIAL, PacketWriteEndInfo::EndPage, 0)
        .expect("写 OpusTags");
}

/// 现场编一个单声道 OGG/Opus（正弦波，振幅 0.5，时长按帧补齐到 20 ms 的整数倍）。
fn write_ogg_opus(path: &Path, freq: f64, seconds: f64) {
    let total = (OPUS_RATE as f64 * seconds) as usize;
    let mut pcm: Vec<i16> = (0..total)
        .map(|i| {
            let value = (2.0 * std::f64::consts::PI * freq * i as f64 / OPUS_RATE as f64).sin();
            (value * 0.5 * 32767.0) as i16
        })
        .collect();
    while pcm.len() % OPUS_FRAME != 0 {
        pcm.push(0);
    }

    let mut encoder =
        Encoder::new(OPUS_RATE, OpusChannels::Mono, Application::Voip).expect("创建 Opus 编码器");

    let file = File::create(path).expect("创建 OGG 文件");
    let mut writer = PacketWriter::new(BufWriter::new(file));
    write_opus_head(&mut writer, 1);
    write_opus_tags(&mut writer);

    let frames = pcm.len() / OPUS_FRAME;
    let mut encoded = vec![0u8; 4_000];
    let mut granule: u64 = 0;
    for index in 0..frames {
        let n = encoder
            .encode(
                &pcm[index * OPUS_FRAME..(index + 1) * OPUS_FRAME],
                &mut encoded,
            )
            .expect("Opus 编码");
        granule += OPUS_FRAME as u64;
        let end = if index + 1 == frames {
            PacketWriteEndInfo::EndStream
        } else {
            PacketWriteEndInfo::NormalPacket
        };
        writer
            .write_packet(encoded[..n].to_vec(), OPUS_SERIAL, end, granule)
            .expect("写 Opus 音频包");
    }
}

/// 只写头部 + 一个占位音频包的多声道 OGG/Opus。
///
/// 声道数检查发生在解码之前，所以不需要真实的 6 声道音频数据。
fn write_multichannel_ogg_opus(path: &Path, channels: u8) {
    let file = File::create(path).expect("创建 OGG 文件");
    let mut writer = PacketWriter::new(BufWriter::new(file));
    write_opus_head(&mut writer, channels);
    write_opus_tags(&mut writer);
    // 一个 TOC 字节 = 1 帧的静音包，够让探测拿到音频轨。
    writer
        .write_packet(
            vec![0u8; 3],
            OPUS_SERIAL,
            PacketWriteEndInfo::EndStream,
            OPUS_FRAME as u64,
        )
        .expect("写占位音频包");
}

/// 用零交叉数估算频率（跳过首尾各 200 样本，避开编解码器边缘效应）。
fn estimate_frequency(samples: &[i16], rate: u32) -> f64 {
    let body = &samples[200..samples.len() - 200];
    let crossings = body
        .windows(2)
        .filter(|pair| (pair[0] < 0) != (pair[1] < 0))
        .count();
    crossings as f64 / 2.0 / (body.len() as f64 / f64::from(rate))
}

/// OGG/Opus（symphonia 没有 Opus 解码器，走 libopus）：能解出正确的 PCM。
#[test]
fn decodes_opus_in_ogg() {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let src = dir.path().join("source.ogg");
    write_ogg_opus(&src, 440.0, 1.0);

    let dest = dir.path().join("out.wav");
    transcode_to_wav_16bit(&src, &dest).expect("OGG/Opus 应能转码");

    let (spec, samples) = read_wav(&dest);
    assert_eq!(spec.channels, 1);
    // Opus 的编码层固定 48 kHz，输出保持这个采样率（sherpa 侧自己重采样）。
    assert_eq!(spec.sample_rate, OPUS_RATE);
    assert_eq!(spec.bits_per_sample, 16);
    // 50 个 960 帧的包 = 48000 帧，减掉 OpusHead 里声明的 pre-skip 312。
    assert_eq!(
        samples.len(),
        (OPUS_RATE - 312) as usize,
        "pre-skip 应按 OpusHead 的值裁掉"
    );
    assert!(
        samples.iter().map(|s| s.abs()).max().unwrap() > 10_000,
        "解出来不该是静音"
    );
    assert!(
        (estimate_frequency(&samples, OPUS_RATE) - 440.0).abs() < 5.0,
        "频率应约等于 440 Hz，实际 {:.1}",
        estimate_frequency(&samples, OPUS_RATE)
    );
}

/// 多声道 Opus（channel mapping family 1）：报专门的错误码，而不是笼统的「不支持」。
#[test]
fn rejects_multichannel_opus() {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let src = dir.path().join("source.ogg");
    write_multichannel_ogg_opus(&src, 6);

    let dest = dir.path().join("out.wav");
    let err = transcode_to_wav_16bit(&src, &dest).expect_err("多声道 Opus 应被拒");
    assert!(
        matches!(err, TranscodeError::OpusMultichannel(6)),
        "应报多声道 Opus，实际: {err:?}"
    );
    assert!(!dest.exists(), "失败时不应留下半成品");
}
