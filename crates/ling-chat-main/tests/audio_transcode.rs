//! `utils::audio::transcode_to_wav_16bit` 的回归测试。
//!
//! 重点覆盖「以前会被 `load_reference_audio` 拒掉的输入」：24 位 / 浮点 WAV。
//! 后者是手工解析 RIFF 头的老实现读不了的（只认 16/32 位整型），而用户从
//! 音乐 App 或录音笔导出的参考音频经常就是这些格式。
//!
//! 测试素材用 hound 现场生成，不引入二进制 fixture。

use std::path::Path;

use hound::{SampleFormat, WavSpec, WavWriter};
use ling_chat_main::utils::audio::{
    MAX_REFERENCE_AUDIO_SECONDS, TranscodeError, transcode_to_wav_16bit,
};

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
