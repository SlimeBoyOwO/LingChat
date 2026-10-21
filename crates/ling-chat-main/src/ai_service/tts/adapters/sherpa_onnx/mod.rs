//! Sherpa-ONNX 本地 TTS 适配器。
//!
//! - [`SherpaOnnxAdapter`]：模型生命周期（创建 / 参数 / 合成入口）；
//! - `engine`：按模型类型定位文件、构造推理引擎；
//! - `text`：可读字符过滤、空格处理、按标点切句、情绪 → 韵律；
//! - `audio`：WAV 编解码、参考音频、静音裁剪、接缝淡入淡出。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{Value as JsonValue, json};
use sherpa_onnx::{GenerationConfig, OfflineTts};

use crate::ai_service::tts::provider::TtsAdapter;
use crate::config::tts::TtsConfig;

mod audio;
mod engine;
mod text;

pub use audio::load_reference_audio;
use audio::{
    LEAD_SILENCE_KEEP, TRAIL_SILENCE_KEEP, apply_edge_fades, cap_inner_silence, f32_to_wav,
    strip_edge_silence,
};
use engine::{kokoro_sid, model_reads_latin, needs_digit_normalization};
use text::{
    CLAUSE_PAUSE, ReadableScripts, collapse_cjk_spaces, emotion_prosody, has_inner_clause_punct,
    readable_scripts, reads_han_as_chinese, resolve_emotion, split_clauses, strip_unreadable,
};

/// 本次编译链接进来的 sherpa-onnx 把哪个执行提供方编进去了：`dml`（Windows DirectML
/// 版，见 scripts/build-desktop-sherpa-dml.mjs）、`nnapi`（Android NNAPI 版，见
/// scripts/build-android-sherpa-nnapi.mjs）或 `cpu`。
///
/// 由本 crate 的 build.rs 读 `SHERPA_ONNX_LIB_DIR/sherpa-build.json` 发射（见
/// `emit_sherpa_provider`）——以**实际链接的那套库**为准：上游 build.rs 自己下的官方
/// 预编译包没编 DirectML / NNAPI，此时请求 GPU 只会被静默降级到 CPU，所以那种情况就
/// 不该把 provider 交给 sherpa。
#[cfg(any(target_os = "windows", target_os = "android"))]
const SHERPA_PROVIDER: &str = env!("SHERPA_ONNX_PROVIDER");

/// 停顿缩放。sherpa 把「长度超过 0.2 秒的静音」按这个比例缩放（<1 缩短、>1 拉长）。
///
/// Rust 绑定的 `Default` 是 0.2——上游 CLI 用来压掉空档的默认值，落在句读上就是
/// 「所有停顿只剩两成」：本机实测 72 字中文长句，句号/问号后的停顿从 0.70~1.03 秒被
/// 压到 0.13~0.24 秒，和逗号后的停顿（0.15 秒）几乎分不出来，听感就是「断句不清、
/// 读得赶」。反过来完全不缩放（1.0）又太拖：zipvoice 的自然句读停顿能到 1 秒。
/// 取 0.6——句号后约 0.42~0.45 秒、逗号后约 0.17~0.24 秒，停顿的**长短层次**保留，
/// 整段又不会听起来慢。
///
/// 句读位置本身由模型的 token 序列决定：sherpa 的前端即使按标点切句，各 impl 也会把
/// token 展平回一条序列再推理，所以这里只需要管好这个缩放系数。
const SILENCE_SCALE: f32 = 0.6;

/// 一次推理里最多放几句——就是 sherpa 的 `max_num_sentences`，取 0 表示不限。
///
/// 超过这个数，sherpa 会把文本里的句子**分批**推理，批与批之间是硬接
/// （`offline-tts-matcha-impl.h` 里就是 `ans.samples.insert(...)`：没有静音、没有
/// 淡入淡出），上下文也断在批边界上。我们自己在按标点切句（见 [`split_clauses`]），
/// 一片里可能有两三个标点（短句合并、`9:30` 这种词内标点），撞上分批就前功尽弃，
/// 所以干脆不限批：一片永远只跑一次推理，各句 token 展平回一条序列，标点和语气
/// 都还连着（见 `Process()`）。我们切的每一片最长也就几句话，不存在内存问题。
const MAX_NUM_SENTENCES: i32 = 0;

/// ZipVoice 的 flow-matching 推理步数。
///
/// 官方文档与示例都用 4（`--num-steps=4`，见 `python-api-examples/zipvoice-tts.py`、
/// `c-api-examples/zipvoice-tts-zh-en-c-api.c`），而 sherpa-onnx 的 Rust 绑定
/// `GenerationConfig::default()` 给的是 5，原生侧 `SHERPA_ONNX_OR(config->num_steps, 5)`
/// 又把这个 5 原样透传，于是每次都比官方多跑一步。实测 4/5/8/16 步的输出时长
/// （3.50~3.74 秒）与音色（余弦距离 1.1~5.5‰，与同一配置重复生成的抖动同量级）没有
/// 可辨差别，16 步只是把耗时从 5.7 秒拉到 18.2 秒——所以这里不是为了修音质，而是
/// 对齐上游、顺带省一步推理。
const ZIPVOICE_NUM_STEPS: i32 = 4;

/// ZipVoice 引导强度 `guidance_scale` 的默认值。
///
/// 取上游原生默认（`offline-tts-zipvoice-model-config.h`：`guidance_scale = 1.0`），
/// 也就是不做额外的 CFG 放大。注意它是**模型配置**而不是逐次生成参数：sherpa 在
/// `OfflineTts::create` 时读进引擎，之后改只能重建适配器（设置页保存会重建）。
const ZIPVOICE_GUIDANCE_SCALE: f32 = 1.0;

/// 设置页滑动条的范围。越界/缺失的值在 [`SherpaTuning::from_options`] 里收敛到这里，
/// 免得手改设置文件把引擎调成非法参数（sherpa 对 `num_steps <= 0` 会直接报错）。
const NUM_STEPS_MIN: i32 = 1;
const NUM_STEPS_MAX: i32 = 10;
const GUIDANCE_SCALE_MIN: f32 = 1.0;
const GUIDANCE_SCALE_MAX: f32 = 10.0;

/// 用户可调的采样参数（设置页「参数」区的两个滑动条）。
///
/// 各模型只读自己认识的那部分：`num_steps` 只有 ZipVoice 这类 flow-matching 模型用，
/// `guidance_scale` 只有 ZipVoice 用；其余模型拿到它们不会出错，只是不起作用。
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
    /// 把设置里的原始值（可能缺失、可能是手改出来的越界值）收敛到可用范围。
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

pub struct SherpaOnnxAdapter {
    tts: Option<OfflineTts>,
    /// 传给模型的说话人下标（kokoro 由 [`kokoro_sid`] 按语言/音色挑，其余模型为 0）。
    sid: i32,
    speed: f32,
    /// 用户调的 flow-matching 推理步数（见 [`SherpaTuning`]）。
    num_steps: i32,
    model_path: String,
    model_type: String,
    language: String,
    /// 设置页的「音色」标签（female/male/…；kokoro 还认纯数字 sid）。
    voice: String,
    use_gpu: bool,
    reference_audio: Option<Vec<f32>>,
    reference_sample_rate: Option<i32>,
    reference_text: Option<String>,
    /// 中文模型 + 模型目录里没有规则 FST 时，需要在合成前把阿拉伯数字改写成汉字。
    digit_normalization: bool,
    /// 模型读得出哪几种文字（拉丁字母 / 假名 / 谚文）。不认得的那几种要在合成前剔掉，
    /// 见 [`strip_unreadable`]。
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

        // 以模型目录本身为准解析真实类型：角色设置里的 `sherpa_onnx_model_type`
        // 默认为 `vits`，若照单全收会让 Kokoro/Matcha 等模型按错误类型加载而报
        // 「未找到模型文件」。
        let model_type = crate::ai_service::tts::local::sherpa_onnx_manager::resolve_model_type(
            model_dir,
            &model_type,
        );

        let provider = Self::provider_for(use_gpu);

        let engine = |provider: Option<&str>| {
            engine::create_engine(&model_type, model_dir, &language, provider, tuning)
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
        // 中文模型若模型目录里没带规则 FST（zipvoice/supertonic 的实现不读 rule_fsts，
        // 别的中文模型也可能没随包带），数字得由我们在送进 sherpa 之前改写。
        let digit_normalization = needs_digit_normalization(model_dir);
        // 假名/谚文按目标语言留：用户在角色设置里选的 `sherpa_onnx_lang` 就是模型
        // 说的语言（见 fields.ts），日文/韩文模型不需要英文词表也能读自己的文字。
        let scripts = readable_scripts(&language, model_reads_latin(model_dir));
        // kokoro 的 sid 是模型自带的音色下标（见 [`kokoro_sid`]）；其余模型的 sid 只有
        // 多说话人 VITS / Kitten 用得上，保持 0。
        let sid = if model_type == "kokoro" {
            kokoro_sid(&language, &voice, tts.num_speakers())
        } else {
            0
        };
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

    /// 根据是否开启 GPU 加速与目标平台，选择 ONNX Runtime 执行提供方。
    ///
    /// 返回 `None` 表示使用 CPU。各平台：
    /// - Android: `nnapi`（设备后端，Android 8.1+）。只有本次链接的 sherpa 是**编了
    ///   NNAPI 的那份**（`pnpm android:build` 会自编，见
    ///   scripts/build-android-sherpa-nnapi.mjs）才给 `nnapi`：官方 `-android` 包按
    ///   API 21 编，NNAPI 那段代码根本不在，传了只会打一行 fallback 日志再静默回落
    ///   CPU。以编译期常量 `SHERPA_ONNX_PROVIDER`（来自 sherpa-build.json）为准。
    /// - Apple（macOS / iOS）: `coreml`
    /// - Windows: 只有本次链接的 sherpa 是**编了 DirectML 的那份**（用
    ///   `pnpm tauri:build:win` 打的包，见 scripts/build-desktop-sherpa-dml.mjs）才给
    ///   `dml`。官方预编译包没编 DirectML，传了只会打一行 fallback 日志然后静默回落
    ///   CPU —— 那等于把开关变成假的，所以宁可不交。
    /// - 其它（Linux x86_64 等）: CPU。Linux 上 sherpa 只有 CUDA 一条 GPU 路，
    ///   代价是官方 CUDA 包（+388 MB），已按体积取舍移除。
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

        // 情绪没有模型级入口（见 emotion_prosody），只调语速和标点停顿。上游给的
        // 通常是情绪模型的输出；为空或不认识时在这里对正文补一次预测（见
        // resolve_emotion），所以试听这类没有情绪标签的入口也能走情绪。
        let emotion = resolve_emotion(
            emo,
            text,
            crate::ai_service::emotion::global_classifier().map(|classifier| classifier.as_ref()),
        );
        let (speed_mul, pause_mul) = emotion_prosody(&emotion);

        let mut gen_config = GenerationConfig {
            sid: self.sid,
            speed: self.speed * speed_mul,
            // 见 SILENCE_SCALE：压掉两成的默认值会让句读停顿和逗号停顿分不出来。
            silence_scale: SILENCE_SCALE,
            reference_audio: self.reference_audio.clone(),
            reference_sample_rate: self.reference_sample_rate.unwrap_or(0),
            reference_text: self.reference_text.clone(),
            ..Default::default()
        };
        if self.model_type == "zipvoice" {
            gen_config.num_steps = self.num_steps; // 见 ZIPVOICE_NUM_STEPS / SherpaTuning
        }

        // 日文/韩文的汉字读法与中文不同（见 reads_han_as_chinese），这两门语言绕开下面
        // 两步中文改写，数字原样交给模型自己的前端。
        let chinese_readings = reads_han_as_chinese(&self.language);

        // 中文文本里数字之间夹的标点先归置：`9:30` 要念「九点三十」、`1,000` 要念
        // 「一千」、`2026-10-20` 要念「二零二六年十月二十日」。模型自带的规则 FST
        // 只改数字、不懂标点（实测「上午9:30」→「上午九:三十」），冒号/逗号的读法
        // 全靠这里。见 chinese_digits::normalize_number_punct。
        let with_punct;
        let text: &str =
            if chinese_readings && crate::ai_service::tts::chinese_digits::has_han(text) {
                with_punct = crate::ai_service::tts::chinese_digits::normalize_number_punct(text);
                &with_punct
            } else {
                text
            };

        // 中文文本且模型里没带规则 FST 时，先把阿拉伯数字改写成汉字（见 chinese_digits）：
        // 那些模型下数字读法都不对——CharacterLexicon 会直接丢词，zipvoice 这类会念错
        // （反读实测「第 3 章」听成「对章」）。纯英文台词不改写，数字该按英文念。
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

        // 不分语言，一律按标点自己切句、逐句推理、再拼回去（见 split_clauses）。
        //
        // 以前只有含汉字的文本才切，理由是「英文标点交给模型自己处理」：结果是纯英文、
        // 纯假名、谚文整段一次推理——句间停顿由模型临场发挥（情绪里的停顿倍率也就
        // 完全失效），接缝没有淡入淡出；而同一段日文有没有汉字还会走两条不同的路径。
        // 英文的 `Mr.` / `Dr.` / `e.g.` 这类缩写另有保护，见 [`split_clauses`]。
        let clauses = split_clauses(text);

        let mut assembled: Vec<f32> = Vec::new();
        let mut sample_rate = tts.sample_rate();

        for (clause, pause, _) in clauses {
            // 数字改写可能带出分隔符，这里按小句统一清掉读不出来的字符
            // （见 strip_unreadable）；再收掉中文/日文里没意义的空格——kokoro 会把
            // 空格当成一个非汉字段单独推理，多出一整句的停顿（见 collapse_cjk_spaces）。
            let piece_text = collapse_cjk_spaces(&strip_unreadable(&clause, self.scripts));
            // 只剩标点的小句（「……」「~」）没有可念的字，跳过：丢给模型只会得到一段
            // 空音频或一声怪音。
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

            // 合并出来的那片可能带着模型自己在逗号后塞的静音（0.35~0.68 秒，见
            // cap_inner_silence），先压回我们自己的从句停顿，再和别处对齐。
            let mut samples = audio.samples().to_vec();
            if has_inner_clause_punct(&piece_text) {
                cap_inner_silence(&mut samples, sample_rate, CLAUSE_PAUSE);
            }

            // 每片的静音都去掉：停顿由我们按标点精确插入，片自带的静音只会让停顿
            // 忽长忽短；接缝两侧各做一次淡入淡出，避免波形跳变听起来是「咔哒」。
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

        // 首尾各留一点点静音（与串联多段台词时的观感一致，见 LEAD/TRAIL_SILENCE_KEEP）。
        let lead = vec![0.0; (sample_rate as f32 * LEAD_SILENCE_KEEP) as usize];
        let trail = vec![0.0; (sample_rate as f32 * TRAIL_SILENCE_KEEP) as usize];
        let mut samples = Vec::with_capacity(lead.len() + assembled.len() + trail.len());
        samples.extend_from_slice(&lead);
        samples.extend_from_slice(&assembled);
        samples.extend_from_slice(&trail);

        Ok(f32_to_wav(&samples, sample_rate))
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut params = HashMap::new();
        params.insert("model_path".into(), json!(self.model_path));
        params.insert("model_type".into(), json!(self.model_type));
        params.insert("language".into(), json!(self.language));
        params.insert("voice".into(), json!(self.voice));
        params.insert("use_gpu".into(), json!(self.use_gpu));
        params.insert("speed".into(), json!(self.speed));
        params.insert("sid".into(), json!(self.sid));
        params.insert(
            "has_reference_audio".into(),
            json!(self.reference_audio.is_some()),
        );
        params
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

    /// 设置页两个滑动条的值：缺失 / 越界 / NaN 都要收敛到可用范围，
    /// 不能让手改过的设置文件把引擎调成 `num_steps = 0` 这种非法参数。
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

    /// ZipVoice 零样本语音克隆：加载参考音频 → 设置 → 合成 → 验证非静音。
    ///
    /// 依赖真实模型和参考音频，默认忽略；通过设置 `SHERPA_TEST_MODEL_DIR`（模型目录）
    /// 并用 `-- --ignored` 显式运行。
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

        // 参考文本必须与参考音频内容一致（ZipVoice 会用它对提示音频做对齐），
        // 换参考音频时用 SHERPA_TEST_REF_TEXT 一起换。
        let ref_text = std::env::var("SHERPA_TEST_REF_TEXT")
            .unwrap_or_else(|_| "你好，我是白子。".to_string());

        // 加载参考音频
        let (samples, sample_rate) =
            load_reference_audio(Path::new(&ref_path)).expect("加载参考音频失败");
        eprintln!(
            "参考音频: sr={}, samples={} ({:.2}s)",
            sample_rate,
            samples.len(),
            samples.len() as f64 / sample_rate as f64
        );

        // 初始化 adapter
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

        // 设置参考音频
        let mut adapter = adapter;
        adapter.set_reference_audio(samples, sample_rate, ref_text);
        adapter.set_speed(1.0);

        // 合成
        let text = "老师好，今天也请多指教了。";
        eprintln!("合成文本: {text}");
        let wav = adapter.generate_voice(text, "").await.expect("合成失败");

        let (sr, num_samples) = audio::parse_generated_wav(&wav);
        eprintln!("输出: sr={sr}, 样本数={num_samples}, WAV-Ok");

        // 验证音频非静音
        assert!(audio::pcm_has_signal(&wav, 200), "生成音频疑似全静音");

        // 保存输出
        let out = std::env::temp_dir().join("sherpa_clone_output.wav");
        std::fs::write(&out, &wav).expect("写入输出失败");
        eprintln!("输出文件: {}", out.display());
    }

    /// 完整跑一遍 Matcha 语音合成（覆盖初始化 + generate_voice + WAV 编码），
    /// 分别验证 CPU 与 GPU(此机无 NVIDIA -> CUDA 失败回退 CPU) 两条路径。
    ///
    /// 依赖真实模型，默认忽略；通过设置 `SHERPA_TEST_MODEL_DIR`（模型目录）
    /// 并用 `-- --ignored` 显式运行。
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
