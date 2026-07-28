//! 内置 TTS 的 PCM/WAV Rust 封装。

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const WAV_HEADER_LEN: usize = 44;

fn wav_header(sample_rate: u32, pcm_bytes: u32) -> [u8; WAV_HEADER_LEN] {
    let mut header = [0u8; WAV_HEADER_LEN];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&(36u32.saturating_add(pcm_bytes)).to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    header[22..24].copy_from_slice(&1u16.to_le_bytes()); // 单声道
    header[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&(sample_rate.saturating_mul(2)).to_le_bytes());
    header[32..34].copy_from_slice(&2u16.to_le_bytes());
    header[34..36].copy_from_slice(&16u16.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&pcm_bytes.to_le_bytes());
    header
}

pub fn encode_wav_pcm16(sample_rate: u32, pcm: &[u8]) -> Result<Vec<u8>, String> {
    if sample_rate == 0 {
        return Err("采样率不能为 0".into());
    }
    if pcm.is_empty() || pcm.len() % 2 != 0 {
        return Err("PCM_16 数据为空或字节数不是 2 的倍数".into());
    }
    let pcm_len = u32::try_from(pcm.len()).map_err(|_| "PCM 数据超过 WAV 4 GiB 上限")?;
    let mut wav = Vec::with_capacity(WAV_HEADER_LEN + pcm.len());
    wav.extend_from_slice(&wav_header(sample_rate, pcm_len));
    wav.extend_from_slice(pcm);
    Ok(wav)
}

/// 把 Python 逐段返回的 PCM 直接写入临时 WAV，完成后原子替换最终文件。
pub struct StreamingWavWriter {
    final_path: PathBuf,
    temp_path: PathBuf,
    file: Option<std::fs::File>,
    sample_rate: Option<u32>,
    pcm_bytes: u64,
    committed: bool,
}

impl StreamingWavWriter {
    pub fn create(final_path: &Path) -> Result<Self, String> {
        let parent = final_path
            .parent()
            .ok_or_else(|| "WAV 输出路径没有父目录".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建 WAV 输出目录失败: {error}"))?;
        let file_name = final_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("voice.wav");
        let temp_path = parent.join(format!(".{file_name}.{}.part", uuid::Uuid::new_v4()));
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|error| format!("创建临时 WAV 失败: {error}"))?;
        file.write_all(&[0u8; WAV_HEADER_LEN])
            .map_err(|error| format!("写入 WAV 占位头失败: {error}"))?;
        Ok(Self {
            final_path: final_path.to_path_buf(),
            temp_path,
            file: Some(file),
            sample_rate: None,
            pcm_bytes: 0,
            committed: false,
        })
    }

    pub fn push(&mut self, sample_rate: u32, pcm: &[u8]) -> Result<(), String> {
        if sample_rate == 0 {
            return Err("流式 PCM 采样率不能为 0".into());
        }
        if pcm.is_empty() || pcm.len() % 2 != 0 {
            return Err("流式 PCM 分块为空或长度无效".into());
        }
        match self.sample_rate {
            Some(current) if current != sample_rate => {
                return Err(format!(
                    "流式 PCM 采样率发生变化: {current} -> {sample_rate}"
                ))
            }
            None => self.sample_rate = Some(sample_rate),
            _ => {}
        }
        self.file
            .as_mut()
            .ok_or_else(|| "WAV 写入器已经关闭".to_string())?
            .write_all(pcm)
            .map_err(|error| format!("写入流式 PCM 失败: {error}"))?;
        self.pcm_bytes = self
            .pcm_bytes
            .checked_add(pcm.len() as u64)
            .ok_or_else(|| "WAV 长度溢出".to_string())?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), String> {
        let sample_rate = self
            .sample_rate
            .ok_or_else(|| "没有收到任何 PCM 分块".to_string())?;
        let pcm_bytes = u32::try_from(self.pcm_bytes).map_err(|_| "PCM 数据超过 WAV 4 GiB 上限")?;
        let mut file = self
            .file
            .take()
            .ok_or_else(|| "WAV 写入器已经关闭".to_string())?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("定位 WAV 头失败: {error}"))?;
        file.write_all(&wav_header(sample_rate, pcm_bytes))
            .map_err(|error| format!("写入最终 WAV 头失败: {error}"))?;
        file.flush()
            .map_err(|error| format!("刷新 WAV 文件失败: {error}"))?;
        drop(file);
        if self.final_path.exists() {
            std::fs::remove_file(&self.final_path)
                .map_err(|error| format!("替换旧 WAV 失败: {error}"))?;
        }
        std::fs::rename(&self.temp_path, &self.final_path)
            .map_err(|error| format!("提交 WAV 文件失败: {error}"))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for StreamingWavWriter {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.file.take();
            let _ = std::fs::remove_file(&self.temp_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_standard_pcm16_wav() {
        let wav = encode_wav_pcm16(22_050, &[0, 0, 1, 0]).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 22_050);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 4);
    }

    #[test]
    fn streams_pcm_to_atomic_wav_file() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("stream.wav");
        let mut writer = StreamingWavWriter::create(&output).unwrap();
        writer.push(22_050, &[0, 0, 1, 0]).unwrap();
        writer.push(22_050, &[2, 0, 3, 0]).unwrap();
        writer.finish().unwrap();

        let wav = std::fs::read(&output).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert!(!std::fs::read_dir(temp.path())
            .unwrap()
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".part")));
    }
}
