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

/// 立体声下混成单声道，帧数不变：`load_reference_audio` 反正会自己下混，
/// 这里提前做掉能让落盘的参考音频小一半（平板上数据分区吃紧）。
#[test]
fn downmixes_stereo_to_mono() {
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
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 48_000);
    // 6 个交错样本 = 3 帧，每帧两个声道取平均。
    assert_eq!(samples.len(), 3);
    assert_eq!(samples, vec![0, 0, 0]);
}

/// 左右声道不同的立体声：下混取算术平均（与 `load_reference_audio` 的做法一致）。
#[test]
fn downmix_averages_channels() {
    let (dir, src) = temp_pair();
    // 第 1 帧：(1000 + 3000)/2 = 2000；第 2 帧：(-1000 + -4000)/2 = -2500
    let input = [1_000i16, 3_000, -1_000, -4_000];
    write_wav(
        &src,
        WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        },
        &input,
    );

    let dest = dir.path().join("out.wav");
    transcode_to_wav_16bit(&src, &dest).expect("立体声 WAV 应能转码");

    let (spec, samples) = read_wav(&dest);
    assert_eq!(spec.channels, 1);
    assert_eq!(samples, vec![2_000, -2_500]);
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

/// ADTS 裸流里的一个 AAC-LC 静音帧：7 字节 ADTS 头 + 4 字节帧数据。
///
/// AAC 帧没法像 WAV 那样用代码合成，这里内嵌 ffmpeg 生成的静音帧
/// （`ffmpeg -f lavfi -i anullsrc=r=44100:cl=mono -c:a aac -f adts` 的第 2 帧；
/// 第 1 帧里塞了 ffmpeg 的版本字符串）。44.1 kHz 单声道的静音帧只有 4 字节，
/// 正好适合内嵌；ADTS 每帧独立，重复写就能凑出时长。
const AAC_LC_SILENCE_FRAME: [u8; 11] = [
    0xFF, 0xF1, 0x50, 0x40, 0x01, 0x7F, 0xFC, 0x01, 0x18, 0x20, 0x07,
];

/// 裸 `.aac`（ADTS）能导入。
///
/// ADTS 读取器藏在 symphonia 的 `aac` feature 里（`symphonia-codec-aac::AdtsReader`），
/// 光看 `symphonia-format-*` 的名单看不出来 —— 曾经因此误判 `.aac` 不支持、把它挡在
/// 导入对话框外。另外内容其实是 mp4/m4a、只是扩展名叫 `.aac` 的文件也能用（探测以
/// 内容为准）。HE-AAC（SBR/PS）同样能转码：解码器只认 AAC-LC，会丢掉 SBR 层、只留
/// 11 kHz 以下的 LC 核心（实机验证音量与时长正常），因此不算「格式不支持」。
#[test]
fn accepts_raw_aac_adts() {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let src = dir.path().join("source.aac");
    let mut data = Vec::new();
    for _ in 0..25 {
        data.extend_from_slice(&AAC_LC_SILENCE_FRAME);
    }
    std::fs::write(&src, &data).expect("写 ADTS 文件");

    let dest = dir.path().join("out.wav");
    transcode_to_wav_16bit(&src, &dest).expect("ADTS 应能转码");

    let (spec, samples) = read_wav(&dest);
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 44_100);
    // 一个 ADTS 帧 = 1024 个样本
    assert_eq!(samples.len(), 25 * 1024);
    assert!(
        samples.iter().all(|s| s.abs() < 64),
        "静音帧应解出接近 0 的样本"
    );
}
