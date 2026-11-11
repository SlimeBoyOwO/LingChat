//! 音频解码 / 转码工具：把导入的参考音频统一转成 16-bit PCM WAV。
//! symphonia 解码、hound 重编码；Opus 交由 libopus。

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

/// 参考音频时长上限（秒），防止误选整首歌占满数据分区。
pub const MAX_REFERENCE_AUDIO_SECONDS: u64 = 120;

/// Opus 解码采样率；保持 48 kHz，降到 16 kHz 会丢失高频让克隆音色发闷。
const OPUS_DECODE_RATE: u32 = 48_000;

/// Opus 单个包的帧数上限：120 ms @48 kHz（RFC 6716 允许的最大帧长）。
const OPUS_MAX_FRAME_SAMPLES: usize = 5_760;

/// 音频转码错误。
#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    /// 容器 / 编码识别不了或当前不支持（上层给前端「格式不支持」提示）。
    #[error("不支持的音频格式: {0}")]
    Unsupported(String),
    /// 多声道 Opus（`opus::Decoder` 只支持单/双声道）。
    #[error("多声道 Opus 暂不支持（{0} 声道）")]
    OpusMultichannel(usize),
    /// 超过 [`MAX_REFERENCE_AUDIO_SECONDS`]。
    #[error("音频过长: {seconds:.0} 秒 (上限 {limit} 秒)")]
    TooLong { seconds: f64, limit: u64 },
    /// 文件内容损坏、IO 失败、写盘失败等。
    #[error("{0}")]
    Other(String),
}

/// 把音频转成 16-bit PCM 单声道 WAV；采样率保持原样，失败时删除写了一半的 `dest`。
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

    // 取第一条有采样率的音频轨：mp4/m4a 的默认轨可能是视频/封面。
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.sample_rate.is_some())
        .ok_or_else(|| TranscodeError::Unsupported("文件中没有音频轨道".into()))?
        .clone();
    let track_id = track.id;

    // 能算出总时长就提前拒，免得白解码整首歌。
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

    // Opus 交给 libopus；symphonia 不会解码 Opus。
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

        // 复用 i16 缓冲；copy_interleaved_ref 顺带做 f32 → i16。
        let buffer = samples.get_or_insert_with(|| {
            let duration = decoded.capacity() as u64;
            SampleBuffer::<i16>::new(duration, spec)
        });
        buffer.copy_interleaved_ref(decoded);

        sink.write(dest, channels, spec.rate, buffer.samples())?;
    }

    sink.finish()
}

/// 解码 OGG/Opus；symphonia 拆包，音频包交给 libopus，输出固定 48 kHz。
fn transcode_opus(
    format: &mut dyn FormatReader,
    track: &Track,
    dest: &Path,
) -> Result<(), TranscodeError> {
    let channel_count = track.codec_params.channels.map(|c| c.count()).unwrap_or(0);

    // `opus::Decoder` 只有单/双声道；多声道直接报错。
    if !(1..=2).contains(&channel_count) {
        return Err(TranscodeError::OpusMultichannel(channel_count));
    }
    let channels = channel_count as u16;
    let opus_channels = if channel_count == 1 {
        OpusChannels::Mono
    } else {
        OpusChannels::Stereo
    };

    // pre-skip 是编码器前导延迟，必须丢掉开头这些样本，否则音频整体后移。
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

        // 脏帧跳过，不中断整段音频。
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

/// 边解码边写 WAV；统一处理规格、时长上限、多声道下混。
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
