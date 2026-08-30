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
use crate::ai_service::tts::adapters::cosyvoice::CosyvoiceAdapter;
use crate::ai_service::tts::adapters::fish_s2::FishS2Adapter;
use crate::ai_service::tts::adapters::gsv::GsvAdapter;
use crate::ai_service::tts::adapters::indextts::IndexTtsAdapter;
use crate::ai_service::tts::adapters::opentts::OpenTtsAdapter;
use crate::ai_service::tts::adapters::sbv2::Sbv2Adapter;
use crate::ai_service::tts::adapters::sbv2api::Sbv2ApiAdapter;
use crate::ai_service::tts::adapters::vits::VitsAdapter;
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::tts::local::adapter::LocalTtsAdapter;
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
    pub fish_s2: bool,
    pub sbv2_local: bool,
    pub cosyvoice: bool,
}

#[derive(Clone, Debug)]
pub struct VoiceMaker {
    provider: TtsProvider,
    tts_type: String,
    lang: String,
    /// 中文方言（仅 cosyvoice + lang=zh 时生效；空 = 普通话）
    voice_dialect: Option<String>,
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

/// 优先朗读 `japanese_text` 译文的语音语言白名单。
///
/// 选文（`segment_text_for_lang`）与补生成语音（`api::chat::generate_line_voice`）
/// 共用此判断，避免两处各写一份列表导致口径漂移。日语也在名单内：主模型通常
/// 自带日语译文，无译文时由选文逻辑回退主台词。
pub(crate) fn lang_prefers_translation(lang: &str) -> bool {
    matches!(
        lang,
        "ja" | "en" | "ko" | "es" | "ar" | "de" | "fr" | "ru" | "pt"
    )
}

/// 返回当前语音语言实际送入 TTS 的台词文本。
///
/// `japanese_text` 是历史字段名，现用于保存 ja/en/ko/es/ar 等目标语言译文；
/// 中文与自动模式始终优先朗读主台词。消息生成器也复用本函数，确保界面显示
/// 的台词与 TTS 真正朗读的文本完全一致。
pub(crate) fn segment_text_for_lang<'a>(
    lang: &str,
    segment: &'a EmotionSegment,
) -> Option<&'a str> {
    match lang {
        // 译文统一存放在 japanese_text 字段（历史命名），白名单语言均优先取译文
        l if lang_prefers_translation(l) && !segment.japanese_text.trim().is_empty() => {
            Some(&segment.japanese_text)
        },
        // 无译文时除日语外不朗读错误语言；日语由下方 _ 臂回退主台词
        l if lang_prefers_translation(l) && l != "ja" => None,
        "zh" if !segment.following_text.trim().is_empty() => Some(&segment.following_text),
        _ if !segment.following_text.trim().is_empty() => Some(&segment.following_text),
        _ if !segment.japanese_text.trim().is_empty() => Some(&segment.japanese_text),
        _ => None,
    }
}

/// 生成保持原扩展名的临时路径（`foo.wav` → `foo.part.wav`），避免 adapter
/// 因扩展名改变而选择错误编码器；成功校验后再原子重命名到最终文件。
fn partial_audio_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let file_name = if extension.is_empty() {
        format!("{stem}.part")
    } else {
        format!("{stem}.part.{extension}")
    };
    path.with_file_name(file_name)
}

/// CosyVoice `language_hints` 官方支持范围（不含 es/ar，方言经 instruction 触发）。
fn cosyvoice_supported_language(lang: &str) -> bool {
    matches!(
        lang,
        "zh" | "en" | "ja" | "ko" | "fr" | "de" | "ru" | "pt" | "th" | "id" | "vi"
    )
}

impl VoiceMaker {
    pub fn new(temp_dir: PathBuf, audio_format: impl Into<String>, tts_config: TtsConfig) -> Self {
        let audio_format = audio_format.into();
        let provider = TtsProvider::new(&audio_format);
        Self {
            provider,
            tts_type: String::new(),
            lang: "ja".into(),
            voice_dialect: None,
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

    /// 设置中文方言（仅 cosyvoice + lang=zh 时生效；空 = 普通话）。
    pub fn set_voice_dialect(&mut self, dialect: Option<String>) {
        self.voice_dialect = dialect;
    }

    pub fn set_character_path(&mut self, path: Option<PathBuf>) {
        self.character_path = path;
    }

    pub fn tts_type(&self) -> &str {
        &self.tts_type
    }

    /// 当前 VoiceMaker 实际使用的语音语言。
    ///
    /// 该值已经应用角色设置优先、全局 TTS 设置兜底规则，是翻译、合成与显示
    /// 应共同使用的权威语言来源。
    pub fn lang(&self) -> &str {
        &self.lang
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
        // Fish S2 可使用角色级音色，也可回退到全局默认音色。
        let fish_s2 =
            non_empty(&cfg.fish_s2_voice) || !self.tts_config.fish_s2_voice.trim().is_empty();
        // Local SBV2 only needs a voice_id; engine readiness is checked later
        let sbv2_local = non_empty(&cfg.sbv2_local_voice_id);
        // CosyVoice 云端音色：角色选了 voice_id 即可用（Key 缺失在初始化时禁用）
        let cosyvoice = non_empty(&cfg.cosyvoice_voice_id);

        self.availability = TtsAvailability {
            sva,
            sbv2,
            bv2,
            sbv2api,
            gsv,
            aivis,
            opentts,
            fish_s2,
            sbv2_local,
            cosyvoice,
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
            },
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
            },
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
            },
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
                    },
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
            },
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
            },
            "gsv" if self.availability.gsv => {
                let ref_audio_path = match (&self.character_path, &cfg.gsv_voice_filename) {
                    (Some(base), Some(name_)) if !name_.is_empty() => {
                        base.join("voice").join(name_).to_string_lossy().to_string()
                    },
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
                    },
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
            },
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
                    },
                }
            },
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
                        },
                    }
                }
            },
            "fishs2" if self.availability.fish_s2 => {
                // s2.cpp 固定返回 WAV；确保缓存文件扩展名与实际内容一致。
                self.audio_format = "wav".to_string();
                self.provider.audio_format = "wav".to_string();
                let voice = if non_empty(&cfg.fish_s2_voice) {
                    cfg.fish_s2_voice.clone().unwrap_or_default()
                } else {
                    self.tts_config.fish_s2_voice.clone()
                };
                match FishS2Adapter::new(self.tts_config.fish_s2_api_url.clone(), voice) {
                    Ok(adapter) => self.provider.fish_s2 = Some(Arc::new(adapter)),
                    Err(error) => {
                        tracing::warn!("Fish S2 初始化失败: {error}");
                        self.provider.disable();
                    },
                }
            },
            "cosyvoice" if self.availability.cosyvoice => {
                let api_key = self
                    .tts_config
                    .cosyvoice_api_key
                    .clone()
                    .unwrap_or_default();
                if api_key.trim().is_empty() {
                    tracing::warn!("CosyVoice API 密钥未设置,禁用 TTS");
                    self.provider.disable();
                } else {
                    let voice_id = cfg.cosyvoice_voice_id.clone().unwrap_or_default();
                    // 合成模型必须与注册音色时的模型一致：优先从音色记录解析，
                    // 找不到（音色记录被清）回退全局默认模型
                    let model = self
                        .tts_config
                        .cosyvoice_voices
                        .iter()
                        .find(|v| v.voice_id == voice_id)
                        .map(|v| v.model.clone())
                        .or_else(|| self.tts_config.cosyvoice_models.first().cloned())
                        .unwrap_or_else(|| "cosyvoice-v3.5-flash".to_string());
                    let mut adapter = CosyvoiceAdapter::new(api_key, model, voice_id);
                    // 方言：仅中文生效，通过 instruction 指令触发（复刻音色需指令才输出方言）；
                    // 其余语言：仅官方支持的语言才用 language_hints 指定合成目标语言
                    // （es/ar 等不支持的值会触发云端报错，跳过即可）
                    if self.lang == "zh" {
                        if let Some(dialect) =
                            self.voice_dialect.clone().filter(|d| !d.trim().is_empty())
                        {
                            tracing::info!("CosyVoice 方言模式: {dialect}");
                            adapter = adapter.with_instruction(&format!("用{dialect}说话。"));
                        }
                    } else if cosyvoice_supported_language(&self.lang) {
                        adapter = adapter.with_language_hints(&self.lang);
                    }
                    self.provider.cosyvoice = Some(Arc::new(adapter));
                }
            },
            "indextts2" => {
                // IndexTTS 2.5 起官方支持中/英/日/西班牙/阿拉伯语，
                // 不再对日语等语言做中文兜底，lang 直接透传给服务端。
                self.provider.indextts = Some(Arc::new(IndexTtsAdapter::new(
                    self.tts_config.indextts_api_url.clone(),
                    self.lang.clone(),
                )));
            },
            _ => {
                tracing::warn!("TTS 类型不可用或未初始化: {tts_type}");
            },
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

    /// 为每个片段生成语音，返回与 `segments` 对齐的成功标记。
    ///
    /// adapter 先写入保持原扩展名的临时文件；仅当调用成功且文件非空时才原子
    /// 重命名为最终文件。失败会清理临时/残缺产物并清空对应 `voice_file`。
    pub async fn generate_voice_files(&self, segments: &mut [EmotionSegment]) -> Vec<bool> {
        let mut successes = vec![false; segments.len()];
        if self.tts_type.is_empty() {
            return successes;
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
            return successes;
        }
        if let Err(e) = tokio::fs::create_dir_all(&self.temp_dir).await {
            tracing::error!("创建 TTS 输出目录失败: {e}");
            return successes;
        }

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
        for (position, seg) in segments.iter().enumerate() {
            let Some(text) = segment_text_for_lang(&self.lang, seg).map(str::to_owned) else {
                continue;
            };
            // 将情绪分类器的预测标签传给 TTS 适配器（IndexTTS2 与 Fish S2
            // 会消费 emo，其余适配器忽略该参数）。
            let emo = seg.predicted.clone();

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
            let partial_path = partial_audio_path(&file_path);

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
                let _ = tokio::fs::remove_file(&partial_path).await;
                let generated = provider
                    .generate_voice(&text, &partial_path, &tts_type, &emo)
                    .await;
                let mut ok = match generated {
                    Ok(()) => tokio::fs::metadata(&partial_path)
                        .await
                        .map(|metadata| metadata.len() > 0)
                        .unwrap_or(false),
                    Err(e) => {
                        tracing::error!("片段 {index} 语音生成失败: {e}");
                        false
                    },
                };

                if ok {
                    let _ = tokio::fs::remove_file(&file_path).await;
                    if let Err(e) = tokio::fs::rename(&partial_path, &file_path).await {
                        tracing::error!("片段 {index} 提交语音文件失败: {e}");
                        ok = false;
                    }
                }
                if !ok {
                    let _ = tokio::fs::remove_file(&partial_path).await;
                    let _ = tokio::fs::remove_file(&file_path).await;
                }
                (position, file_path, ok)
            });
        }

        for (position, file_path, ok) in join_all(futs).await {
            successes[position] = ok;
            if ok {
                segments[position].voice_file = file_path.to_string_lossy().to_string();
            } else {
                segments[position].voice_file.clear();
            }
        }
        successes
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{partial_audio_path, segment_text_for_lang};
    use crate::ai_service::message_system::processor::EmotionSegment;

    fn segment(main: &str, translated: &str) -> EmotionSegment {
        EmotionSegment {
            following_text: main.to_string(),
            japanese_text: translated.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn chinese_tts_uses_main_text_even_when_secondary_text_exists() {
        let value = segment("早上好", "おはよう");
        assert_eq!(segment_text_for_lang("zh", &value), Some("早上好"));
    }

    #[test]
    fn non_chinese_tts_uses_target_translation() {
        let value = segment("早上好", "좋은 아침");
        assert_eq!(segment_text_for_lang("ko", &value), Some("좋은 아침"));
        assert_eq!(segment_text_for_lang("en", &value), Some("좋은 아침"));
        assert_eq!(segment_text_for_lang("ja", &value), Some("좋은 아침"));
        assert_eq!(segment_text_for_lang("es", &value), Some("좋은 아침"));
        assert_eq!(segment_text_for_lang("ar", &value), Some("좋은 아침"));
    }

    #[test]
    fn non_chinese_tts_does_not_fall_back_to_wrong_language() {
        let value = segment("早上好", "");
        assert_eq!(segment_text_for_lang("ko", &value), None);
        assert_eq!(segment_text_for_lang("en", &value), None);
        assert_eq!(segment_text_for_lang("es", &value), None);
        assert_eq!(segment_text_for_lang("ar", &value), None);
    }

    #[test]
    fn auto_and_unknown_languages_keep_main_text() {
        let value = segment("早上好", "おはよう");
        assert_eq!(segment_text_for_lang("auto", &value), Some("早上好"));
        assert_eq!(segment_text_for_lang("", &value), Some("早上好"));
    }

    #[test]
    fn partial_audio_path_keeps_original_extension() {
        assert_eq!(
            partial_audio_path(Path::new("voice.wav")),
            Path::new("voice.part.wav")
        );
    }
}
