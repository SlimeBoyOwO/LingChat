//! 角色级 `VoiceMaker`
//!
//! 职责：
//! - 根据 `VoiceModel` 配置检测每种 TTS 的可用性
//! - 基于当前 `tts_type` 初始化对应 adapter
//! - `generate_voice_files(segments)`：并发为每段生成音频到磁盘

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use futures_util::future::join_all;

use crate::ai_service::message_system::processor::EmotionSegment;
use crate::ai_service::tts::adapters::aivis::AivisAdapter;
use crate::ai_service::tts::adapters::bv2::Bv2Adapter;
use crate::ai_service::tts::adapters::gsv::GsvAdapter;
use crate::ai_service::tts::adapters::indextts::IndexTtsAdapter;
use crate::ai_service::tts::adapters::opentts::OpenTtsAdapter;
use crate::ai_service::tts::adapters::sbv2::Sbv2Adapter;
use crate::ai_service::tts::adapters::sbv2api::Sbv2ApiAdapter;
use crate::ai_service::tts::adapters::vits::VitsAdapter;
use crate::ai_service::tts::local::adapter::LocalTtsAdapter;
use crate::ai_service::tts::local::LocalTtsRuntime;
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
    pub opentts: bool,
    pub sbv2_local: bool,
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
    /// Local TTS 共享运行时（进程内引擎 + 路径 + 全局开关）。由
    /// `build_voice_maker` 注入一次，`sbv2_local` 适配器惰性引导时使用。
    local_runtime: Option<LocalTtsRuntime>,
    local_cloud_fallback: Option<LocalCloudFallback>,
}

#[derive(Clone, Debug)]
struct LocalCloudFallback {
    model_name: String,
    speaker_id: i32,
    adapter: Arc<OnceLock<Arc<Sbv2ApiAdapter>>>,
}

impl LocalCloudFallback {
    fn adapter(&self, api_url: &str) -> Arc<Sbv2ApiAdapter> {
        self.adapter
            .get_or_init(|| {
                Arc::new(Sbv2ApiAdapter::new(
                    api_url.to_string(),
                    self.model_name.clone(),
                    self.speaker_id,
                ))
            })
            .clone()
    }
}

fn non_empty(s: &Option<String>) -> bool {
    s.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false)
}

fn gsv_prompt_language(prompt_text: &str) -> &'static str {
    if prompt_text
        .chars()
        .any(|c| matches!(c, '\u{ac00}'..='\u{d7af}'))
    {
        "ko"
    } else if prompt_text.chars().any(|c| {
        matches!(
            c,
            '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'
        )
    }) {
        "ja"
    } else if prompt_text
        .chars()
        .any(|c| matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'))
    {
        "zh"
    } else if prompt_text.chars().any(|c| c.is_ascii_alphabetic()) {
        "en"
    } else {
        "zh"
    }
}

fn segment_text_for_lang<'a>(lang: &str, segment: &'a EmotionSegment) -> Option<&'a str> {
    match lang {
        "ja" | "en" | "ko" if !segment.japanese_text.trim().is_empty() => {
            Some(&segment.japanese_text)
        }
        "en" | "ko" => None,
        "zh" if !segment.following_text.trim().is_empty() => Some(&segment.following_text),
        _ if !segment.following_text.trim().is_empty() => Some(&segment.following_text),
        _ if !segment.japanese_text.trim().is_empty() => Some(&segment.japanese_text),
        _ => None,
    }
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
            local_runtime: None,
            local_cloud_fallback: None,
        }
    }

    /// 注入本地 TTS 共享运行时。由 `build_voice_maker` 在角色注册时调用，
    /// `sbv2_local` 适配器惰性引导及云端 fallback 判断都依赖它。
    pub fn set_local_runtime(&mut self, local_runtime: Option<LocalTtsRuntime>) {
        self.local_runtime = local_runtime;
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
        // OpenTTS 可用性：角色级 voice 优先，全局 TTS 配置兜底，任一非空即可用
        let opentts =
            non_empty(&cfg.opentts_voice) || !self.tts_config.opentts_voice.trim().is_empty();
        // Local SBV2 only needs a voice_id; engine readiness is checked later
        let sbv2_local = non_empty(&cfg.sbv2_local_voice_id);

        self.availability = TtsAvailability {
            sva,
            sbv2,
            bv2,
            sbv2api,
            gsv,
            aivis,
            opentts,
            sbv2_local,
        };
    }

    /// 按当前 `tts_type` 初始化对应 adapter。
    pub fn set_tts_settings(&mut self, cfg: &VoiceModel, tts_type: &str, name: &str) -> Result<()> {
        self.check_tts_availability(cfg);
        self.tts_type = tts_type.to_string();
        self.local_cloud_fallback = None;

        match tts_type {
            "sva-vits" if self.availability.sva => {
                if let Some(id) = cfg
                    .sva_speaker_id
                    .as_deref()
                    .and_then(|s| s.parse::<i32>().ok())
                {
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
            "localsbv2api" if self.availability.sbv2_local => {
                self.provider.sbv2api = None;
                self.local_cloud_fallback = match (
                    cfg.sbv2_local_cloud_fallback_model
                        .clone()
                        .filter(|v| !v.trim().is_empty()),
                    cfg.sbv2_local_cloud_fallback_speaker_id
                        .as_deref()
                        .and_then(|v| v.parse::<i32>().ok()),
                ) {
                    (Some(model_name), Some(speaker_id)) => Some(LocalCloudFallback {
                        model_name,
                        speaker_id,
                        adapter: Arc::new(OnceLock::new()),
                    }),
                    _ => None,
                };
                let runtime = match &self.local_runtime {
                    Some(runtime) => runtime.clone(),
                    None => {
                        tracing::warn!(
                            "sbv2_local 已选择但本地 TTS 运行时未注入；chat 路由将返回错误"
                        );
                        self.provider.disable();
                        return Ok(());
                    }
                };
                let engine = runtime.engine;
                let paths = runtime.paths;
                let voice_id = cfg.sbv2_local_voice_id.clone().unwrap_or_default();
                let speaker_id = cfg.sbv2_local_speaker_id.unwrap_or(0);
                let style_id = cfg.sbv2_local_style_id.unwrap_or(0);
                let length_scale = cfg.sbv2_local_length_scale.unwrap_or(1.0);
                let sdp_ratio = cfg.sbv2_local_sdp_ratio.unwrap_or(0.0);
                self.provider.sbv2_local = Some(Arc::new(LocalTtsAdapter::with_params(
                    engine,
                    voice_id,
                    style_id,
                    speaker_id,
                    sdp_ratio,
                    length_scale,
                    paths,
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
                let ref_audio_path = match (&self.character_path, &cfg.gsv_voice_filename) {
                    (Some(base), Some(name_)) if !name_.is_empty() => {
                        base.join("voice").join(name_).to_string_lossy().to_string()
                    }
                    _ => String::new(),
                };
                let prompt_text = cfg.gsv_voice_text.clone().unwrap_or_default();
                let prompt_lang = gsv_prompt_language(&prompt_text).to_string();
                let voice_lang = match self.lang.as_str() {
                    "zh" => "zh",
                    "ja" => "ja",
                    "en" => "en",
                    "ko" => "ko",
                    other => {
                        tracing::warn!("GPT-SoVITS 暂不支持语言 {other}，回退到中文");
                        "zh"
                    }
                }
                .to_string();
                let adapter = GsvAdapter::new(
                    self.tts_config.gsv_api_url.clone(),
                    ref_audio_path,
                    prompt_text,
                    prompt_lang,
                    voice_lang,
                    cfg.gsv_gpt_model_name.clone(),
                    cfg.gsv_sovits_model_name.clone(),
                );
                self.provider.gsv = Some(Arc::new(adapter));
                let _ = name;
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
                        tracing::warn!("AIVIS 初始化失败: {e}");
                        self.provider.disable();
                    }
                }
            }
            "opentts" if self.availability.opentts => {
                // 角色级 voice 优先；为空时回退到全局 TTS 配置的音色标识
                let voice = if non_empty(&cfg.opentts_voice) {
                    cfg.opentts_voice.clone().unwrap_or_default()
                } else {
                    self.tts_config.opentts_voice.clone()
                };
                let model = if self.tts_config.opentts_model.trim().is_empty() {
                    "FunAudioLLM/CosyVoice2-0.5B".to_string()
                } else {
                    self.tts_config.opentts_model.clone()
                };
                let api_url = if self.tts_config.opentts_api_url.trim().is_empty() {
                    "https://api.siliconflow.cn/v1".to_string()
                } else {
                    self.tts_config.opentts_api_url.clone()
                };
                let api_key = self.tts_config.opentts_api_key.clone().unwrap_or_default();
                if api_key.trim().is_empty() {
                    tracing::warn!("OpenTTS API 密钥未设置，禁用 TTS");
                    self.provider.disable();
                } else {
                    match OpenTtsAdapter::new(
                        api_url,
                        api_key,
                        model,
                        voice,
                        self.audio_format.clone(),
                        self.lang.clone(),
                    ) {
                        Ok(a) => self.provider.opentts = Some(Arc::new(a)),
                        Err(e) => {
                            tracing::warn!("OpenTTS 初始化失败: {e}");
                            self.provider.disable();
                        }
                    }
                }
            }
            "indextts2" => {
                self.provider.indextts = Some(Arc::new(IndexTtsAdapter::new(
                    self.tts_config.indextts_api_url.clone(),
                )));
            }
            _ => {
                tracing::warn!("TTS 类型不可用或未初始化: {tts_type}");
            }
        }

        Ok(())
    }

    /// 更新语言并重新初始化当前 TTS adapter。
    pub fn update_lang_and_refresh(
        &mut self,
        cfg: &VoiceModel,
        tts_type: &str,
        name: &str,
        lang: impl Into<String>,
    ) {
        self.lang = lang.into();
        self.provider = TtsProvider::new(&self.audio_format);
        if let Err(e) = self.set_tts_settings(cfg, tts_type, name) {
            tracing::warn!("切换语音语言后重新初始化 TTS 失败: {e}");
        } else {
            tracing::info!("语音语言已切换为: {}, tts_type: {}", self.lang, tts_type);
        }
    }

    pub async fn generate_voice_files(&self, segments: &mut [EmotionSegment]) {
        if self.tts_type.is_empty() {
            return;
        }
        if !self.provider.is_enabled() {
            if let Some(text) = segments
                .iter()
                .find_map(|segment| segment_text_for_lang(&self.lang, segment))
            {
                self.provider.recover_in_background(
                    text.to_owned(),
                    self.tts_type.clone(),
                    String::new(),
                );
            }
            return;
        }
        tokio::fs::create_dir_all(&self.temp_dir).await.ok();

        let use_cloud_fallback = self.tts_type == "localsbv2api"
            && self
                .local_runtime
                .as_ref()
                .is_some_and(|runtime| !runtime.is_enabled());
        let fallback_adapter = if use_cloud_fallback {
            tracing::info!(
                "角色配置为 localsbv2api，但本地 TTS 已被全局禁用，改用独立配置的云端 fallback"
            );
            self.local_cloud_fallback
                .as_ref()
                .map(|fallback| fallback.adapter(&self.tts_config.sbv2api_api_url))
        } else {
            None
        };

        let mut futs = Vec::new();
        for seg in segments.iter_mut() {
            let Some(text) = segment_text_for_lang(&self.lang, seg).map(str::to_owned) else {
                continue;
            };
            let emo = String::new();

            let file_name = if seg.voice_file.is_empty() {
                format!(
                    "{}_part_{}.{}",
                    uuid::Uuid::new_v4(),
                    seg.index,
                    self.audio_format
                )
            } else {
                seg.voice_file.clone()
            };
            let file_path = self.temp_dir.join(&file_name);
            seg.voice_file = file_path.to_string_lossy().to_string();

            let mut provider = self.provider.clone();
            let tts_type = if use_cloud_fallback {
                if let Some(adapter) = fallback_adapter.clone() {
                    provider.sbv2api = Some(adapter);
                } else {
                    tracing::warn!(
                        "本地 TTS 已禁用，但角色未配置完整的云端 fallback 模型与说话人 ID"
                    );
                }
                "sbv2api".to_string()
            } else {
                self.tts_type.clone()
            };
            let index = seg.index;
            futs.push(async move {
                if let Err(e) = provider
                    .generate_voice(&text, &file_path, &tts_type, &emo)
                    .await
                {
                    tracing::error!("片段 {index} 语音生成失败: {e}");
                }
            });
        }
        if !futs.is_empty() {
            join_all(futs).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::tts::local::engine::LocalTtsEngine;
    use crate::ai_service::tts::local::paths::LocalTtsPaths;
    use crate::ai_service::tts::local::LocalTtsSwitch;

    #[test]
    fn local_mode_defers_cloud_fallback_adapter_creation() {
        let mut maker = VoiceMaker::new(PathBuf::from("voice"), "wav", TtsConfig::default());
        maker.set_local_runtime(Some(LocalTtsRuntime::new(
            Arc::new(LocalTtsEngine::new()),
            LocalTtsPaths {
                root: PathBuf::from("tts-local"),
                assets: PathBuf::from("tts-local/assets"),
                voices: PathBuf::from("tts-local/voices"),
                cache: PathBuf::from("cache"),
            },
            LocalTtsSwitch::new(true),
        )));
        let cfg = VoiceModel {
            sbv2api_name: Some("ordinary-cloud-model".into()),
            sbv2api_speaker_id: Some("99".into()),
            sbv2_local_voice_id: Some("local-voice".into()),
            sbv2_local_cloud_fallback_model: Some("fallback-model".into()),
            sbv2_local_cloud_fallback_speaker_id: Some("7".into()),
            ..VoiceModel::default()
        };

        maker
            .set_tts_settings(&cfg, "localsbv2api", "test")
            .unwrap();

        assert!(maker.provider.sbv2api.is_none());
        let fallback = maker.local_cloud_fallback.as_ref().unwrap();
        assert_eq!(fallback.model_name, "fallback-model");
        assert_eq!(fallback.speaker_id, 7);
        assert!(fallback.adapter.get().is_none());
    }
}
