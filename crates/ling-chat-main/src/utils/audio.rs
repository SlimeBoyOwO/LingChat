//! 音频解码 / 转码通用工具。
//!
//! 目前唯一的用途是把用户导入的「零样本克隆参考音频」统一转成 WAV：
//! sherpa-onnx 只吃 WAV，而 [`crate::ai_service::tts::adapters::sherpa_onnx::load_reference_audio`]
//! 还要求 16/32 位 PCM（它是手工解析 RIFF 头的）。要求用户自己先转格式体验太差，
//! 这里用 symphonia 解码、hound 重编码，一次吸收掉容器 / 编码 / 位深 / float /
//! 多声道（输出单声道，下游本来就会下混）的差异。
//!
//! Opus 是唯一例外：symphonia 0.5 没有 Opus 解码器，而 `.ogg` / `.opus` 里塞
//! Opus 极常见（Telegram / WhatsApp 的语音消息就是）。容器解析（OpusHead 里的
//! pre-skip、声道数）仍交给 symphonia，音频包交给 libopus，见 [`transcode_opus`]。

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use opus::{Channels as OpusChannels, Decoder as OpusDecoder};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_OPUS, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, Track};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// 参考音频时长上限（秒）。
///
/// 零样本克隆的参考音频通常几秒到十几秒，这个上限只是防止「误选一整首歌」把
/// 几十 MB 的 PCM 落进应用数据目录（平板的数据分区本来就吃紧）。
pub const MAX_REFERENCE_AUDIO_SECONDS: u64 = 120;

/// Opus 解码采样率。
///
/// Opus 的编码层固定按 48 kHz 工作（OpusHead 里的原始采样率只是元数据），libopus
/// 也按这个值输出 PCM。参考音频保持 48 kHz 而不是降到 16 kHz：
/// sherpa-onnx 会用 `reference_sample_rate` 把它重采样到模型采样率
/// （zipvoice 是 24 kHz，见 `offline-tts-zipvoice-impl.h` 的
/// `ComputeMelSpectrogram`），而它的 mel 滤波器上限是模型采样率的 1/2（12 kHz）——
/// 若这里按 16 kHz 解码，重采样时低通截止会掉到 ~8 kHz，8–12 kHz 那一段直接没了，
/// 克隆音色会发闷。省下来的磁盘在该路径上另有办法（见下方单声道下混）。
const OPUS_DECODE_RATE: u32 = 48_000;

/// Opus 单个包的帧数上限：120 ms @48 kHz（RFC 6716 允许的最大帧长）。
const OPUS_MAX_FRAME_SAMPLES: usize = 5_760;

/// 音频转码错误。
#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    /// 容器 / 编码识别不了或当前不支持（上层给前端「格式不支持」提示）。
    #[error("不支持的音频格式: {0}")]
    Unsupported(String),
    /// 多声道 Opus（channel mapping family 1 及以上，例如 5.1）。
    ///
    /// `opus::Decoder` 只提供单/双声道解码（多声道要 `opus_multistream_decoder`），
    /// 而语音参考音频用不到多声道，单独成一档好给前端一句能自查的提示。
    #[error("多声道 Opus 暂不支持（{0} 声道）")]
    OpusMultichannel(usize),
    /// 超过 [`MAX_REFERENCE_AUDIO_SECONDS`]。
    #[error("音频过长: {seconds:.0} 秒 (上限 {limit} 秒)")]
    TooLong { seconds: f64, limit: u64 },
    /// 文件内容损坏、IO 失败、写盘失败等。
    #[error("{0}")]
    Other(String),
}

/// 把音频转成 16-bit PCM 单声道 WAV。
///
/// 边解边写（不把整段 PCM 攒在内存里），采样率保持源文件原样：
/// `load_reference_audio` 接受任意采样率，并由它自己交给 sherpa 重采样。
/// 声道固定单声道——参考音频的下游（`load_reference_audio`）无论如何都会把它
/// 平均成单声道，立体声原样落盘只会白占一倍磁盘（平板上数据分区本就吃紧）。
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

    // Opus 交给 libopus：symphonia 只认容器不认编码（`make()` 只会回一句
    // unsupported codec），而 `.ogg` / `.opus` 里塞 Opus 极常见。
    if track.codec_params.codec == CODEC_TYPE_OPUS {
        return transcode_opus(&mut *format, &track, dest);
    }

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| TranscodeError::Unsupported(e.to_string()))?;

    let mut sink = WavSink::new();
    let mut samples: Option<SampleBuffer<i16>> = None;

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

        // 复用同一块 i16 缓冲；copy_interleaved_ref 顺带做 f32 → i16 转换。
        let buffer = samples.get_or_insert_with(|| {
            let duration = decoded.capacity() as u64;
            SampleBuffer::<i16>::new(duration, spec)
        });
        buffer.copy_interleaved_ref(decoded);

        sink.write(dest, channels, spec.rate, buffer.samples())?;
    }

    sink.finish()
}

/// 解码 OGG/Opus（`.ogg` / `.oga` / `.opus`）。
///
/// symphonia 负责拆包并告诉我们 OpusHead 里的声道数与 pre-skip，音频包本身交给
/// libopus（`symphonia::default::get_codecs()` 里没有 Opus 解码器）。
///
/// 输出固定 48 kHz：Opus 的编码层就是按 48 kHz 工作的，libopus 也按这个采样率
/// 出 PCM（`opus::Decoder::new` 也支持 16 kHz 等值，但那属于额外重采样，交给
/// sherpa 按模型需要做更合适）。
fn transcode_opus(
    format: &mut dyn FormatReader,
    track: &Track,
    dest: &Path,
) -> Result<(), TranscodeError> {
    let channel_count = track.codec_params.channels.map(|c| c.count()).unwrap_or(0);

    // `opus::Decoder` 只有单/双声道两种（多声道要 opus_multistream_decoder，
    // 这个 crate 没包）。语音参考音频用不到多声道，直接给一句能自查的提示。
    if !(1..=2).contains(&channel_count) {
        return Err(TranscodeError::OpusMultichannel(channel_count));
    }
    let channels = channel_count as u16;
    let opus_channels = if channel_count == 1 {
        OpusChannels::Mono
    } else {
        OpusChannels::Stereo
    };

    // pre-skip 是编码器的前导延迟，按 Opus 规范必须丢掉开头这些样本，
    // 否则解出来的音频整体后移（symphonia 已从 OpusHead 读出来放在 delay 上）。
    let mut skip_left = track.codec_params.delay.unwrap_or(0) as usize;

    let mut decoder = OpusDecoder::new(OPUS_DECODE_RATE, opus_channels)
        .map_err(|e| TranscodeError::Other(format!("初始化 Opus 解码器失败: {e}")))?;

    let mut sink = WavSink::new();
    let mut pcm = vec![0i16; OPUS_MAX_FRAME_SAMPLES * usize::from(channels)];

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            },
            Err(e) => return Err(TranscodeError::Other(format!("读取音频数据失败: {e}"))),
        };
        if packet.track_id() != track.id {
            continue;
        }

        // 与 symphonia 那条路径一致：单个包坏掉（脏帧）跳过，不中断整段音频。
        let Ok(decoded_frames) = decoder.decode(&packet.data, &mut pcm, false) else {
            continue;
        };
        if decoded_frames == 0 {
            continue;
        }

        let mut start = 0;
        if skip_left > 0 {
            let drop_frames = skip_left.min(decoded_frames);
            start = drop_frames * usize::from(channels);
            skip_left -= drop_frames;
        }
        let end = decoded_frames * usize::from(channels);

        sink.write(dest, channels, OPUS_DECODE_RATE, &pcm[start..end])?;
    }

    sink.finish()
}

/// 边解码边写 WAV 的落盘器。
///
/// symphonia 与 libopus 两条解码路径都把 PCM 交给它，统一处理首次定规格、
/// 中途换规格（级联流）、时长上限、多声道下混与写样本，避免两套重复的写入逻辑。
struct WavSink {
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    written_frames: u64,
    /// 首批样本的源声道数（输出固定单声道，所以单独记一份用于检测规格变化）。
    source_channels: Option<u16>,
}

impl WavSink {
    fn new() -> Self {
        Self {
            writer: None,
            written_frames: 0,
            source_channels: None,
        }
    }

    /// 写一批交错 i16 样本；首次调用时按 `rate` 创建输出文件（单声道）。
    ///
    /// 源是立体声等多声道时在这里就下混：`load_reference_audio` 反正会平均成
    /// 单声道，原样落盘只是白占磁盘。
    fn write(
        &mut self,
        dest: &Path,
        channels: u16,
        rate: u32,
        samples: &[i16],
    ) -> Result<(), TranscodeError> {
        if samples.is_empty() {
            return Ok(());
        }
        let source_channels = channels.max(1);

        if let Some(existing) = self.writer.as_ref() {
            // 中途换规格（级联流）会让已写入的头部与实际数据错位，直接拒绝。
            if self.source_channels != Some(source_channels) || existing.spec().sample_rate != rate
            {
                return Err(TranscodeError::Other("音频规格中途变化，无法转换".into()));
            }
        } else {
            self.source_channels = Some(source_channels);
            self.writer = Some(
                hound::WavWriter::create(
                    dest,
                    hound::WavSpec {
                        channels: 1,
                        sample_rate: rate,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .map_err(|e| TranscodeError::Other(format!("创建 WAV 失败: {e}")))?,
            );
        }

        let frames = (samples.len() / usize::from(source_channels)) as u64;
        let total_frames = self.written_frames + frames;
        if total_frames / u64::from(rate) > MAX_REFERENCE_AUDIO_SECONDS {
            return Err(TranscodeError::TooLong {
                seconds: total_frames as f64 / f64::from(rate),
                limit: MAX_REFERENCE_AUDIO_SECONDS,
            });
        }
        self.written_frames = total_frames;

        let writer = self.writer.as_mut().expect("writer 已在上面就绪");
        if source_channels == 1 {
            for &sample in samples {
                writer
                    .write_sample(sample)
                    .map_err(|e| TranscodeError::Other(format!("写入 WAV 失败: {e}")))?;
            }
            return Ok(());
        }

        let divisor = f32::from(source_channels);
        for frame in samples.chunks_exact(usize::from(source_channels)) {
            let sum: i32 = frame.iter().map(|&sample| i32::from(sample)).sum();
            let mixed = (sum as f32 / divisor).round() as i16;
            writer
                .write_sample(mixed)
                .map_err(|e| TranscodeError::Other(format!("写入 WAV 失败: {e}")))?;
        }
        Ok(())
    }

    /// 收尾；如果一个样本都没写过，说明整段音频没能解码。
    fn finish(self) -> Result<(), TranscodeError> {
        let writer = self
            .writer
            .ok_or_else(|| TranscodeError::Other("文件中没有可解码的音频".into()))?;
        writer
            .finalize()
            .map_err(|e| TranscodeError::Other(format!("写入 WAV 失败: {e}")))?;
        Ok(())
    }
}
