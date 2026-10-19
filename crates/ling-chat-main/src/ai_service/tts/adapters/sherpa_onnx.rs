use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{Value as JsonValue, json};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKittenModelConfig,
    OfflineTtsKokoroModelConfig, OfflineTtsMatchaModelConfig, OfflineTtsModelConfig,
    OfflineTtsPocketModelConfig, OfflineTtsSupertonicModelConfig, OfflineTtsVitsModelConfig,
    OfflineTtsZipvoiceModelConfig,
};

use crate::ai_service::tts::adapters::gsv::gsv_emo_category;
use crate::ai_service::tts::provider::TtsAdapter;
use crate::config::tts::TtsConfig;

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

/// 首尾静音最多各保留多久（秒）。中间的停顿是句读，要留着；前后的空白在多段台词的
/// 场景里只会变成「每句都要等一会儿才开口」——zipvoice 逐次推理的段首静音实测能到
/// 0.4 秒，各模型之间也不一致。
const LEAD_SILENCE_KEEP: f32 = 0.1;
const TRAIL_SILENCE_KEEP: f32 = 0.15;

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

/// 句末标点：在这里切句，标点本身留在小句里（模型据此收尾），后面停 [`SENTENCE_PAUSE`]。
const SENTENCE_PUNCT: &[char] = &['。', '！', '？', '.', '!', '?', '…'];
/// 从句标点（逗号、顿号、分号、冒号）：也切句，后面停 [`CLAUSE_PAUSE`]。
///
/// 只想「按句号切」的话把这里清空即可：逗号会留在文本里交给模型自己处理，整句只做
/// 一次推理、语气更连贯，但逗号处的停顿长度就不可控了。
///
/// 数字/字母中间的这些标点不算断句（`9:30`、`3.14`、`1,000`），见 [`split_clauses`]。
const CLAUSE_PUNCT: &[char] = &['，', '、', '；', '：', ',', ';', ':', '—'];

/// 小句之间插多长静音（秒）。取值沿用先前实测的自然句读：句号后 0.4 秒左右、
/// 逗号后 0.2 秒左右（见 SILENCE_SCALE 的注释）。
const SENTENCE_PAUSE: f32 = 0.40;
const CLAUSE_PAUSE: f32 = 0.20;

/// 拼接处淡入淡出多久（秒）。20ms 足够把波形接缝压回静音，又短到听不出音量变化。
const JOINT_FADE: f32 = 0.02;

/// 静音判定阈值，与 sherpa 的 `ScaleSilence` 一致（0.01，约 -40 dBFS）。
const SILENCE_AMP: f32 = 0.01;

pub struct SherpaOnnxAdapter {
    tts: Option<OfflineTts>,
    sid: i32,
    speed: f32,
    model_path: String,
    model_type: String,
    language: String,
    voice: String,
    use_gpu: bool,
    reference_audio: Option<Vec<f32>>,
    reference_sample_rate: Option<i32>,
    reference_text: Option<String>,
    /// 中文模型 + 模型目录里没有规则 FST 时，需要在合成前把阿拉伯数字改写成汉字。
    digit_normalization: bool,
    /// 模型自己认得拉丁字母 / 日文假名吗。kokoro 多语言版带英文词表（`lexicon-us-en.txt`）
    /// 所以认得；中文 matcha / zipvoice 的词表里一个拉丁字母都没有。不认得时，汉字句子
    /// 里夹的英文、罗马音、假名要在合成前剔掉，见 [`strip_unreadable`]。
    reads_latin: bool,
}

impl SherpaOnnxAdapter {
    pub fn new(
        _config: TtsConfig,
        model_path: String,
        model_type: String,
        language: String,
        voice: String,
        use_gpu: bool,
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

        // 按平台选择合适的执行提供方（GPU/专用加速）；不可用或失败时回退 CPU。
        let create = |provider: Option<&str>| -> Result<OfflineTts> {
            match model_type.as_str() {
                "vits" | "fastspeech2" => Self::create_vits_tts(model_dir, provider),
                "matcha" => Self::create_matcha_tts(model_dir, provider),
                "kokoro" => Self::create_kokoro_tts(model_dir, &language, provider),
                "kitten" => Self::create_kitten_tts(model_dir, provider),
                "zipvoice" => Self::create_zipvoice_tts(model_dir, provider),
                "pocket" => Self::create_pocket_tts(model_dir, provider),
                "supertonic" => Self::create_supertonic_tts(model_dir, provider),
                other => Err(anyhow!("Sherpa-ONNX 不支持的模型类型: {other}")),
            }
        };

        let tts = match create(provider) {
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
                    create(None)?
                },
                None => return Err(e),
            },
        };

        let sr = tts.sample_rate();
        // 中文模型若模型目录里没带规则 FST（zipvoice/supertonic 的实现不读 rule_fsts，
        // 别的中文模型也可能没随包带），数字得由我们在送进 sherpa 之前改写。
        let digit_normalization = needs_digit_normalization(model_dir);
        let reads_latin = model_reads_latin(model_dir);
        tracing::info!(
            "Sherpa-ONNX 初始化完成: model_type={model_type}, language={language}, provider={:?}, sample_rate={sr}, 数字改写={digit_normalization}, 认得拉丁字母={reads_latin}",
            provider.unwrap_or("cpu")
        );

        Ok(Self {
            tts: Some(tts),
            sid: 0,
            speed: 1.0,
            model_path,
            model_type,
            language,
            voice,
            use_gpu,
            reference_audio: None,
            reference_sample_rate: None,
            reference_text: None,
            digit_normalization,
            reads_latin,
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

    fn create_vits_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let model_file = find_file(
            model_dir,
            &["model.onnx", "tts-model.onnx", "sherpa-onnx-tts.onnx"],
        )?;
        let tokens_file = find_file(model_dir, &["tokens.txt"])?;

        let vits_config = OfflineTtsVitsModelConfig {
            model: Some(model_file.to_string_lossy().to_string()),
            lexicon: find_file_optional(model_dir, &["lexicon.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            tokens: Some(tokens_file.to_string_lossy().to_string()),
            data_dir: find_dir_optional(model_dir, &["data-dir", "data"])
                .map(|p| p.to_string_lossy().to_string()),
            noise_scale: 0.667,
            noise_scale_w: 0.8,
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict-dir", "dict"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: vits_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: MAX_NUM_SENTENCES,
            silence_scale: SILENCE_SCALE,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX VITS 模型加载失败，请检查模型文件是否完整"))
    }

    /// 返回 `espeak-ng-data` 目录路径（即 data_dir 的取值）。
    fn espeak_data_dir(model_dir: &Path) -> Option<PathBuf> {
        find_dir_optional(model_dir, &["espeak-ng-data"])
    }

    fn create_matcha_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let acoustic_model = find_file(
            model_dir,
            &["model.onnx", "model-steps-3.onnx", "model-steps-6.onnx"],
        )?;
        let tokens_file = find_file(model_dir, &["tokens.txt"])?;
        let vocoder = find_file(
            model_dir,
            &[
                "vocos-22khz-univ.onnx",
                "vocos-16khz-univ.onnx",
                "hifigan_v3.onnx",
                "mb_melgan.onnx",
                "vocoder.onnx",
            ],
        )?;

        let matcha_config = OfflineTtsMatchaModelConfig {
            acoustic_model: Some(acoustic_model.to_string_lossy().to_string()),
            vocoder: Some(vocoder.to_string_lossy().to_string()),
            lexicon: find_file_optional(model_dir, &["lexicon.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            tokens: Some(tokens_file.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
            noise_scale: 0.667,
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                matcha: matcha_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: MAX_NUM_SENTENCES,
            silence_scale: SILENCE_SCALE,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Matcha 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_kokoro_tts(
        model_dir: &Path,
        language: &str,
        provider: Option<&str>,
    ) -> Result<OfflineTts> {
        let model = find_file(model_dir, &["model.onnx", "model.int8.onnx"])?;
        let voices = find_file(model_dir, &["voices.bin"])?;
        // Android/arm64 上 kokoro 的 int8 量化会输出纯静音或钉死的垃圾样本（上游
        // k2-fsa/sherpa-onnx#3754；同一设备换 fp32 立即恢复，已在实机验证）。这里
        // 直接报错，避免用户只听到一段静音却看不出原因。桌面端不受影响。
        if cfg!(target_os = "android")
            && model
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("int8"))
        {
            return Err(anyhow!(
                "Android 上 int8 量化的 Kokoro 模型会输出静音/杂音（上游缺陷 \
                 sherpa-onnx#3754），请改用未量化版本（Kokoro 多语言 103 音色 (未量化)）\
                 或其他模型"
            ));
        }
        // 多语言模型：只有日文需要显式指定——不指定时会被逐字当成中文念（27 字要
        // 16 秒）。中文/英文/中英混合以及数字都交给模型自行判断即可；显式传 "zh"
        // 反而会把句子里的英文片段和数字整段丢掉（音素化失败后静默跳过）。
        // 注意不能传 "auto"：它会让纯英文/日文片段音素化失败并让整次合成直接报错。
        let lang = match language.trim() {
            "ja" => Some("ja".to_string()),
            _ => None,
        };

        let kokoro_config = OfflineTtsKokoroModelConfig {
            model: Some(model.to_string_lossy().to_string()),
            voices: Some(voices.to_string_lossy().to_string()),
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict"])
                .map(|p| p.to_string_lossy().to_string()),
            lexicon: kokoro_lexicons(model_dir),
            lang,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: kokoro_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: MAX_NUM_SENTENCES,
            silence_scale: SILENCE_SCALE,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Kokoro 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_zipvoice_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let encoder = find_file(model_dir, &["text_encoder.onnx"])?;
        let decoder = find_file(model_dir, &["fm_decoder.onnx"])?;
        let vocoder = find_file(model_dir, &["vocos_24khz.onnx", "vocoder.onnx"])?;

        // data-dir 与 lexicon 都是 sherpa-onnx 的致命依赖：任一为空，原生侧就
        // `SHERPA_ONNX_EXIT(-1)`（=`_Exit(-1)`，进程以 255 静默退出，Rust 侧捕不到），
        // 所以先在这里拦成普通错误。
        let data_dir = zipvoice_data_dir(model_dir).ok_or_else(|| {
            anyhow!(
                "ZipVoice 模型缺少 espeak-ng-data（需含 phontab/phondata/phonindex），模型包不完整: {}",
                model_dir.display()
            )
        })?;
        let lexicon = zipvoice_lexicon(model_dir)?;

        let zipvoice_config = OfflineTtsZipvoiceModelConfig {
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            encoder: Some(encoder.to_string_lossy().to_string()),
            decoder: Some(decoder.to_string_lossy().to_string()),
            vocoder: Some(vocoder.to_string_lossy().to_string()),
            // ZipVoice 的 data-dir 必须是直接包含 phontab/phondata/phonindex 的目录
            //（与 Matcha/espeak 的 espeak-ng-data 父目录层级不同）。部分模型把
            // 这些语音库文件放到 espeak-ng-data/ 下，因此需按 phontab 实际所在目录解析。
            data_dir: Some(data_dir.to_string_lossy().to_string()),
            lexicon: Some(lexicon.to_string_lossy().to_string()),
            // 默认值必须显式写：sherpa-onnx 的 Rust 绑定这里 `Default` 全是 0.0，
            // 而原生侧要求 feat_scale/target_rms/guidance_scale > 0（0 会被
            // `Generate` 直接判为非法参数）。数值取上游原生默认值，见
            // sherpa-onnx/csrc/offline-tts-zipvoice-model-config.h：
            // `feat_scale = 0.1, t_shift = 0.5, target_rms = 0.1, guidance_scale = 1.0`。
            // 这三个值直接决定提示梅尔缩放、解码时间步与响度归一化，偏离一个数量级
            // 就会合成出极轻的低频噪声（实测 feat_scale=10、t_shift=0.005 时音频
            // 均响度只有 -36 dB，频谱集中在极低频）。
            feat_scale: 0.1,
            t_shift: 0.5,
            target_rms: 0.1,
            guidance_scale: 1.0,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                zipvoice: zipvoice_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: MAX_NUM_SENTENCES,
            silence_scale: SILENCE_SCALE,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX ZipVoice 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_kitten_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let model = find_file(model_dir, &["model.onnx", "model.int8.onnx"])?;
        let voices = find_file(model_dir, &["voices.bin"])?;

        let kitten_config = OfflineTtsKittenModelConfig {
            model: Some(model.to_string_lossy().to_string()),
            voices: Some(voices.to_string_lossy().to_string()),
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
            ..Default::default()
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kitten: kitten_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: MAX_NUM_SENTENCES,
            silence_scale: SILENCE_SCALE,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Kitten 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_pocket_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let pocket_config = OfflineTtsPocketModelConfig {
            lm_flow: Some(
                find_file(model_dir, &["lm_flow.int8.onnx", "lm_flow.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            lm_main: Some(
                find_file(model_dir, &["lm_main.int8.onnx", "lm_main.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            encoder: Some(
                find_file(model_dir, &["encoder.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            decoder: Some(
                find_file(model_dir, &["decoder.int8.onnx", "decoder.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            text_conditioner: Some(
                find_file(model_dir, &["text_conditioner.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            vocab_json: Some(
                find_file(model_dir, &["vocab.json"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            token_scores_json: Some(
                find_file(model_dir, &["token_scores.json"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            voice_embedding_cache_capacity: 8,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                pocket: pocket_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: MAX_NUM_SENTENCES,
            silence_scale: SILENCE_SCALE,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Pocket TTS 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_supertonic_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let supertonic_config = OfflineTtsSupertonicModelConfig {
            duration_predictor: Some(
                find_file(model_dir, &["duration_predictor.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            text_encoder: Some(
                find_file(model_dir, &["text_encoder.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            vector_estimator: Some(
                find_file(model_dir, &["vector_estimator.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            vocoder: Some(
                find_file(model_dir, &["vocoder.onnx", "vocoder_f16.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            tts_json: Some(
                find_file(model_dir, &["tts.json"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            unicode_indexer: find_file_optional(model_dir, &["unicode_indexer.json"])
                .map(|p| p.to_string_lossy().to_string()),
            voice_style: find_file_optional(model_dir, &["voice_style.onnx"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                supertonic: supertonic_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            silence_scale: SILENCE_SCALE,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Supertonic 模型加载失败，请检查模型文件是否完整"))
    }
}

#[async_trait]
impl TtsAdapter for SherpaOnnxAdapter {
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>> {
        let tts = self
            .tts
            .as_ref()
            .ok_or_else(|| anyhow!("Sherpa-ONNX 引擎未初始化"))?;

        // 情绪没有模型级入口（见 emotion_prosody），只调语速和标点停顿。
        let (speed_mul, pause_mul) = emotion_prosody(emo);

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
            gen_config.num_steps = ZIPVOICE_NUM_STEPS; // 见 ZIPVOICE_NUM_STEPS
        }

        // 中文文本里数字之间夹的标点先归置：`9:30` 要念「九点三十」、`1,000` 要念
        // 「一千」、`2026-10-20` 要念「二零二六年十月二十日」。模型自带的规则 FST
        // 只改数字、不懂标点（实测「上午9:30」→「上午九:三十」），冒号/逗号的读法
        // 全靠这里。见 chinese_digits::normalize_number_punct。
        let with_punct;
        let text: &str = if crate::ai_service::tts::chinese_digits::has_han(text) {
            with_punct = crate::ai_service::tts::chinese_digits::normalize_number_punct(text);
            &with_punct
        } else {
            text
        };

        // 中文文本且模型里没带规则 FST 时，先把阿拉伯数字改写成汉字（见 chinese_digits）：
        // 那些模型下数字读法都不对——CharacterLexicon 会直接丢词，zipvoice 这类会念错
        // （反读实测「第 3 章」听成「对章」）。纯英文台词不改写，数字该按英文念。
        let normalized;
        let text: &str =
            if self.digit_normalization && crate::ai_service::tts::chinese_digits::has_han(text) {
                normalized = crate::ai_service::tts::chinese_digits::normalize(text);
                &normalized
            } else {
                text
            };

        // 汉字句子按标点自己切句、逐句推理、再拼回去（见 split_clauses）：纯英文台词
        // 不切，英文的标点交给模型自己处理。
        let clauses = if crate::ai_service::tts::chinese_digits::has_han(text) {
            split_clauses(text)
        } else {
            vec![(text.to_owned(), 0.0, None)]
        };

        let mut assembled: Vec<f32> = Vec::new();
        let mut sample_rate = tts.sample_rate();

        for (clause, pause, _) in clauses {
            // 数字改写可能带出分隔符，这里按小句统一清掉读不出来的字符
            // （见 strip_unreadable）。
            let piece_text = strip_unreadable(&clause, self.reads_latin);
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

            // 每片的静音都去掉：停顿由我们按标点精确插入，片自带的静音只会让停顿
            // 忽长忽短；接缝两侧各做一次淡入淡出，避免波形跳变听起来是「咔哒」。
            let edge = strip_edge_silence(audio.samples());
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

fn find_file(dir: &Path, candidates: &[&str]) -> Result<PathBuf> {
    for name in candidates {
        let path = dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "在 {} 中未找到模型文件，候选: {:?}",
        dir.display(),
        candidates
    ))
}

/// 模型目录里可能出现的文本规则 FST，按应用顺序排列。
///
/// 中文模型把规则 FST 放在模型目录里，sherpa 会在分词前依次改写文本（实测
/// `text = tn->Normalize(text)`）：阿拉伯数字「3」先被改写成「三」，再交给 lexicon。
///
/// 不挂这些文件时，中文模型的 CharacterLexicon 对非汉字只能丢词——设备日志里就是
/// `Ignore OOV '3'`，用户听到的是「第 3 章」读成「第 章」。名字与顺序都照上游示例：
/// matcha-zh 是 `phone/date/number.fst`（scripts/matcha-tts/zh/generate_samples.py），
/// kokoro 多语种是 `phone-zh/date-zh/number-zh.fst`（scripts/kokoro/v1.1-zh/…）。
const RULE_FST_ORDER: &[&str] = &[
    "phone.fst",
    "date.fst",
    "number.fst",
    "phone-zh.fst",
    "date-zh.fst",
    "number-zh.fst",
    // 兜底：有的模型只给一个合并好的规则文件。
    "rule_fst.fst",
    "rule_fst.far",
    "rule.fst",
];

/// 收集模型目录里可用的文本规则 FST 路径（顺序同 [`RULE_FST_ORDER`]）。
fn rule_fst_paths(model_dir: &Path) -> Vec<PathBuf> {
    RULE_FST_ORDER
        .iter()
        .filter_map(|name| find_file_optional(model_dir, &[name]))
        .collect()
}

/// 若模型目录内附带文本规则 FST（matcha-zh 的 `number/date/phone.fst`、kokoro 的
/// `*-zh.fst` 等），自动挂载到 `OfflineTtsConfig`，使数字、日期、电话号码在读之前
/// 先被改写成可读文本。
fn with_rule_files(mut config: OfflineTtsConfig, model_dir: &Path) -> OfflineTtsConfig {
    let fsts: Vec<String> = rule_fst_paths(model_dir)
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    if !fsts.is_empty() {
        // sherpa 侧按逗号切分，依次 Normalize。
        config.rule_fsts = Some(fsts.join(","));
    }
    if let Some(far) = find_file_optional(model_dir, &["rulefst.far", "rule.far"]) {
        config.rule_fars = Some(far.to_string_lossy().to_string());
    }
    config
}

/// 这个模型要不要在 Rust 侧自己改写阿拉伯数字（见 [`chinese_digits`]）。
///
/// 模型目录里带了规则 FST 的（matcha-zh、kokoro）交给 sherpa 自己改写——它的读法更
/// 全（还含日期、电话号码规则），不必重复处理；没带的（zipvoice / supertonic 的实现
/// 根本不读 `rule_fsts`，别的中文模型也可能没随包带 FST）就得我们补，否则数字要么被
/// 前端丢词，要么念错。
///
/// 真正动不动手还要看当次文本（纯英文台词不塞汉字数字，见 [`chinese_digits::has_han`]）。
fn needs_digit_normalization(model_dir: &Path) -> bool {
    rule_fst_paths(model_dir).is_empty()
}

/// 模型自己认得拉丁字母 / 日文假名吗——决定汉字句子里的英文、罗马音、假名要不要剔掉。
///
/// 依据是「模型目录里的词表认不认识拉丁字母」：kokoro 多语言版自带英文词表
/// （`lexicon-us-en.txt` / `lexicon-gb-en.txt`），英文片段能直接读；中文 matcha /
/// zipvoice 的词表里一个带字母的词条都没有（实测三张中文表 66k / 68k 条，键里含
/// 拉丁字母的条目数为 0）。
///
/// 先看文件名（省一次读表）：文件名里带独立的一段 `en`（`lexicon-us-en.txt`、
/// `lexicon-zh-en.txt`）就当作认得英文。没有这种文件时再看词表内容——自定义的
/// 中英模型可能只给一个 `lexicon.txt`，键里出现拉丁字母同样说明模型认得。
fn model_reads_latin(model_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(model_dir) else {
        return false;
    };
    let mut unnamed: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(name.starts_with("lexicon") && name.ends_with(".txt")) {
            continue;
        }
        if name.split(['-', '_', '.']).any(|part| part == "en") {
            return true;
        }
        unnamed.push(path);
    }
    // 词表内容里出现拉丁字母的键，同样说明模型自己认得（查到一个就收工）。
    unnamed.iter().any(|path| {
        std::fs::read_to_string(path).is_ok_and(|text| {
            text.lines().any(|line| {
                line.split_whitespace()
                    .next()
                    .is_some_and(|word| word.chars().any(|c| c.is_ascii_alphabetic()))
            })
        })
    })
}

/// 是不是汉字（范围与 `chinese_digits::has_han` 一致）。
fn is_han(c: char) -> bool {
    let cp = c as u32;
    (0x3400..=0x4DBF).contains(&cp)
        || (0x4E00..=0x9FFF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x2_0000..=0x3_FFFF).contains(&cp)
}

/// 送给 sherpa 之前，把「读不出来、还会被硬念出来或带跑整句」的字符换成空格。
///
/// 三件事：
///
/// 1. 从句标点（`，、；：,;:—`）要留着：短句合并后一片里有两个小句（见 [`Clause`]），
///    标点在 sherpa 前端有对应的 token（词表没收录的会退化成 pad），模型据此收语气、
///    给停顿；换成空格它只是个读不出来的空词（日志里 `Ignore OOV ' '`），停顿全靠模型
///    临场发挥。标点把这一片切成多份 token 序列也没关系——[`MAX_NUM_SENTENCES`] 之内
///    仍在一次推理里。
/// 2. 其余符号（`~`、`～`、`♪`、`♡`、`→`、emoji、孤立括号、引号……）既不在词表里、
///    也不是汉字，`MatchaTtsLexicon` 会走 **espeak（en-us）兜底**当英文念出来：ASCII
///    `~` 实测被念成 "tilde"（SenseVoice 反读「好~」得到「好挑的」）。
/// 3. 模型读不了的文字——英文、罗马音、假名——会被同一套 espeak 兜底音素化：中文
///    matcha 的词表里一个英文词都没有，`hello` 这类要么被丢词、要么按字母念；零样本
///    的 zipvoice 更糟，夹一句英文会把整句带跑（SenseVoice 反读里冒出参考音频的内容，
///    听起来就是「换了个人、还念了参考音频里的话」）。所以模型不认得拉丁字母时
///    （见 [`model_reads_latin`]）连同假名一起整段剔掉——宁可少念一句英文，也不要
///    整句走样。纯英文台词不走这里（调用处只在含汉字时才动手），英文由模型自己处理。
fn strip_unreadable(text: &str, keep_latin: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let speakable = is_han(c)
            || c.is_ascii_digit()
            || matches!(c, '０'..='９')
            // 句末标点：切句与停顿由 split_clauses 决定，标点本身留给模型收尾。
            || matches!(c, '。' | '！' | '？' | '.' | '!' | '?')
            // 从句标点：合并短句时放回来的那个（见 Clause），别在这里又抹掉。
            || CLAUSE_PUNCT.contains(&c)
            || (keep_latin
                && (c.is_alphanumeric()
                    // 英文缩写里的撇号，留在词里，别把 don't 拆成两个词。
                    || (matches!(c, '\'' | '’')
                        && i > 0
                        && i + 1 < chars.len()
                        && chars[i - 1].is_ascii_alphanumeric()
                        && chars[i + 1].is_ascii_alphanumeric())));
        if speakable && !c.is_whitespace() {
            // 标点紧贴前面的词：别把符号换出来的空格留在标点前（`别走 ,好嘛`）。
            if (SENTENCE_PUNCT.contains(&c) || CLAUSE_PUNCT.contains(&c)) && out.ends_with(' ') {
                out.pop();
            }
            out.push(c);
        } else if !out.ends_with(' ') {
            // 连续符号/空白只留一个空格，免得前端切出多个空词。
            out.push(' ');
        }
    }
    out.trim().to_string()
}

/// 小句太短时不要单独送进模型：实测两字的「对了」单独合成，SenseVoice 反读成
/// 「饿了」（同一句里连着念就是对的）——短句缺少上下文，模型容易读错、读得生硬。
/// 短于这个字数的先并到下一句里（末尾实在只剩一句短的，就并回前一句）。
///
/// 合并处那个标点会原样放回文本（见 [`Clause`] / [`merge_short_clauses`]），所以
/// 停顿不会被吃掉；实测同样一句「喂？你在听吗？」，并成「喂？你在听吗？」（标点留着）
/// 比并成「喂 你在听吗？」（空格）读得准——空格那版 SenseVoice 把「喂」听成「问」。
const MIN_CLAUSE_CHARS: usize = 3;

/// 情绪 →（语速倍率，停顿倍率）。
///
/// sherpa-onnx 的这几个引擎里只有 `vits` 能接 `emotion_id`，而且还要模型导出时带
/// `emotion_id` 输入 + `num_emotions` 元数据；matcha / kokoro / zipvoice 完全没有
/// 情绪入口。所以在这些模型上，情绪只能落成**不动音色**的两个参数：语速和标点
/// 停顿。倍率都收在 ±12% / ±25% 内，避免听成「变速播放」。
///
/// 分类沿用 GSV 的六分类（[`gsv_emo_category`]，与提示词里的情绪清单一致）：
/// 吃惊 / 开心 / 恐惧 / 难过 / 生气 / 中立；未识别的情绪一律按中立。
fn emotion_prosody(emo: &str) -> (f32, f32) {
    // 六分类名本身（「开心」「恐惧」…）也是合法输入：提示词给的是「高兴 / 害怕 /
    // 惊讶」这类同义词，但分类器与立绘槽位会直接给分类名，所以先原样认一次。
    let category = match emo {
        "吃惊" | "开心" | "恐惧" | "难过" | "生气" | "中立" => emo,
        other => gsv_emo_category(other),
    };

    match category {
        // 情绪越高涨，语速越快、标点停顿越短。
        "开心" => (1.05, 0.90),
        "生气" => (1.08, 0.80),
        "恐惧" => (1.10, 0.80),
        "吃惊" => (1.08, 0.75),
        // 低落时放慢、句读拉长。
        "难过" => (0.90, 1.25),
        _ => (1.0, 1.0),
    }
}

/// 切出来的小句：文本、之后停多久（秒）、以及**被切掉的那个从句标点**（没有就是
/// `None`）。
///
/// 标点要单独留一份，是因为短句合并时得把它放回文本里（见 [`merge_short_clauses`]）：
/// 直接从文本里丢掉，合并处就只剩一个空格——那在 sherpa 前端是个读不出来的空词，
/// 模型既读不到标点、也拿不到标点该有的语气。句末标点（`。！？`）本来就写在小句里，
/// 所以这里只记从句标点。
type Clause = (String, f32, Option<char>);

/// 按标点把文本切成小句，并给出每句之后该停多久（秒；最后一句是 0）。
///
/// 为什么要自己切：sherpa 自己也会按标点把文本切成若干份 token 序列送进模型，但
/// **拼接处是硬接**——`offline-tts-matcha-impl.h`（v1.13.5）里多句/多批的结果就是
/// `ans.samples.insert(ans.samples.end(), ...)`，既没有淡入淡出也没有额外静音；两截
/// 波形在接缝处幅值不连续，听感上就是「咔哒」一声（零样本模型接缝两侧音色也会微微
/// 跳）。自己切之后每处停顿长度可控，接缝两侧再各做一次淡入淡出（见
/// [`apply_edge_fades`]）。
///
/// 数字/字母**中间**的标点不算断句：`9:30`、`3.14`、`1,000`、`www.bing.com` 都要整块
/// 念（模型自带的 rule FST 也认这些写法，见 [`crate::ai_service::tts::chinese_digits`]）。
/// 判定只看前后是不是紧挨着的 ASCII 字母数字，所以全角的 `1，000` 一样不会在那里切。
fn split_clauses(text: &str) -> Vec<Clause> {
    let chars: Vec<char> = text.chars().collect();
    let mut raw: Vec<Clause> = Vec::new();
    let mut cur = String::new();

    for (i, &c) in chars.iter().enumerate() {
        let inside_word = i > 0
            && chars[i - 1].is_ascii_alphanumeric()
            && chars.get(i + 1).is_some_and(|n| n.is_ascii_alphanumeric());

        if SENTENCE_PUNCT.contains(&c) {
            cur.push(c);
            if !inside_word {
                raw.push((std::mem::take(&mut cur), SENTENCE_PAUSE, None));
            }
        } else if CLAUSE_PUNCT.contains(&c) {
            if inside_word {
                cur.push(c);
            } else {
                raw.push((std::mem::take(&mut cur), CLAUSE_PAUSE, Some(c)));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        raw.push((cur, 0.0, None));
    }

    // 连写的标点（「真的？！」）会切出空片段，只剩标点的片段（「！」「……」）也一起
    // 去掉；整段末尾不留停顿。
    raw.retain(|(clause, _, _)| clause.chars().any(char::is_alphanumeric));
    if let Some(last) = raw.last_mut() {
        last.1 = 0.0;
    }

    merge_short_clauses(raw)
}

/// 可念的字数（汉字/字母/数字），不算标点与空白。
fn speakable_len(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

/// 把短于 [`MIN_CLAUSE_CHARS`] 的小句并到下一句（末尾的短句并回前一句）。
///
/// 合并处原来那个标点要放回去（见 [`Clause`]）：留着它 sherpa 就把这一片切成两份
/// token 序列——只要不撞上 [`MAX_NUM_SENTENCES`] 就还是一次推理——模型既读得到标点
/// 带来的语气和停顿，短句也从后半句拿到了上下文。
fn merge_short_clauses(raw: Vec<Clause>) -> Vec<Clause> {
    let mut out: Vec<Clause> = Vec::new();
    let mut pending: Option<Clause> = None;

    for (clause, pause, tail_punct) in raw {
        let merged = match pending.take() {
            Some((prev, _, prev_punct)) => {
                (join_clauses(&prev, prev_punct, &clause), pause, tail_punct)
            },
            None => (clause, pause, tail_punct),
        };
        if speakable_len(&merged.0) < MIN_CLAUSE_CHARS {
            pending = Some(merged);
        } else {
            out.push(merged);
        }
    }

    if let Some((tail, _, _)) = pending {
        match out.last_mut() {
            Some(last) => {
                last.0 = join_clauses(&last.0, last.2, &tail);
                last.2 = None;
                last.1 = 0.0; // 并完就是整段最后一句了，后面不留停顿
            },
            None => out.push((tail, 0.0, None)),
        }
    }
    out
}

/// 合并两个小句：把前一句被切掉的标点放回去，而不是塞一个空格。
fn join_clauses(prev: &str, prev_punct: Option<char>, next: &str) -> String {
    match prev_punct {
        Some(punct) => format!("{prev}{punct}{next}"),
        None => format!("{prev}{next}"),
    }
}

fn find_file_optional(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
}

fn find_dir_optional(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.is_dir())
}

/// 收集 Kokoro 需要的 lexicon 文件（逗号分隔）。
///
/// 顺序与官方示例一致：英文 → 中文，其余 `lexicon*.txt` 兜底追加。
/// 英文只挂一份：`lexicon-us-en.txt` 与 `lexicon-gb-en.txt` 内容高度重叠，
/// 同时挂载会刷出上千行 "Duplicated word ... Ignore it" 告警。
///
/// Kokoro 不挂 lexicon 时中文只能退到 espeak-ng 的音素化，实测合成中文几乎出不来
/// 有效音频（只有 0.2~0.5 秒的噪声或纯静音），因此这里必须显式挂载。
fn kokoro_lexicons(model_dir: &Path) -> Option<String> {
    const ENGLISH: &[&str] = &["lexicon-us-en.txt", "lexicon-gb-en.txt"];
    const CHINESE: &[&str] = &["lexicon-zh.txt"];

    let mut files: Vec<PathBuf> = Vec::new();
    for group in [ENGLISH, CHINESE] {
        if let Some(path) = find_file_optional(model_dir, group) {
            files.push(path);
        }
    }

    // 自定义模型可能只带 lexicon.txt 之类，兜底扫描目录内其余 lexicon*.txt。
    if let Ok(entries) = std::fs::read_dir(model_dir) {
        let mut extra: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("lexicon") && name.ends_with(".txt"))
                    && !files.contains(path)
                    && !ENGLISH.iter().any(|name| path.ends_with(name))
            })
            .collect();
        extra.sort();
        files.extend(extra);
    }

    if files.is_empty() {
        return None;
    }
    Some(
        files
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// 定位 ZipVoice 数据目录：该目录需直接包含 phontab/phondata/phonindex。
/// 优先匹配目录本身或其 `espeak-ng-data` 子目录，其次匹配模型目录。
fn zipvoice_data_dir(model_dir: &Path) -> Option<PathBuf> {
    if model_dir.join("phontab").exists() {
        return Some(model_dir.to_path_buf());
    }
    if find_dir_optional(model_dir, &["espeak-ng-data"]).is_some_and(|p| p.join("phontab").exists())
    {
        return find_dir_optional(model_dir, &["espeak-ng-data"]);
    }
    None
}

/// 随包的 ZipVoice 中英音素表（`字/词 → 声母0 韵母…`）在 data 目录下的相对路径。
///
/// 为什么必须随包带一份：sherpa-onnx 的 `MatchaTtsLexicon` 在 lexicon 为空时直接
/// `SHERPA_ONNX_EXIT(-1)`（= `_Exit(-1)`，进程以 255 静默退出，Rust 侧捕不到），
/// 而 ZipVoice 的 `InitFrontend()` 必然走这个类。上游官方包里的 `lexicon.txt` 并不是
/// 模型自带文件，而是 CI 打包时用 `scripts/zipvoice/zh-en/generate_lexicon.py`
/// （pypinyin + 中英音素转换）现场生成的；我们用的 ModelScope 镜像包
/// （`gomodels/sherpa`）里只有 `pinyin.raw`、没有该文件，而 `pinyin.raw` 的整音节写法
/// （`好 hao3`）与模型 `tokens.txt` 的「声母+0 / 韵母」方案（`h0 ao3`）不兼容，
/// 不能直接拿来当 lexicon。
///
/// 这个文件即上述脚本的输出（68037 条），音素已逐条校验都在模型 `tokens.txt` 里。
/// 它放在 `data/third_party/` 下随包发布（桌面端进 `bundle.resources`、移动端进
/// `data.7z`），而不是 `include_str!` 进二进制——1.7MB 文本只有 ZipVoice 用得上，
/// 没必要让每个构建产物都背上它。
const ZIPVOICE_LEXICON_REL_PATH: &str = "third_party/zipvoice/lexicon-zh-en.txt";

/// 读取随包的中英音素表，行尾统一成 LF。
///
/// 行尾必须干净：`\r` 会被当成音素的一部分而全部查不到（`.gitattributes` 已固定
/// 该文件 LF，这里再兜一层——手工拷贝过的副本可能不是）。
fn bundled_zipvoice_lexicon(data_dir: &Path) -> Result<String> {
    let path = data_dir.join(ZIPVOICE_LEXICON_REL_PATH);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        anyhow!(
            "读取随包 ZipVoice 音素表失败（{}）: {e}——安装包不完整？",
            path.display()
        )
    })?;
    Ok(if text.contains('\r') {
        text.replace("\r\n", "\n")
    } else {
        text
    })
}

/// 随包资源所在的 data 目录。
///
/// 正常路径是 [`crate::data_dir::get_data_dir`]（App 启动时初始化）；单测/集成测试
/// 直接构造适配器、不走启动流程，这里退回仓库里的 `data/`（与 [`crate::data_dir`]
/// 在 debug 模式下的解析一致）。
fn bundled_assets_data_dir() -> PathBuf {
    crate::data_dir::try_get_data_dir()
        .cloned()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
}

/// 定位 ZipVoice 的 lexicon：优先模型目录自带的，缺失时写入随包的中英音素表。
///
/// 写回模型目录而不是临时目录，是为了让模型目录自洽（其它工具、以及下次启动
/// 都直接命中），而且只需要写一次。
fn zipvoice_lexicon(model_dir: &Path) -> Result<PathBuf> {
    if let Some(path) = find_file_optional(model_dir, &["lexicon.txt"]) {
        // 0 字节的 lexicon 等于没有：sherpa 只在「路径为空」时报错退出，空文件则
        // 让每个字都变成 OOV（念不出声），所以这里也按缺失处理。
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 0 {
            return Ok(path);
        }
        tracing::warn!(
            "ZipVoice 模型自带的 {} 为空，改用随包中英音素表",
            path.display()
        );
    }

    let text = bundled_zipvoice_lexicon(&bundled_assets_data_dir())?;
    if !zipvoice_lexicon_matches_tokens(model_dir, &text) {
        return Err(anyhow!(
            "ZipVoice 模型缺少 lexicon.txt，且随包中英音素表与 {} 里的 tokens.txt 不匹配（不是同一个中英模型）",
            model_dir.display()
        ));
    }

    let path = model_dir.join("lexicon.txt");
    std::fs::write(&path, text.as_bytes())
        .map_err(|e| anyhow!("写入 ZipVoice lexicon.txt 失败（{}）: {e}", path.display()))?;
    tracing::info!(
        "ZipVoice 模型缺少 lexicon.txt（上游由 CI 生成），已写入随包中英音素表: {}",
        path.display()
    );
    Ok(path)
}

/// 随包音素表的音素是否都被模型 `tokens.txt` 收录。
///
/// 防止把中英音素表塞给别的 ZipVoice 模型（例如纯英文模型）——那种情况下宁可报错，
/// 也不要念出一堆 OOV。
fn zipvoice_lexicon_matches_tokens(model_dir: &Path, lexicon: &str) -> bool {
    let Ok(tokens) = std::fs::read_to_string(model_dir.join("tokens.txt")) else {
        return false;
    };
    let tokens: HashSet<&str> = tokens
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    lexicon
        .lines()
        .all(|line| line.split_whitespace().skip(1).all(|p| tokens.contains(p)))
}

/// 首尾静音的边界 `[start, end)`（整段都静音时 `start >= end`）。
fn silence_bounds(samples: &[f32]) -> (usize, usize) {
    let mut start = 0;
    while start < samples.len() && samples[start].abs() <= SILENCE_AMP {
        start += 1;
    }
    let mut end = samples.len();
    while end > start && samples[end - 1].abs() <= SILENCE_AMP {
        end -= 1;
    }
    (start, end)
}

/// 去掉首尾**全部**静音（不留余量）。
///
/// 逐句合成时片与片之间要插我们自己算好的停顿，各片自带的静音（0.1~0.4 秒不等，
/// 模型之间还不一样）只会让停顿忽长忽短，所以这里一点不留；整段首尾的静音由
/// 调用处按 [`LEAD_SILENCE_KEEP`] / [`TRAIL_SILENCE_KEEP`] 补回来。
fn strip_edge_silence(samples: &[f32]) -> &[f32] {
    let (start, end) = silence_bounds(samples);
    &samples[start..end]
}

/// 给一段音频的首尾各加一小段线性淡入/淡出（时长见 [`JOINT_FADE`]）。
///
/// 拼接处需要它：两截波形在接缝处的幅值一般不连续（sherpa 内部就是直接把多段
/// `insert` 到一起的），听感上就是「咔哒」一声。淡入淡出把接缝两侧压到 0，
/// 接缝就落回静音里了。
fn apply_edge_fades(samples: &mut [f32], sample_rate: i32) {
    if sample_rate <= 0 {
        return;
    }
    let fade = ((sample_rate as f32 * JOINT_FADE) as usize).min(samples.len() / 2);
    for i in 0..fade {
        let gain = i as f32 / fade as f32;
        samples[i] *= gain;
        let last = samples.len() - 1 - i;
        samples[last] *= gain;
    }
}

fn f32_to_wav(samples: &[f32], sample_rate: i32) -> Vec<u8> {
    let num_samples = samples.len();
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;
    let sr = sample_rate as u32;
    let byte_rate = sr * 2;

    let mut wav = Vec::with_capacity(44 + data_size);

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(file_size as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sr.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());

    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data_size as u32).to_le_bytes());

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        wav.extend_from_slice(&i16_sample.to_le_bytes());
    }

    wav
}

pub fn load_reference_audio(path: &Path) -> Result<(Vec<f32>, i32)> {
    // Android 的 content:// URI（早期版本直接存进设置的路径）读不了，早点说清楚
    // 该怎么办，别让用户看到下面那句「仅支持 WAV」而摸不着头脑。
    if path.to_string_lossy().starts_with("content://") {
        return Err(anyhow!(
            "参考音频是 Android 的 content:// URI，请先在语音设置里重新导入一次（会复制到应用数据目录）: {}",
            path.display()
        ));
    }

    // 仅支持 WAV 格式（sherpa-onnx 零样本克隆要求 WAV 输入）
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "wav" {
        return Err(anyhow!(
            "参考音频仅支持 WAV 格式，当前文件格式: {} ({}). 请转换为 WAV 后重试",
            ext,
            path.display()
        ));
    }

    let data = std::fs::read(path)
        .map_err(|e| anyhow!("无法读取参考音频文件 {}: {}", path.display(), e))?;

    // 按块流解析，而不是按固定偏移：`fmt ` 前面可能排着 `bext`（广播 WAV）、`LIST`、
    // `JUNK` 等块（摄像/录音软件导出很常见），固定偏移在那里读到的是无关字节。
    let wav = parse_wav(&data)
        .ok_or_else(|| anyhow!("WAV 文件结构不对（缺 fmt / data 块）: {}", path.display()))?;
    let format = wav.format;

    // 采样率要原样交给 sherpa 重采样：读错一位，参考音频就等于被按错误的速度播放，
    // 合成出来音色、语速全变（实测把 22050 的样本标成 48000，时长少一半；标成
    // 16000，时长多三成，音色距离 41~55‰——接近「换了个人」的量级）。
    if !(1..=384_000).contains(&format.sample_rate) {
        return Err(anyhow!(
            "WAV 头里的采样率不对（读到 {}），请用「导入参考音频」重新导入: {}",
            format.sample_rate,
            path.display()
        ));
    }
    if !(1..=8).contains(&format.channels) {
        return Err(anyhow!(
            "WAV 头里的声道数不对（读到 {}）: {}",
            format.channels,
            path.display()
        ));
    }

    let samples = decode_wav_samples(&data[wav.offset..wav.offset + wav.len], &format)
        .map_err(|e| anyhow!("{e}: {}", path.display()))?;

    // 多声道取平均。chunks_exact：末尾不足一帧的残字节丢掉，不能让它们单独成一帧。
    let mono_samples = if format.channels > 1 {
        samples
            .chunks_exact(format.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / format.channels as f32)
            .collect()
    } else {
        samples
    };

    if mono_samples.is_empty() {
        return Err(anyhow!("参考音频为空"));
    }

    Ok((mono_samples, format.sample_rate))
}

/// WAV 的 `fmt ` 块内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WavFormat {
    /// 1 = PCM，3 = IEEE float（`0xFFFE` = WAVE_FORMAT_EXTENSIBLE 已按子格式解开）。
    format_tag: u16,
    channels: u16,
    sample_rate: i32,
    bits_per_sample: u16,
}

/// 解析出来的 WAV：格式 + `data` 块在文件里的位置。
struct WavData {
    format: WavFormat,
    /// `data` 块载荷的起始偏移。
    offset: usize,
    /// `data` 块载荷的字节数（以文件实际长度收口）。
    len: usize,
}

/// 遍历 RIFF 块，定位 `fmt ` 与 `data`。
fn parse_wav(data: &[u8]) -> Option<WavData> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }

    let mut format = None;
    let mut audio: Option<(usize, usize)> = None;
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let id: [u8; 4] = data[pos..pos + 4].try_into().ok()?;
        let declared = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body = pos + 8;
        // 截断的文件里块长度可能超过实际剩余字节，按实际长度收口。
        let end = body.saturating_add(declared).min(data.len());
        match &id {
            b"fmt " => format = parse_fmt_chunk(&data[body..end]),
            b"data" => audio = Some((body, end - body)),
            _ => {},
        }
        // RIFF 块按偶数字节对齐。
        pos = end.saturating_add(declared & 1).max(body);
    }

    Some(WavData {
        format: format?,
        offset: audio?.0,
        len: audio?.1,
    })
}

fn parse_fmt_chunk(body: &[u8]) -> Option<WavFormat> {
    if body.len() < 16 {
        return None;
    }
    let mut format_tag = u16::from_le_bytes([body[0], body[1]]);
    let channels = u16::from_le_bytes([body[2], body[3]]);
    let sample_rate = i32::from_le_bytes([body[4], body[5], body[6], body[7]]);
    let bits_per_sample = u16::from_le_bytes([body[14], body[15]]);
    // WAVE_FORMAT_EXTENSIBLE（24 位/多声道常见）：真正的编码在 fmt 块尾部
    // SubFormat GUID 的前两个字节。
    if format_tag == 0xFFFE && body.len() >= 26 {
        format_tag = u16::from_le_bytes([body[24], body[25]]);
    }
    Some(WavFormat {
        format_tag,
        channels,
        sample_rate,
        bits_per_sample,
    })
}

/// 把 WAV 的样本数据解码成 -1.0~1.0 的 f32。
fn decode_wav_samples(raw: &[u8], format: &WavFormat) -> Result<Vec<f32>> {
    const PCM: u16 = 1;
    const FLOAT: u16 = 3;

    let samples: Vec<f32> = match (format.format_tag, format.bits_per_sample) {
        // 8 位 WAV 是无符号 PCM，中点 128。
        (PCM, 8) => raw.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect(),
        (PCM, 16) => raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0)
            .collect(),
        (PCM, 24) => raw
            .as_chunks::<3>()
            .0
            .iter()
            .map(|b| {
                // 24 位小端、手工符号扩展。
                let v = ((b[2] as i8 as i32) << 16) | ((b[1] as i32) << 8) | b[0] as i32;
                v as f32 / 8_388_608.0
            })
            .collect(),
        (PCM, 32) => raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0)
            .collect(),
        (FLOAT, 32) => raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect(),
        (FLOAT, 64) => raw
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| f64::from_le_bytes(*b) as f32)
            .collect(),
        (tag, bits) => {
            return Err(anyhow!(
                "不支持的 WAV 编码（format tag = {tag}、位深 = {bits} 位；支持 PCM 8/16/24/32 与 float 32/64）"
            ));
        },
    };
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model_dir() -> String {
        std::env::var("SHERPA_TEST_MODEL_DIR").unwrap_or_default()
    }

    /// 规则 FST 的收集顺序，以及「模型目录里有没有 FST」这个判断。
    #[test]
    fn collects_rule_fsts_in_order() {
        let dir = std::env::temp_dir().join(format!("lingchat-digit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录失败");

        // 没带 FST 的模型要由我们在 Rust 侧改写数字：zipvoice / supertonic 的实现
        // 不读 rule_fsts，数字会被丢词或念错。
        assert!(needs_digit_normalization(&dir));
        assert!(rule_fst_paths(&dir).is_empty());

        // 顺序照上游示例：phone → date → number。
        std::fs::write(dir.join("number.fst"), b"stub").expect("写占位 FST 失败");
        std::fs::write(dir.join("phone.fst"), b"stub").expect("写占位 FST 失败");
        std::fs::write(dir.join("date.fst"), b"stub").expect("写占位 FST 失败");
        let names: Vec<String> = rule_fst_paths(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, ["phone.fst", "date.fst", "number.fst"]);
        assert!(
            !needs_digit_normalization(&dir),
            "带 FST 的交给 sherpa 自己改写"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 符号换成空格：句末标点保留，从句标点（逗号/顿号/分号/冒号）也保留（短句合并
    /// 之后要靠它给停顿），`~`/`♪`/emoji 不再被 espeak 兜底念成英文（ASCII `~` 会被
    /// 念成 "tilde"）；模型读不了的英文/假名整段剔掉。
    #[test]
    fn strips_unreadable_chars_but_keeps_sentence_ends() {
        // 认得拉丁字母的模型（kokoro 多语言）：英文原样留着，标点都留着。
        let latin = true;
        assert_eq!(
            strip_unreadable("他说，你好、今天：很好；真的？", latin),
            "他说，你好、今天：很好；真的？"
        );
        // 句末标点原样保留（切句与停顿由 split_clauses 负责）。
        assert_eq!(
            strip_unreadable("你好。真的吗！好呀？", latin),
            "你好。真的吗！好呀？"
        );
        // 波浪号、音乐符号、箭头、装饰符号一律换成空格。
        assert_eq!(strip_unreadable("好~呀", latin), "好 呀");
        assert_eq!(strip_unreadable("好～♪♡→呀", latin), "好 呀");
        // 英文缩写里的撇号要留着，别把 don't 拆开。
        assert_eq!(strip_unreadable("I don't ~know~", latin), "I don't know");
        // 连续符号只留一个空格；符号换出来的空格不留在标点前面。
        assert_eq!(
            strip_unreadable("他说,\"别走\",好嘛.", latin),
            "他说, 别走,好嘛."
        );

        // 中文模型（matcha / zipvoice）读不了英文、罗马音、假名：连着符号一起剔掉，
        // 否则要么被丢词，要么被 espeak 按字母念、把整句带跑。
        let no_latin = false;
        assert_eq!(strip_unreadable("今天真开心 hello", no_latin), "今天真开心");
        assert_eq!(strip_unreadable("hello 今天真开心", no_latin), "今天真开心");
        assert_eq!(
            strip_unreadable("おはよう 今日はいい天気", no_latin),
            "今日 天気"
        );
        assert_eq!(
            strip_unreadable("AI 帮你查了 wiki，好呀~", no_latin),
            "帮你查了，好呀"
        );
        // 数字和标点必须留着：数字交给 FST / 数字改写，标点决定切句与停顿
        // （从句标点是短句合并时放回来的，绝不能在这里又抹掉）。
        assert_eq!(strip_unreadable("第 3 章。", no_latin), "第 3 章。");
        assert_eq!(strip_unreadable("嗯，好的呀", no_latin), "嗯，好的呀");
    }

    /// 按标点切小句时，句末停顿要长于逗号停顿，末尾不留停顿，空片段丢掉；太短的小句
    /// 要并到下一句（「对了」单独念会被读成「饿了」）、且合并处的标点要放回去；数字/
    /// 字母中间的标点不算断句（`9:30`、`3.14`、`1,000`）。
    #[test]
    fn splits_clauses_with_pauses() {
        assert_eq!(
            split_clauses("他说今天很好，你好呀我的朋友。今天天气真好，我们一起出去玩吧！"),
            vec![
                ("他说今天很好".to_string(), CLAUSE_PAUSE, Some('，')),
                ("你好呀我的朋友。".to_string(), SENTENCE_PAUSE, None),
                ("今天天气真好".to_string(), CLAUSE_PAUSE, Some('，')),
                ("我们一起出去玩吧！".to_string(), 0.0, None),
            ]
        );
        // 两字的小句没有上下文，并到下一句；合并处的标点原样放回去，停顿才不会丢。
        assert_eq!(
            split_clauses("对了，你吃饭了吗？"),
            vec![("对了，你吃饭了吗？".to_string(), 0.0, None)]
        );
        // 末尾剩一句短的：并回前一句。
        assert_eq!(
            split_clauses("今天天气真不错。好吧。"),
            vec![("今天天气真不错。好吧。".to_string(), 0.0, None)]
        );
        // 3 字以上就单独成句、保留标点停顿（阈值取 6 会把这些并成一片，标点处全没停顿）。
        assert_eq!(
            split_clauses("嗯，好的呀，那我们走吧。"),
            vec![
                ("嗯，好的呀".to_string(), CLAUSE_PAUSE, Some('，')),
                ("那我们走吧。".to_string(), 0.0, None),
            ]
        );
        assert_eq!(
            split_clauses("我觉得，还是算了吧。"),
            vec![
                ("我觉得".to_string(), CLAUSE_PAUSE, Some('，')),
                ("还是算了吧。".to_string(), 0.0, None),
            ]
        );
        // 没有标点的整句只有一片。
        assert_eq!(
            split_clauses("你好呀"),
            vec![("你好呀".to_string(), 0.0, None)]
        );
        // 连写标点切出来的空片段（只剩「！」）要丢掉。
        assert_eq!(
            split_clauses("真的？！"),
            vec![("真的？".to_string(), 0.0, None)]
        );

        // 数字/字母中间的标点不算断句：时刻、小数、千分位、域名都要整块念
        // （以前 `9:30` 会在冒号处切一刀，停顿插进时间中间，读成「上午九，三十开会」）。
        assert_eq!(
            split_clauses("上午 9:30 开会，别迟到了。"),
            vec![
                ("上午 9:30 开会".to_string(), CLAUSE_PAUSE, Some('，')),
                ("别迟到了。".to_string(), 0.0, None),
            ]
        );
        assert_eq!(
            split_clauses("圆周率是 3.14。"),
            vec![("圆周率是 3.14。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("一共 1,000 块，别忘了。"),
            vec![
                ("一共 1,000 块".to_string(), CLAUSE_PAUSE, Some('，')),
                ("别忘了。".to_string(), 0.0, None),
            ]
        );
        // 末尾的短句并回前一句时，那句被切掉的逗号同样要放回去。
        assert_eq!(
            split_clauses("一共 1,000 块，收好。"),
            vec![("一共 1,000 块，收好。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("我打开了 www.bing.com。"),
            vec![("我打开了 www.bing.com。".to_string(), 0.0, None)]
        );
    }

    /// 情绪只落在语速/停顿两个参数上（这些模型没有 emotion 输入），分类走 GSV 六分类。
    #[test]
    fn maps_emotion_to_prosody() {
        // 中立与未识别（含空标签）不改变任何参数。
        for emo in ["", "中立", "平静", "正常", "疑惑"] {
            assert_eq!(emotion_prosody(emo), (1.0, 1.0), "{emo}");
        }
        // 高涨：语速更快、标点停顿更短。
        for emo in ["开心", "生气", "恐惧", "吃惊"] {
            let (speed, pause) = emotion_prosody(emo);
            assert!(speed > 1.0 && pause < 1.0, "{emo}: {speed}/{pause}");
        }
        // 低落：语速更慢、标点停顿更长。
        let (speed, pause) = emotion_prosody("难过");
        assert!(speed < 1.0 && pause > 1.0, "难过: {speed}/{pause}");
        // 同义词归到同一类，避免提示词/分类器的不同叫法失效。
        assert_eq!(emotion_prosody("高兴"), emotion_prosody("开心"));
        assert_eq!(emotion_prosody("害怕"), emotion_prosody("恐惧"));
        assert_eq!(emotion_prosody("惊讶"), emotion_prosody("吃惊"));
        assert_eq!(emotion_prosody("哭泣"), emotion_prosody("难过"));
    }

    /// 拼接处的淡入淡出：首尾样本归零、中间不动，20ms 以外不受影响。
    #[test]
    fn fades_clause_edges_and_strips_silence() {
        let sample_rate = 1000; // 20ms = 20 个样本
        let mut samples = vec![1.0f32; 100];
        apply_edge_fades(&mut samples, sample_rate);
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[99], 0.0);
        assert!(samples[10] < 1.0 && samples[19] < 1.0);
        assert_eq!(samples[20], 1.0, "20ms 之后的样本不该被动");
        assert_eq!(samples[50], 1.0);

        // 首尾静音要全部去掉（停顿由调用处按标点精确插入）。
        let mut padded = vec![0.0; 10];
        padded.extend_from_slice(&[0.5; 5]);
        padded.extend_from_slice(&[0.0; 10]);
        let edge = strip_edge_silence(&padded);
        assert_eq!(edge.len(), 5);
        assert!(edge.iter().all(|s| *s == 0.5));
    }

    /// WAV 头必须按块流解析：`fmt ` 前面多一个块（广播 WAV 的 `bext`、`LIST`）时，
    /// 固定偏移读到的是无关字节（实测 44100Hz 被读成 sr=808464432）。
    #[test]
    fn parses_wav_with_chunks_before_fmt() {
        let data = build_wav(&[
            (b"bext", &[b'0'; 602]),
            (b"fmt ", &pcm16_fmt(44_100, 1)),
            (b"data", &[0u8; 8]),
        ]);
        let parsed = parse_wav(&data).expect("应能解析");
        assert_eq!(parsed.format.sample_rate, 44_100);
        assert_eq!(parsed.format.channels, 1);
        assert_eq!(parsed.format.bits_per_sample, 16);
        assert_eq!(parsed.len, 8);

        // 标准头（fmt 在最前）同样要对。
        let data = build_wav(&[(b"fmt ", &pcm16_fmt(22_050, 2)), (b"data", &[0u8; 16])]);
        let parsed = parse_wav(&data).expect("应能解析");
        assert_eq!(parsed.format.sample_rate, 22_050);
        assert_eq!(parsed.format.channels, 2);
    }

    /// 参考音频的样本格式：16/24/32 位 PCM 与 32 位 float 都要按对的解释读。
    #[test]
    fn decodes_reference_audio_formats() {
        // 24 位：0x7FFFFF 是满幅正，0x800000 是满幅负。
        let raw = [0xFFu8, 0xFF, 0x7F, 0x00, 0x00, 0x80];
        let fmt = WavFormat {
            format_tag: 1,
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 24,
        };
        let samples = decode_wav_samples(&raw, &fmt).expect("24 位应能解码");
        assert!((samples[0] - 1.0).abs() < 1e-6);
        assert!((samples[1] + 1.0).abs() < 1e-6);

        // float32 的位模式按 float 读，而不是按 int32 读（按 int32 读会得到畸变波形）。
        let raw: Vec<u8> = [0.25f32, -0.5]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let fmt = WavFormat {
            format_tag: 3,
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 32,
        };
        let samples = decode_wav_samples(&raw, &fmt).expect("float32 应能解码");
        assert_eq!(samples, vec![0.25, -0.5]);

        // 不认识的编码要报错，别把 ADPCM 当 PCM 读。
        let fmt = WavFormat {
            format_tag: 2,
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 4,
        };
        assert!(decode_wav_samples(&raw, &fmt).is_err());
    }

    /// Android 早期版本存下来的 `content://` 路径要给出可操作的提示。
    #[test]
    fn rejects_content_uri_reference() {
        let err = load_reference_audio(Path::new("content://media/external/audio/1.wav"))
            .expect_err("content:// 应直接报错");
        assert!(err.to_string().contains("重新导入"), "{err}");
    }

    /// 只带简单规则表的模型目录：有英文词表就认为认得拉丁字母。
    #[test]
    fn detects_models_that_read_latin() {
        let dir = std::env::temp_dir().join(format!("lingchat-latin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录失败");

        // 没有词表 / 词表里只有汉字：读不了拉丁字母。
        assert!(!model_reads_latin(&dir));
        std::fs::write(dir.join("lexicon.txt"), "好 h0 ao3\n行 xing2\n").expect("写词表失败");
        assert!(!model_reads_latin(&dir));

        // kokoro 那种独立的英文词表：文件名带 -en 就算认得。
        std::fs::write(dir.join("lexicon-us-en.txt"), "hello HH AH0 L OW1\n")
            .expect("写英文词表失败");
        assert!(model_reads_latin(&dir));

        // 只给 lexicon.txt 的自定义中英模型：键里有拉丁字母同样算认得。
        std::fs::remove_file(dir.join("lexicon-us-en.txt")).expect("删英文词表失败");
        std::fs::write(dir.join("lexicon.txt"), "hello hh ah l ow\n").expect("写词表失败");
        assert!(model_reads_latin(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 拼一个最简 WAV（只有 RIFF 头 + 给定块序列）。
    fn build_wav(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut body = b"WAVE".to_vec();
        for (id, payload) in chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            body.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&(body.len() as u32).to_le_bytes());
        wav.extend_from_slice(&body);
        wav
    }

    /// 16 位 PCM 的 fmt 块内容。
    fn pcm16_fmt(sample_rate: u32, channels: u16) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes()); // PCM
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&sample_rate.to_le_bytes());
        fmt.extend_from_slice(&(sample_rate * 2 * channels as u32).to_le_bytes());
        fmt.extend_from_slice(&(2 * channels).to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        fmt
    }

    /// 解析我们生成的 WAV 字节并返回 (采样率, 单声道样本数)。
    fn parse_generated_wav(wav: &[u8]) -> (u32, usize) {
        let parsed = parse_wav(wav).expect("缺 RIFF/WAVE/fmt /data 头");
        assert_eq!(parsed.format.format_tag, 1, "输出应是 PCM");
        assert_eq!(parsed.format.channels, 1, "输出应是单声道");
        assert_eq!(parsed.format.bits_per_sample, 16);
        assert_eq!(wav.len(), 44 + parsed.len, "WAV 里有多余字节");
        (parsed.format.sample_rate as u32, parsed.len / 2)
    }

    /// 仓库里的 data 目录：单测不走 App 启动流程，`data_dir` 没初始化。
    fn test_data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    /// 造一个「假 ZipVoice 模型目录」：只会写 tokens.txt。
    ///
    /// `tokens_from_bundled = true` 时用随包音素表的音素生成 tokens.txt（模拟中英
    /// 模型，音素全部命中）；否则只放无关音素（模拟别的 ZipVoice 模型）。
    fn fake_zipvoice_dir(tag: &str, tokens_from_bundled: bool) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lingchat-zipvoice-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时模型目录失败");

        let tokens = if tokens_from_bundled {
            let bundled = bundled_zipvoice_lexicon(&test_data_dir()).expect("读取随包音素表");
            let mut phonemes: Vec<&str> = bundled
                .lines()
                .flat_map(|line| line.split_whitespace().skip(1))
                .collect();
            phonemes.sort_unstable();
            phonemes.dedup();
            phonemes.iter().map(|p| format!("{p} 0\n")).collect()
        } else {
            "x0 0\ny0 1\n".to_string()
        };
        std::fs::write(dir.join("tokens.txt"), tokens).expect("写 tokens.txt 失败");
        dir
    }

    /// 随包音素表读不到（安装包不完整）时报明确的错，而不是静默地念不出声。
    #[test]
    fn zipvoice_missing_bundled_lexicon_errors() {
        let empty =
            std::env::temp_dir().join(format!("lingchat-empty-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&empty);
        std::fs::create_dir_all(&empty).expect("建空目录失败");

        let err = bundled_zipvoice_lexicon(&empty).expect_err("缺文件时应报错");
        assert!(
            err.to_string().contains("读取随包 ZipVoice 音素表失败"),
            "错误信息应说明读不到随包音素表: {err}"
        );

        let _ = std::fs::remove_dir_all(&empty);
    }

    /// 模型目录缺 lexicon.txt 时，应写入随包中英音素表并返回该路径。
    #[test]
    fn zipvoice_missing_lexicon_falls_back_to_bundled() {
        let dir = fake_zipvoice_dir("bundled", true);

        let path = zipvoice_lexicon(&dir).expect("应回退到随包音素表");
        assert_eq!(path, dir.join("lexicon.txt"));

        let written = std::fs::read_to_string(&path).expect("读取写入的 lexicon 失败");
        assert_eq!(written, bundled_zipvoice_lexicon(&test_data_dir()).unwrap());
        assert!(
            written.lines().count() > 60_000,
            "随包音素表疑似被截断: {} 行",
            written.lines().count()
        );

        // 已存在（非空）的 lexicon 直接复用，不再覆盖
        std::fs::write(&path, "好 h0 ao3\n").expect("写测试 lexicon 失败");
        assert_eq!(zipvoice_lexicon(&dir).expect("应复用已有 lexicon"), path);
        assert_eq!(
            std::fs::read_to_string(&path).expect("读取 lexicon 失败"),
            "好 h0 ao3\n"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 0 字节的 lexicon.txt 视为缺失（否则每个字都会变成 OOV）。
    #[test]
    fn zipvoice_empty_lexicon_is_replaced() {
        let dir = fake_zipvoice_dir("empty", true);
        std::fs::write(dir.join("lexicon.txt"), "").expect("写空 lexicon 失败");

        let path = zipvoice_lexicon(&dir).expect("空 lexicon 应被随包音素表替换");
        let written = std::fs::read_to_string(&path).expect("读取 lexicon 失败");
        assert!(written.lines().count() > 60_000, "应写入完整随包音素表");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 音素与 tokens.txt 不匹配时不允许写入（宁可报错也不要念出一堆 OOV）。
    #[test]
    fn zipvoice_lexicon_rejected_when_tokens_mismatch() {
        let dir = fake_zipvoice_dir("mismatch", false);

        let err = zipvoice_lexicon(&dir).expect_err("音素不匹配时应报错");
        assert!(
            err.to_string().contains("tokens.txt"),
            "错误信息应说明 tokens.txt 不匹配: {err}"
        );
        assert!(
            !dir.join("lexicon.txt").exists(),
            "不匹配时不应写入 lexicon"
        );

        let _ = std::fs::remove_dir_all(&dir);
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

        let (sr, num_samples) = parse_generated_wav(&wav);
        eprintln!("输出: sr={sr}, 样本数={num_samples}, WAV-Ok");

        // 验证音频非静音
        let parsed = parse_wav(&wav).unwrap();
        let pcm = &wav[parsed.offset..parsed.offset + parsed.len];
        let has_signal = pcm
            .as_chunks::<2>()
            .0
            .iter()
            .any(|c| i16::from_le_bytes(*c).unsigned_abs() > 200);
        assert!(has_signal, "生成音频疑似全静音");

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
            )
            .unwrap_or_else(|e| panic!("use_gpu={use_gpu} 初始化失败: {e}"));

            let wav = adapter
                .generate_voice("你好，世界。这是一段语音合成测试。", "")
                .await
                .expect("生成失败");

            let (sr, samples) = parse_generated_wav(&wav);
            assert_eq!(sr, 22050, "Matcha-zh 采样率应为 22050");
            assert!(samples >= 5, "生成音频过短");

            let parsed = parse_wav(&wav).unwrap();
            let pcm = &wav[parsed.offset..parsed.offset + parsed.len];
            let has_signal = pcm.as_chunks::<2>().0.iter().any(|c| {
                let v = i16::from_le_bytes(*c);
                v.unsigned_abs() > 200
            });
            assert!(has_signal, "use_gpu={use_gpu} 生成音频疑似全静音");
            eprintln!("use_gpu={use_gpu}: sr={sr}, 样本数={samples}, WAV-Ok");
        }
    }
}
