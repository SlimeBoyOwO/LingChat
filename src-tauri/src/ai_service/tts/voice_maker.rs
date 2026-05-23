//! 角色级 `VoiceMaker`，对应 Python `ling_chat/core/ai_service/voice_maker.py`。
//!
//! 职责：
//! - 根据 `VoiceModel` 配置检测每种 TTS 的可用性
//! - 基于当前 `tts_type` 初始化对应 adapter
//! - `generate_voice_files(segments)`：并发为每段生成音频到磁盘

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use futures_util::future::join_all;

use crate::ai_service::message_system::processor::EmotionSegment;
use crate::ai_service::tts::adapters::aivis::AivisAdapter;
use crate::ai_service::tts::adapters::bv2::Bv2Adapter;
use crate::ai_service::tts::adapters::gsv::GsvAdapter;
use crate::ai_service::tts::adapters::indextts::IndexTtsAdapter;
use crate::ai_service::tts::adapters::sbv2::Sbv2Adapter;
use crate::ai_service::tts::adapters::sbv2api::Sbv2ApiAdapter;
use crate::ai_service::tts::adapters::vits::VitsAdapter;
use crate::ai_service::tts::provider::TtsProvider;
use crate::ai_service::types::VoiceModel;
use crate::config::tts::TtsConfig;

/// 各 TTS 后端的可用性标志。
#[derive(Debug, Default, Clone, Copy)]
pub struct TtsAvailability {
    pub sva: bool,
    pub sbv2: bool,
    pub bv2: bool,
    pub sbv2api: bool,
    pub gsv: bool,
    pub aivis: bool,
}

#[derive(Clone, Debug)]
pub struct VoiceMaker {
    provider: TtsProvider,
    tts_type: String,
    lang: String,
    character_path: Option<PathBuf>,
    temp_dir: PathBuf,
    audio_format: String,
    availability: TtsAvailability,
    tts_config: TtsConfig,
}

fn non_empty(s: &Option<String>) -> bool {
    s.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false)
}

impl VoiceMaker {
    pub fn new(temp_dir: PathBuf, audio_format: impl Into<String>, tts_config: TtsConfig) -> Self {
        let audio_format = audio_format.into();
        let provider = TtsProvider::new(&audio_format);
        Self {
            provider,
            tts_type: String::new(),
            lang: "ja".into(),
            character_path: None,
            temp_dir,
            audio_format,
            availability: TtsAvailability::default(),
            tts_config,
        }
    }

    pub fn set_lang(&mut self, lang: impl Into<String>) {
        self.lang = lang.into();
    }

    pub fn set_character_path(&mut self, path: Option<PathBuf>) {
        self.character_path = path;
    }

    pub fn tts_type(&self) -> &str {
        &self.tts_type
    }

    pub fn availability(&self) -> TtsAvailability {
        self.availability
    }

    pub fn is_enabled(&self) -> bool {
        self.provider.is_enabled() && !self.tts_type.is_empty()
    }

    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    pub fn audio_format(&self) -> &str {
        &self.audio_format
    }

    pub fn reactivate(&self) {
        self.provider.reactivate();
    }

    /// 检查 TTS 配置中各后端的可用性（对应 `check_tts_availability`）。
    pub fn check_tts_availability(&mut self, cfg: &VoiceModel) {
        let sva = non_empty(&cfg.sva_speaker_id);
        let sbv2 = non_empty(&cfg.sbv2_speaker_id) && non_empty(&cfg.sbv2_name);
        let bv2 = non_empty(&cfg.bv2_speaker_id);
        let sbv2api = non_empty(&cfg.sbv2api_name) && non_empty(&cfg.sbv2api_speaker_id);
        let gsv = (non_empty(&cfg.gsv_voice_filename) && non_empty(&cfg.gsv_voice_text))
            || (non_empty(&cfg.gsv_gpt_model_name) && non_empty(&cfg.gsv_sovits_model_name));
        let aivis = non_empty(&cfg.aivis_model_uuid);

        self.availability = TtsAvailability {
            sva,
            sbv2,
            bv2,
            sbv2api,
            gsv,
            aivis,
        };
    }

    /// 按当前 `tts_type` 初始化对应 adapter。对应 Python `set_tts_settings`。
    ///
    /// `name` 用于 GSV 参考音频查找；其它类型可传空串。
    pub fn set_tts_settings(&mut self, cfg: &VoiceModel, tts_type: &str, name: &str) -> Result<()> {
        self.check_tts_availability(cfg);
        self.tts_type = tts_type.to_string();

        match tts_type {
            "sva-vits" if self.availability.sva => {
                if let Some(id) = cfg.sva_speaker_id.as_deref().and_then(|s| s.parse::<i32>().ok()) {
                    self.provider.sva = Some(Arc::new(VitsAdapter::new(
                        self.tts_config.simple_vits_api_url.clone(),
                        id,
                        self.audio_format.clone(),
                        "ja".into(),
                    )));
                }
            }
            "sbv2" if self.availability.sbv2 => {
                let id = cfg
                    .sbv2_speaker_id
                    .as_deref()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                let model_name = cfg.sbv2_name.clone().unwrap_or_default();
                self.provider.sbv2 = Some(Arc::new(Sbv2Adapter::new(
                    self.tts_config.sbv2_api_url.clone(),
                    id,
                    model_name,
                    self.audio_format.clone(),
                    &self.lang,
                )));
            }
            "sbv2api" if self.availability.sbv2api => {
                let id = cfg
                    .sbv2api_speaker_id
                    .as_deref()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                let model_name = cfg.sbv2api_name.clone().unwrap_or_default();
                self.provider.sbv2api = Some(Arc::new(Sbv2ApiAdapter::new(
                    self.tts_config.sbv2api_api_url.clone(),
                    model_name,
                    id,
                )));
            }
            "sva-bv2" if self.availability.bv2 => {
                let id = cfg
                    .bv2_speaker_id
                    .as_deref()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                self.provider.bv2 = Some(Arc::new(Bv2Adapter::new(
                    self.tts_config.bv2_api_url.clone(),
                    id,
                    self.audio_format.clone(),
                    self.lang.clone(),
                )));
            }
            "gsv" if self.availability.gsv => {
                // 参考音频：character_path/voice/<gsv_voice_filename>
                let ref_audio_path = match (&self.character_path, &cfg.gsv_voice_filename) {
                    (Some(base), Some(name_)) if !name_.is_empty() => base
                        .join("voice")
                        .join(name_)
                        .to_string_lossy()
                        .to_string(),
                    _ => String::new(),
                };
                let prompt_text = cfg.gsv_voice_text.clone().unwrap_or_default();
                let adapter = GsvAdapter::new(
                    self.tts_config.gsv_api_url.clone(),
                    ref_audio_path,
                    prompt_text,
                    "auto".into(),
                );
                self.provider.gsv = Some(Arc::new(adapter));
                let _ = name; // 预留：Python 版还有按 name 查找 gpt/sovits 权重的逻辑
            }
            "aivis" if self.availability.aivis => {
                let model_uuid = cfg.aivis_model_uuid.clone().unwrap_or_default();
                match AivisAdapter::new(
                    self.tts_config.aivis_api_url.clone(),
                    self.tts_config.aivis_api_key.clone(),
                    model_uuid,
                    None,
                    self.audio_format.clone(),
                    "ja".into(),
                ) {
                    Ok(a) => self.provider.aivis = Some(Arc::new(a)),
                    Err(e) => {
                        log::warn!("AIVIS 初始化失败: {e}");
                        self.provider.disable();
                    }
                }
            }
            "indextts2" => {
                self.provider.indextts = Some(Arc::new(IndexTtsAdapter::new(
                    self.tts_config.indextts_api_url.clone(),
                )));
            }
            _ => {
                log::warn!("TTS 类型不可用或未初始化: {tts_type}");
            }
        }

        Ok(())
    }

    /// 为一批 segment 生成语音文件；成功时把写入的绝对路径回填到 segment.voice_file。
    pub async fn generate_voice_files(&self, segments: &mut [EmotionSegment]) {
        if !self.is_enabled() {
            return;
        }
        tokio::fs::create_dir_all(&self.temp_dir).await.ok();

        let mut futs = Vec::new();
        for seg in segments.iter_mut() {
            let (text, emo) = match self.lang.as_str() {
                "ja" => {
                    if seg.japanese_text.trim().is_empty() {
                        if !seg.following_text.trim().is_empty() {
                            log::warn!("片段 {} 没有日语文本，跳过语音生成", seg.index);
                        }
                        continue;
                    }
                    (seg.japanese_text.clone(), String::new())
                }
                "zh" => {
                    if seg.following_text.trim().is_empty() {
                        log::warn!(
                            "片段 {} 没有中文文本，跳过语音生成 (检查 LLM 输出)",
                            seg.index
                        );
                        continue;
                    }
                    (seg.following_text.clone(), seg.predicted.clone())
                }
                _ => continue,
            };

            let file_name = if seg.voice_file.is_empty() {
                format!("{}_part_{}.{}", uuid::Uuid::new_v4(), seg.index, self.audio_format)
            } else {
                seg.voice_file.clone()
            };
            let file_path = self.temp_dir.join(&file_name);
            seg.voice_file = file_path.to_string_lossy().to_string();

            let provider = self.provider.clone();
            let tts_type = self.tts_type.clone();
            let index = seg.index;
            futs.push(async move {
                if let Err(e) = provider
                    .generate_voice(&text, &file_path, &tts_type, &emo)
                    .await
                {
                    log::error!("片段 {index} 语音生成失败: {e}");
                }
            });
        }
        if !futs.is_empty() {
            join_all(futs).await;
        }
    }
}
