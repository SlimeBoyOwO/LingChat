//! Sherpa-ONNX 本地 TTS 适配器。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use sherpa_onnx::{GenerationConfig, OfflineTts};

use crate::ai_service::tts::provider::TtsAdapter;
use crate::config::tts::TtsConfig;

mod audio;
mod engine;
mod meta;
mod text;

pub use audio::load_reference_audio;
use audio::{
    LEAD_SILENCE_KEEP, TRAIL_SILENCE_KEEP, apply_edge_fades, cap_inner_silence, f32_to_wav,
    strip_edge_silence,
};
use engine::{model_reads_latin, needs_digit_normalization, pick_sid};
use text::{
    CLAUSE_PAUSE, ReadableScripts, collapse_cjk_spaces, emotion_prosody, has_inner_clause_punct,
    readable_scripts, reads_han_as_chinese, resolve_emotion, split_clauses, strip_unreadable,
};

/// 本次链接的 sherpa-onnx 编入的执行提供方（`dml` / `nnapi` / `cpu`）。
///
/// 由 build.rs 读 `sherpa-build.json` 发射；必须与实际链接的库一致，否则请求 GPU 会被静默回退 CPU。
#[cfg(any(target_os = "windows", target_os = "android"))]
const SHERPA_PROVIDER: &str = env!("SHERPA_ONNX_PROVIDER");

/// 静音缩放系数。sherpa 按此缩放超过 0.2 秒的静音；默认 0.2 会把句号/逗号停顿
/// 压得几乎一样，取 0.6 保留停顿的长短层次。
const SILENCE_SCALE: f32 = 0.6;

/// 单次推理最多句数（sherpa 的 `max_num_sentences`），0 表示不限。
///
/// sherpa 分批推理的拼接没有静音与淡入淡出、上下文也断在批边界，所以这里不限批。
const MAX_NUM_SENTENCES: i32 = 0;

/// ZipVoice flow-matching 推理步数。上游文档用 4，Rust 绑定默认 5，这里对齐上游。
const ZIPVOICE_NUM_STEPS: i32 = 4;

/// ZipVoice 引导强度默认值（上游原生默认 1.0）。
///
/// 属于模型配置：修改后需重建适配器，不能逐次生效。
const ZIPVOICE_GUIDANCE_SCALE: f32 = 1.0;

/// 设置页滑动条范围；越界值在 [`SherpaTuning::from_options`] 收敛（sherpa 对 `num_steps <= 0` 直接报错）。
const NUM_STEPS_MIN: i32 = 1;
const NUM_STEPS_MAX: i32 = 10;
const GUIDANCE_SCALE_MIN: f32 = 1.0;
const GUIDANCE_SCALE_MAX: f32 = 10.0;

/// 用户可调的采样参数；各模型只读取自己支持的部分。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SherpaTuning {
    /// flow-matching 推理步数（1–10）。
    pub num_steps: i32,
    /// 引导强度（1.0–10.0）。
    pub guidance_scale: f32,
}

impl Default for SherpaTuning {
    fn default() -> Self {
        Self {
            num_steps: ZIPVOICE_NUM_STEPS,
            guidance_scale: ZIPVOICE_GUIDANCE_SCALE,
        }
    }
}

impl SherpaTuning {
    /// 把设置里的原始值收敛到可用范围。
    pub fn from_options(num_steps: Option<i32>, guidance_scale: Option<f32>) -> Self {
        let default = Self::default();
        Self {
            num_steps: num_steps
                .unwrap_or(default.num_steps)
                .clamp(NUM_STEPS_MIN, NUM_STEPS_MAX),
            guidance_scale: match guidance_scale {
                // NaN 会让 clamp 原样返回 NaN，必须先挡掉。
                Some(v) if v.is_finite() => v.clamp(GUIDANCE_SCALE_MIN, GUIDANCE_SCALE_MAX),
                _ => default.guidance_scale,
            },
        }
    }
}

/// 模型自带的一个说话人（`sid` 为合成时传给 sherpa 的下标）。
#[derive(Debug, Clone, Serialize)]
pub struct SherpaSpeaker {
    pub sid: i32,
    /// metadata 里的名字；没有时为空串。
    pub name: String,
}

/// 读取模型目录里的全部说话人，不加载模型。
///
/// 多文件模型没有单一主模型，返回错误，由调用方回退到语义档。
pub fn list_speakers(model_dir: &Path) -> Result<Vec<SherpaSpeaker>> {
    let model_file = engine::find_model_file(model_dir)?;
    let metadata = meta::read_onnx_metadata(&model_file)?;
    let (num_speakers, names) = meta::speakers_from_metadata(&metadata);
    Ok((0..num_speakers as i32)
        .map(|sid| SherpaSpeaker {
            sid,
            name: names.get(sid as usize).cloned().unwrap_or_default(),
        })
        .collect())
}

pub struct SherpaOnnxAdapter {
    tts: Option<OfflineTts>,
    /// 传给模型的说话人下标（kokoro 按语言/音色挑，其它模型可用数字 voice 指定）。
    sid: i32,
    speed: f32,
    num_steps: i32,
    model_path: String,
    model_type: String,
    language: String,
    /// 设置页的「音色」标签（female/male/…；kokoro 也认数字 sid）。
    voice: String,
    use_gpu: bool,
    reference_audio: Option<Vec<f32>>,
    reference_sample_rate: Option<i32>,
    reference_text: Option<String>,
    /// 中文模型且目录里没有规则 FST 时，合成前把阿拉伯数字改写成汉字。
    digit_normalization: bool,
    /// 模型读得出的文字种类；不认的在合成前剔除（见 [`strip_unreadable`]）。
    scripts: ReadableScripts,
}

impl SherpaOnnxAdapter {
    pub fn new(
        _config: TtsConfig,
        model_path: String,
        model_type: String,
        language: String,
        voice: String,
        use_gpu: bool,
        tuning: SherpaTuning,
    ) -> Result<Self> {
        let model_dir = Path::new(&model_path);
        if !model_dir.exists() {
            return Err(anyhow!("Sherpa-ONNX 模型目录不存在: {model_path}"));
        }

        // 以模型目录为准解析类型，避免角色设置里的默认 vits 让模型按错误类型加载。
        let model_type = crate::ai_service::tts::local::sherpa_onnx_manager::resolve_model_type(
            model_dir,
            &model_type,
        );

        let provider = Self::provider_for(use_gpu);

        let engine = |provider: Option<&str>| {
            engine::create_engine(&model_type, model_dir, provider, tuning)
        };

        let tts = match engine(provider) {
            Ok(tts) => {
                if let Some(p) = provider {
                    tracing::info!("Sherpa-ONNX 使用推理后端: {p}");
                }
                tts
            },
            Err(e) => match provider {
                // GPU/专用后端初始化失败时静默回退到 CPU，保证可用性。
                Some(_) => {
                    tracing::warn!("Sherpa-ONNX 后端初始化失败（{e}），回退到 CPU 推理");
                    engine(None)?
                },
                None => return Err(e),
            },
        };

        let sr = tts.sample_rate();
        // 中文模型若目录里没带规则 FST，数字需在送进 sherpa 前改写。
        let digit_normalization = needs_digit_normalization(model_dir);
        // 假名/谚文按目标语言保留；日文/韩文模型读自己的文字不需要英文词表。
        let scripts = readable_scripts(&language, model_reads_latin(model_dir));
        // 说话人数量来自模型 metadata；具体选择见 `pick_sid`。
        let sid = pick_sid(&model_type, &language, &voice, tts.num_speakers());
        tracing::info!(
            "Sherpa-ONNX 初始化完成: model_type={model_type}, language={language}, voice={voice}, sid={sid}, provider={:?}, sample_rate={sr}, 数字改写={digit_normalization}, 可读文字={scripts:?}",
            provider.unwrap_or("cpu")
        );

        Ok(Self {
            tts: Some(tts),
            sid,
            speed: 1.0,
            num_steps: tuning.num_steps,
            model_path,
            model_type,
            language,
            voice,
            use_gpu,
            reference_audio: None,
            reference_sample_rate: None,
            reference_text: None,
            digit_normalization,
            scripts,
        })
    }

    /// 按平台与编译期后端选择执行提供方；`None` 为 CPU。
    /// 仅当实际链接的库编入了 NNAPI（Android）/ DirectML（Windows）才启用，否则会静默回落；
    /// Apple 用 `coreml`。
    fn provider_for(use_gpu: bool) -> Option<&'static str> {
        if !use_gpu {
            return None;
        }
        #[cfg(target_os = "android")]
        {
            if SHERPA_PROVIDER == "nnapi" {
                return Some("nnapi");
            }
            tracing::warn!(
                "Sherpa-ONNX 这份构建没编 NNAPI（SHERPA_ONNX_PROVIDER={SHERPA_PROVIDER}），\
                 本次按 CPU 推理；要 GPU 请用 `pnpm android:build` 打包"
            );
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            if SHERPA_PROVIDER == "dml" {
                return Some("dml");
            }
            tracing::warn!(
                "Sherpa-ONNX 这份构建没编 DirectML（SHERPA_ONNX_PROVIDER={SHERPA_PROVIDER}），\
                 本次按 CPU 推理；要 GPU 请用 `pnpm tauri:build:win` 打包"
            );
            return None;
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            return Some("coreml");
        }
        #[allow(unreachable_code)]
        None
    }

    pub fn set_reference_audio(&mut self, samples: Vec<f32>, sample_rate: i32, text: String) {
        self.reference_audio = Some(samples);
        self.reference_sample_rate = Some(sample_rate);
        self.reference_text = Some(text);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }
}

#[async_trait]
impl TtsAdapter for SherpaOnnxAdapter {
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>> {
        let tts = self
            .tts
            .as_ref()
            .ok_or_else(|| anyhow!("Sherpa-ONNX 引擎未初始化"))?;

        // 情绪只调语速与停顿；上游没给或不认识时按正文补一次预测。
        let emotion = resolve_emotion(
            emo,
            text,
            crate::ai_service::emotion::global_classifier().map(|classifier| classifier.as_ref()),
        );
        let (speed_mul, pause_mul) = emotion_prosody(&emotion);

        let mut gen_config = GenerationConfig {
            sid: self.sid,
            speed: self.speed * speed_mul,
            // 见 SILENCE_SCALE：默认 0.2 会把句读停顿压得和逗号一样。
            silence_scale: SILENCE_SCALE,
            reference_audio: self.reference_audio.clone(),
            reference_sample_rate: self.reference_sample_rate.unwrap_or(0),
            reference_text: self.reference_text.clone(),
            ..Default::default()
        };
        if self.model_type == "zipvoice" {
            gen_config.num_steps = self.num_steps; // 见 ZIPVOICE_NUM_STEPS / SherpaTuning
        }

        // 日文/韩文的汉字读法与中文不同，数字原样交给模型前端。
        let chinese_readings = reads_han_as_chinese(&self.language);

        // 中文数字间的标点先归置：`9:30` → 「九点三十」等；模型自带 FST 只改数字、不懂标点。
        let with_punct;
        let text: &str =
            if chinese_readings && crate::ai_service::tts::chinese_digits::has_han(text) {
                with_punct = crate::ai_service::tts::chinese_digits::normalize_number_punct(text);
                &with_punct
            } else {
                text
            };

        // 模型没有规则 FST 时，把阿拉伯数字改写成汉字，否则会丢词或念错。
        let normalized;
        let text: &str = if self.digit_normalization
            && chinese_readings
            && crate::ai_service::tts::chinese_digits::has_han(text)
        {
            normalized = crate::ai_service::tts::chinese_digits::normalize(text);
            &normalized
        } else {
            text
        };

        // 统一按标点切句、逐句推理再拼回；英文缩写另有保护（见 [`split_clauses`]）。
        let clauses = split_clauses(text);

        let mut assembled: Vec<f32> = Vec::new();
        let mut sample_rate = tts.sample_rate();

        for (clause, pause, _) in clauses {
            // 清掉读不出来的字符，并收掉中文/日文里无意义的空格。
            let piece_text = collapse_cjk_spaces(&strip_unreadable(&clause, self.scripts));
            // 只剩标点的小句没有可念的字，跳过。
            if !piece_text.chars().any(char::is_alphanumeric) {
                continue;
            }

            let audio = tts
                .generate_with_config(&piece_text, &gen_config, None::<fn(&[f32], f32) -> bool>)
                .ok_or_else(|| anyhow!("Sherpa-ONNX 生成失败（小句「{piece_text}」）"))?;
            if audio.samples().is_empty() {
                return Err(anyhow!(
                    "Sherpa-ONNX 生成的音频为空（小句「{piece_text}」）"
                ));
            }
            sample_rate = audio.sample_rate();

            // 模型可能在逗号后自带静音，先压回统一的从句停顿。
            let mut samples = audio.samples().to_vec();
            if has_inner_clause_punct(&piece_text) {
                cap_inner_silence(&mut samples, sample_rate, CLAUSE_PAUSE);
            }

            // 去掉片首尾静音，停顿由调用处精确插入；接缝两侧淡入淡出避免咔哒声。
            let edge = strip_edge_silence(&samples);
            if edge.is_empty() {
                continue;
            }
            let start = assembled.len();
            assembled.extend_from_slice(edge);
            apply_edge_fades(&mut assembled[start..], sample_rate);

            if pause > 0.0 {
                let pause = pause * pause_mul; // 见 emotion_prosody
                assembled.extend_from_slice(&vec![0.0; (sample_rate as f32 * pause) as usize]);
            }
        }

        if assembled.is_empty() {
            return Err(anyhow!("Sherpa-ONNX 生成的音频为空"));
        }

        // 首尾各留少量静音。
        let lead = vec![0.0; (sample_rate as f32 * LEAD_SILENCE_KEEP) as usize];
        let trail = vec![0.0; (sample_rate as f32 * TRAIL_SILENCE_KEEP) as usize];
        let mut samples = Vec::with_capacity(lead.len() + assembled.len() + trail.len());
        samples.extend_from_slice(&lead);
        samples.extend_from_slice(&assembled);
        samples.extend_from_slice(&trail);

        Ok(f32_to_wav(&samples, sample_rate))
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        HashMap::from([
            ("model_path".into(), json!(self.model_path)),
            ("model_type".into(), json!(self.model_type)),
            ("language".into(), json!(self.language)),
            ("voice".into(), json!(self.voice)),
            ("use_gpu".into(), json!(self.use_gpu)),
            ("speed".into(), json!(self.speed)),
            ("sid".into(), json!(self.sid)),
            (
                "has_reference_audio".into(),
                json!(self.reference_audio.is_some()),
            ),
        ])
    }
}

impl std::fmt::Debug for SherpaOnnxAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SherpaOnnxAdapter")
            .field("model_path", &self.model_path)
            .field("model_type", &self.model_type)
            .field("language", &self.language)
            .field("voice", &self.voice)
            .field("use_gpu", &self.use_gpu)
            .field("sid", &self.sid)
            .field("speed", &self.speed)
            .field("has_reference_audio", &self.reference_audio.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn test_model_dir() -> String {
        std::env::var("SHERPA_TEST_MODEL_DIR").unwrap_or_default()
    }

    /// 音色列表来自模型 metadata：数量、名字、缺省值。
    #[test]
    fn lists_speakers_from_model_metadata() {
        let dir = std::env::temp_dir().join(format!("lingchat-speakers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录失败");
        std::fs::write(
            dir.join("model.onnx"),
            meta::fake_onnx(&[
                ("n_speakers", "3"),
                ("speaker_names", "af_maple,zf_001,zm_009"),
            ]),
        )
        .expect("写假模型失败");

        let speakers = list_speakers(&dir).expect("读取音色失败");
        assert_eq!(speakers.len(), 3);
        assert_eq!(speakers[2].sid, 2);
        assert_eq!(speakers[2].name, "zm_009");

        // 没有 metadata 的模型：单说话人、无名字。
        std::fs::write(dir.join("model.onnx"), meta::fake_onnx(&[])).expect("写假模型失败");
        let speakers = list_speakers(&dir).expect("读取音色失败");
        assert_eq!(speakers.len(), 1);
        assert_eq!(speakers[0].sid, 0);
        assert!(speakers[0].name.is_empty());

        // 多文件模型没有主模型文件：报错，由调用方回退。
        std::fs::remove_file(dir.join("model.onnx")).expect("删除假模型失败");
        assert!(list_speakers(&dir).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 滑动条取值：缺失 / 越界 / NaN 都要收敛到可用范围。
    #[test]
    fn tuning_clamps_out_of_range_values() {
        assert_eq!(SherpaTuning::default().num_steps, 4);
        assert_eq!(SherpaTuning::default().guidance_scale, 1.0);

        assert_eq!(SherpaTuning::from_options(Some(0), None).num_steps, 1);
        assert_eq!(SherpaTuning::from_options(Some(99), None).num_steps, 10);
        assert_eq!(SherpaTuning::from_options(Some(-3), None).num_steps, 1);
        assert_eq!(
            SherpaTuning::from_options(None, None),
            SherpaTuning::default()
        );

        assert_eq!(
            SherpaTuning::from_options(None, Some(0.2)).guidance_scale,
            1.0
        );
        assert_eq!(
            SherpaTuning::from_options(None, Some(42.0)).guidance_scale,
            10.0
        );
        // NaN 会让 clamp 原样返回 NaN，必须先挡掉。
        assert_eq!(
            SherpaTuning::from_options(None, Some(f32::NAN)).guidance_scale,
            1.0
        );
        assert_eq!(
            SherpaTuning::from_options(None, Some(3.5)).guidance_scale,
            3.5
        );
    }

    /// ZipVoice 零样本克隆；依赖真实模型，默认忽略（`SHERPA_TEST_MODEL_DIR`）。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "需要 zipvoice 模型目录（SHERPA_TEST_MODEL_DIR）"]
    async fn zipvoice_clone_with_reference() {
        let model_dir = test_model_dir();
        assert!(
            !model_dir.is_empty() && Path::new(&model_dir).exists(),
            "模型目录不存在，请设置 SHERPA_TEST_MODEL_DIR: {model_dir}"
        );

        let ref_path = std::env::var("SHERPA_TEST_REF_AUDIO")
            .unwrap_or_else(|_| "/tmp/baizi_ref.wav".to_string());
        assert!(Path::new(&ref_path).exists(), "参考音频不存在: {ref_path}");

        // 参考文本必须与参考音频内容一致，换音频时用 SHERPA_TEST_REF_TEXT 一起换。
        let ref_text = std::env::var("SHERPA_TEST_REF_TEXT")
            .unwrap_or_else(|_| "你好，我是白子。".to_string());

        let (samples, sample_rate) =
            load_reference_audio(Path::new(&ref_path)).expect("加载参考音频失败");
        eprintln!(
            "参考音频: sr={}, samples={} ({:.2}s)",
            sample_rate,
            samples.len(),
            samples.len() as f64 / sample_rate as f64
        );

        let adapter = SherpaOnnxAdapter::new(
            TtsConfig::default(),
            model_dir,
            "zipvoice".to_string(),
            "zh".to_string(),
            "female".to_string(),
            false,
            SherpaTuning::default(),
        )
        .expect("初始化 Sherpa-ONNX 失败");

        let mut adapter = adapter;
        adapter.set_reference_audio(samples, sample_rate, ref_text);
        adapter.set_speed(1.0);

        let text = "老师好，今天也请多指教了。";
        eprintln!("合成文本: {text}");
        let wav = adapter.generate_voice(text, "").await.expect("合成失败");

        let (sr, num_samples) = audio::parse_generated_wav(&wav);
        eprintln!("输出: sr={sr}, 样本数={num_samples}, WAV-Ok");

        assert!(audio::pcm_has_signal(&wav, 200), "生成音频疑似全静音");

        let out = std::env::temp_dir().join("sherpa_clone_output.wav");
        std::fs::write(&out, &wav).expect("写入输出失败");
        eprintln!("输出文件: {}", out.display());
    }

    /// 完整跑一遍 Matcha 合成，覆盖 CPU 与 GPU 回退路径；依赖真实模型，默认忽略。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "需要真实 sherpa 模型目录（SHERPA_TEST_MODEL_DIR）"]
    async fn full_matcha_synthesis() {
        let model_dir = test_model_dir();
        assert!(
            !model_dir.is_empty() && Path::new(&model_dir).exists(),
            "模型目录不存在，请设置 SHERPA_TEST_MODEL_DIR: {model_dir}"
        );

        for use_gpu in [false, true] {
            let adapter = SherpaOnnxAdapter::new(
                TtsConfig::default(),
                model_dir.clone(),
                "matcha".to_string(),
                "zh".to_string(),
                "female".to_string(),
                use_gpu,
                SherpaTuning::default(),
            )
            .unwrap_or_else(|e| panic!("use_gpu={use_gpu} 初始化失败: {e}"));

            let wav = adapter
                .generate_voice("你好，世界。这是一段语音合成测试。", "")
                .await
                .expect("生成失败");

            let (sr, samples) = audio::parse_generated_wav(&wav);
            assert_eq!(sr, 22050, "Matcha-zh 采样率应为 22050");
            assert!(samples >= 5, "生成音频过短");

            assert!(
                audio::pcm_has_signal(&wav, 200),
                "use_gpu={use_gpu} 生成音频疑似全静音"
            );
            eprintln!("use_gpu={use_gpu}: sr={sr}, 样本数={samples}, WAV-Ok");
        }
    }
}
