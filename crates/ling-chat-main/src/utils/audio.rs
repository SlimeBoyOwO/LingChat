//! 音频解码 / 转码通用工具。
//!
//! 目前唯一的用途是把用户导入的「零样本克隆参考音频」统一转成 WAV：
//! sherpa-onnx 只吃 WAV，而 [`crate::ai_service::tts::adapters::sherpa_onnx::load_reference_audio`]
//! 还要求 16/32 位 PCM（它是手工解析 RIFF 头的）。要求用户自己先转格式体验太差，
//! 这里用 symphonia 解码、hound 重编码，一次吸收掉容器 / 编码 / 位深 / float 的差异。

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// 参考音频时长上限（秒）。
///
/// 零样本克隆的参考音频通常几秒到十几秒，这个上限只是防止「误选一整首歌」把
/// 几十 MB 的 PCM 落进应用数据目录（平板的数据分区本来就吃紧）。
pub const MAX_REFERENCE_AUDIO_SECONDS: u64 = 120;

/// 音频转码错误。
#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    /// 容器 / 编码识别不了或当前不支持（上层给前端「格式不支持」提示）。
    #[error("不支持的音频格式: {0}")]
    Unsupported(String),
    /// 超过 [`MAX_REFERENCE_AUDIO_SECONDS`]。
    #[error("音频过长: {seconds:.0} 秒 (上限 {limit} 秒)")]
    TooLong { seconds: f64, limit: u64 },
    /// 文件内容损坏、IO 失败、写盘失败等。
    #[error("{0}")]
    Other(String),
}

/// 把音频转成 16-bit PCM WAV。
///
/// 边解边写（不把整段 PCM 攒在内存里），声道数与采样率保持源文件原样：
/// `load_reference_audio` 接受任意采样率，并由它自己下混成单声道。
///
/// 失败时删除写了一半的 `dest`，不在数据目录里留半成品。
pub fn transcode_to_wav_16bit(src: &Path, dest: &Path) -> Result<(), TranscodeError> {
    let result = transcode_inner(src, dest);
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

fn transcode_inner(src: &Path, dest: &Path) -> Result<(), TranscodeError> {
    let file = File::open(src).map_err(|e| TranscodeError::Other(format!("打开音频失败: {e}")))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // 扩展名作为探测提示：Android 上 SAF 暂存文件名仍带原扩展名。
    let mut hint = Hint::new();
    if let Some(ext) = src.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| TranscodeError::Unsupported(e.to_string()))?;
    let mut format = probed.format;

    // 取第一条音频轨，而不是 `default_track()`：带视频的 mp4（用户想抽里面的
    // 音频）和带封面的 m4a，默认轨都可能是视频/图片轨，交给解码器只会报
    // 「不支持的编码」。有采样率就是音频轨（视频轨没有）。
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.sample_rate.is_some())
        .ok_or_else(|| TranscodeError::Unsupported("文件中没有音频轨道".into()))?
        .clone();
    let track_id = track.id;

    // 能提前算出总时长就提前拒（wav/flac/带 Xing 头的 mp3 都给了总帧数），
    // 免得白解码一整首歌才发现超长。
    if let (Some(frames), Some(rate)) =
        (track.codec_params.n_frames, track.codec_params.sample_rate)
    {
        if rate > 0 && frames / u64::from(rate) > MAX_REFERENCE_AUDIO_SECONDS {
            return Err(TranscodeError::TooLong {
                seconds: frames as f64 / f64::from(rate),
                limit: MAX_REFERENCE_AUDIO_SECONDS,
            });
        }
    }

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| TranscodeError::Unsupported(e.to_string()))?;

    let mut writer: Option<hound::WavWriter<BufWriter<File>>> = None;
    let mut samples: Option<SampleBuffer<i16>> = None;
    let mut written_frames: u64 = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // symphonia 用 UnexpectedEof 表示流正常读完。
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            },
            Err(e) => return Err(TranscodeError::Other(format!("读取音频数据失败: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // 单个包坏掉（脏帧）跳过，不因此中断整段音频。
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            },
            Err(e) => return Err(TranscodeError::Other(format!("解码音频失败: {e}"))),
        };

        let spec = *decoded.spec();
        let channels = spec.channels.count() as u16;

        if let Some(existing) = writer.as_ref() {
            // 中途换规格（级联流）会让已写入的头部与实际数据错位，直接拒绝。
            let written = existing.spec();
            if written.channels != channels || written.sample_rate != spec.rate {
                return Err(TranscodeError::Other("音频规格中途变化，无法转换".into()));
            }
        } else {
            writer = Some(
                hound::WavWriter::create(
                    dest,
                    hound::WavSpec {
                        channels,
                        sample_rate: spec.rate,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .map_err(|e| TranscodeError::Other(format!("创建 WAV 失败: {e}")))?,
            );
        }
        let writer = writer.as_mut().expect("writer 已在上面就绪");

        // 复用同一块 i16 缓冲；copy_interleaved_ref 顺带做 f32 → i16 转换。
        let buffer = samples.get_or_insert_with(|| {
            let duration = decoded.capacity() as u64;
            SampleBuffer::<i16>::new(duration, spec)
        });
        buffer.copy_interleaved_ref(decoded);

        written_frames += (buffer.samples().len() / usize::from(channels.max(1))) as u64;
        if written_frames / u64::from(spec.rate) > MAX_REFERENCE_AUDIO_SECONDS {
            return Err(TranscodeError::TooLong {
                seconds: written_frames as f64 / f64::from(spec.rate),
                limit: MAX_REFERENCE_AUDIO_SECONDS,
            });
        }

        for &sample in buffer.samples() {
            writer
                .write_sample(sample)
                .map_err(|e| TranscodeError::Other(format!("写入 WAV 失败: {e}")))?;
        }
    }

    let writer = writer.ok_or_else(|| TranscodeError::Other("文件中没有可解码的音频".into()))?;
    writer
        .finalize()
        .map_err(|e| TranscodeError::Other(format!("写入 WAV 失败: {e}")))?;
    Ok(())
}
