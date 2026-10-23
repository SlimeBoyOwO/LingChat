//! 引擎构造：按模型类型定位文件、填 sherpa 的模型配置，公共部分由 [`build_engine`] 收口。
//!
//! 这里还有「模型目录里有什么」这类探测：`rule_fst_paths`（文本规则 FST）、
//! `model_reads_latin`（词表认不认英文）、`kokoro_lexicons`、`zipvoice_lexicon`。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use sherpa_onnx::{
    OfflineTts, OfflineTtsConfig, OfflineTtsKittenModelConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsMatchaModelConfig, OfflineTtsModelConfig, OfflineTtsPocketModelConfig,
    OfflineTtsSupertonicModelConfig, OfflineTtsVitsModelConfig, OfflineTtsZipvoiceModelConfig,
};

use super::text::is_language;
use super::{MAX_NUM_SENTENCES, SILENCE_SCALE, SherpaTuning};

/// 按模型类型创建引擎。所有实现都只是「找到文件 → 交给 sherpa」，公共配置见
/// [`build_engine`]。
pub(super) fn create_engine(
    model_type: &str,
    model_dir: &Path,
    language: &str,
    provider: Option<&str>,
    tuning: SherpaTuning,
) -> Result<OfflineTts> {
    match model_type {
        "vits" | "fastspeech2" => create_vits_tts(model_dir, provider),
        "matcha" => create_matcha_tts(model_dir, provider),
        "kokoro" => create_kokoro_tts(model_dir, language, provider),
        "kitten" => create_kitten_tts(model_dir, provider),
        "zipvoice" => create_zipvoice_tts(model_dir, provider, tuning.guidance_scale),
        "pocket" => create_pocket_tts(model_dir, provider),
        "supertonic" => create_supertonic_tts(model_dir, provider),
        other => Err(anyhow!("Sherpa-ONNX 不支持的模型类型: {other}")),
    }
}

/// 各模型共用的执行提供方设置（CPU 时为空）。
fn base_model_config(provider: Option<&str>) -> OfflineTtsModelConfig {
    OfflineTtsModelConfig {
        provider: provider.map(str::to_string),
        ..Default::default()
    }
}

/// 各模型共用的引擎配置：
/// - `max_num_sentences`：我们自己切句，见 [`MAX_NUM_SENTENCES`]；
/// - `silence_scale`：停顿缩放，见 [`SILENCE_SCALE`]；
/// - 模型目录里的文本规则 FST（数字/日期/电话改写）。
fn build_engine(model: OfflineTtsModelConfig, model_dir: &Path, label: &str) -> Result<OfflineTts> {
    let config = OfflineTtsConfig {
        model,
        max_num_sentences: MAX_NUM_SENTENCES,
        silence_scale: SILENCE_SCALE,
        ..Default::default()
    };

    OfflineTts::create(&with_rule_files(config, model_dir))
        .ok_or_else(|| anyhow!("Sherpa-ONNX {label} 模型加载失败，请检查模型文件是否完整"))
}

/// VITS / Fastspeech2：`model.onnx` + `tokens.txt`，可选 `lexicon.txt` 与 dict。
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

    build_engine(
        OfflineTtsModelConfig {
            vits: vits_config,
            ..base_model_config(provider)
        },
        model_dir,
        "VITS",
    )
}

/// 返回 `espeak-ng-data` 目录路径（即 data_dir 的取值）。
fn espeak_data_dir(model_dir: &Path) -> Option<PathBuf> {
    find_dir_optional(model_dir, &["espeak-ng-data"])
}

/// Matcha：声学模型 + vocoder（22k/16k 的 vocos 或 hifigan）。
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
        data_dir: espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
        noise_scale: 0.667,
        length_scale: 1.0,
        dict_dir: find_dir_optional(model_dir, &["dict"]).map(|p| p.to_string_lossy().to_string()),
    };

    build_engine(
        OfflineTtsModelConfig {
            matcha: matcha_config,
            ..base_model_config(provider)
        },
        model_dir,
        "Matcha",
    )
}

/// Kokoro 多语言：`voices.bin` 选音色，lexicon 决定读得出哪些文字。
///
/// `lang` 只给日文：其余语言交给模型自己判断（见函数内注释）。
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
        data_dir: espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
        length_scale: 1.0,
        dict_dir: find_dir_optional(model_dir, &["dict"]).map(|p| p.to_string_lossy().to_string()),
        lexicon: kokoro_lexicons(model_dir),
        lang,
    };

    build_engine(
        OfflineTtsModelConfig {
            kokoro: kokoro_config,
            ..base_model_config(provider)
        },
        model_dir,
        "Kokoro",
    )
}

/// ZipVoice 零样本克隆：encoder/decoder/vocoder + espeak 数据目录 + 音素表。
fn create_zipvoice_tts(
    model_dir: &Path,
    provider: Option<&str>,
    guidance_scale: f32,
) -> Result<OfflineTts> {
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
        // 用户可调（设置页「引导强度」），见 ZIPVOICE_GUIDANCE_SCALE。
        guidance_scale,
    };

    build_engine(
        OfflineTtsModelConfig {
            zipvoice: zipvoice_config,
            ..base_model_config(provider)
        },
        model_dir,
        "ZipVoice",
    )
}

/// Kitten：`model.onnx` + `voices.bin`。
fn create_kitten_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
    let model = find_file(model_dir, &["model.onnx", "model.int8.onnx"])?;
    let voices = find_file(model_dir, &["voices.bin"])?;

    let kitten_config = OfflineTtsKittenModelConfig {
        model: Some(model.to_string_lossy().to_string()),
        voices: Some(voices.to_string_lossy().to_string()),
        tokens: find_file_optional(model_dir, &["tokens.txt"])
            .map(|p| p.to_string_lossy().to_string()),
        data_dir: espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
        ..Default::default()
    };

    build_engine(
        OfflineTtsModelConfig {
            kitten: kitten_config,
            ..base_model_config(provider)
        },
        model_dir,
        "Kitten",
    )
}

/// Pocket：一组 onnx + `vocab.json` / `token_scores.json`。
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

    build_engine(
        OfflineTtsModelConfig {
            pocket: pocket_config,
            ..base_model_config(provider)
        },
        model_dir,
        "Pocket TTS",
    )
}

/// Supertonic：duration_predictor / text_encoder / vector_estimator / vocoder。
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

    build_engine(
        OfflineTtsModelConfig {
            supertonic: supertonic_config,
            ..base_model_config(provider)
        },
        model_dir,
        "Supertonic",
    )
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

/// 能拿来读 metadata 的主模型文件，按优先级排列。
///
/// zipvoice / pocket 这类「多文件模型」没有单一主模型（text_encoder + fm_decoder …），
/// 查询音色时会走 `find_model_file` 的报错分支，由调用方回退到只有语义档。
const MODEL_FILE_CANDIDATES: &[&str] = &[
    "model.onnx",
    "model.int8.onnx",
    "tts-model.onnx",
    "sherpa-onnx-tts.onnx",
    "model-steps-3.onnx",
    "model-steps-6.onnx",
];

/// 找到模型目录里的主模型文件（用于读 `n_speakers` / `speaker_names`）。
pub(super) fn find_model_file(dir: &Path) -> Result<PathBuf> {
    find_file(dir, MODEL_FILE_CANDIDATES)
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
pub(super) fn needs_digit_normalization(model_dir: &Path) -> bool {
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
pub(super) fn model_reads_latin(model_dir: &Path) -> bool {
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

/// Git LFS 指针文件的头一行。文件走 LFS 之后，没 `git lfs pull` 的仓库里就是这份
/// 130 字节的文本——直接当音素表读，每个字都会变成 OOV，报错却只说「不匹配」。
const LFS_POINTER_HEADER: &str = "version https://git-lfs.github.com/spec/v1";

/// 读取随包的中英音素表，行尾统一成 LF。
///
/// 行尾必须干净：`\r` 会被当成音素的一部分而全部查不到（`.gitattributes` 用 LFS 的
/// `-text` 保证检出时不做行尾转换，这里再兜一层——手工拷贝过的副本可能不是）。
fn bundled_zipvoice_lexicon(data_dir: &Path) -> Result<String> {
    let path = data_dir.join(ZIPVOICE_LEXICON_REL_PATH);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        anyhow!(
            "读取随包 ZipVoice 音素表失败（{}）: {e}——安装包不完整？",
            path.display()
        )
    })?;
    if text.starts_with(LFS_POINTER_HEADER) {
        return Err(anyhow!(
            "随包 ZipVoice 音素表还是 Git LFS 指针（{}），先执行 `git lfs pull`",
            path.display()
        ));
    }
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

/// kokoro 的说话人下标（`voices.bin` 里的顺序，含义随模型版本变化）。
///
/// 以前固定 0：v1.1-zh 的 0 号是英文女声 `af_maple`，中文台词也交给它念——音色本身
/// 就不是中文的，中文的连读和句读停顿也跟着不对（同一个逗号，实测英文音色停 0.68 秒、
/// 中文音色 0.35 秒；句内多出来的停顿另由 [`super::audio::cap_inner_silence`] 收口）。
/// 所以中文/日文台词按设置页的「音色」挑一个中文音色：
///
/// - v1.1-zh（103 音色）：0-2 英文、3-57 中文女声（`zf_001`…）、58-102 中文男声
///   （`zm_009`…）。
/// - v1.0（53 音色）：0-19 英文、20-27 英式、28-44 其它语言、45-48 中文女声、
///   49-52 中文男声。
///
/// `voice` 直接写成数字时原样采用（收口到有效范围），方便试不同音色。kokoro 没有
/// 童声/老年声，`child` / `elderly` 与空值一样按女声处理。
pub(super) fn kokoro_sid(language: &str, voice: &str, num_speakers: i32) -> i32 {
    let last = num_speakers.saturating_sub(1);
    if last < 0 {
        return 0;
    }

    let voice = voice.trim();
    if let Ok(sid) = voice.parse::<i32>() {
        return sid.clamp(0, last);
    }

    // 英文（及没法确定语言的）保持 0（`af_maple`）：v1.1-zh 里英文也只有女声。
    let chinese = is_language(language, "zh") || is_language(language, "ja");
    if !chinese {
        return 0;
    }

    let male = voice.eq_ignore_ascii_case("male");
    let sid = match num_speakers {
        103 => {
            if male {
                58
            } else {
                3
            }
        },
        53 => {
            if male {
                49
            } else {
                45
            }
        },
        // 不认识的 kokoro 版本：宁可保持原来的 0，也不要瞎指一个音色。
        _ => 0,
    };
    sid.min(last)
}

/// 选传给模型的说话人下标。
///
/// kokoro 的 `voice` 是语义标签（female/male/…），交给 `kokoro_sid` 按语言挑；
/// 其它模型只认纯数字 `voice`（多说话人 VITS/Kitten 靠它选音色），标签一律 0。
/// 数字统一收口到 `0..num_speakers-1`，用户手填/旧配置越界都不会让 sherpa 报错。
pub(super) fn pick_sid(model_type: &str, language: &str, voice: &str, num_speakers: i32) -> i32 {
    if model_type == "kokoro" {
        return kokoro_sid(language, voice, num_speakers);
    }
    let last = num_speakers.saturating_sub(1).max(0);
    voice
        .trim()
        .parse::<i32>()
        .map(|sid| sid.clamp(0, last))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// 中文台词用中文音色，英文台词保持 0；数字写法可直接指定音色。
    #[test]
    fn picks_kokoro_voice_by_language() {
        // v1.1-zh：中文女声 3（zf_001）、中文男声 58（zm_009）。
        assert_eq!(kokoro_sid("zh", "", 103), 3);
        assert_eq!(kokoro_sid("zh", "female", 103), 3);
        assert_eq!(kokoro_sid("zh", "male", 103), 58);
        // 童声/老年声没有对应音色，按女声处理。
        assert_eq!(kokoro_sid("zh", "child", 103), 3);
        assert_eq!(kokoro_sid("zh", "elderly", 103), 3);
        // 日文在这个模型里也只有中文音色可用。
        assert_eq!(kokoro_sid("ja", "female", 103), 3);
        // 英文台词不动：仍是 0（af_maple），男声也没有别的英文音色可选。
        assert_eq!(kokoro_sid("en", "female", 103), 0);
        assert_eq!(kokoro_sid("en", "male", 103), 0);

        // v1.0：中文女声 45、男声 49。
        assert_eq!(kokoro_sid("zh", "female", 53), 45);
        assert_eq!(kokoro_sid("zh", "male", 53), 49);
        assert_eq!(kokoro_sid("en", "female", 53), 0);

        // 不认识的音色数量 / 没有音色：保持 0。
        assert_eq!(kokoro_sid("zh", "female", 0), 0);
        assert_eq!(kokoro_sid("zh", "female", 7), 0);

        // 直接写数字：原样采用（越界收口），大小写不敏感。
        assert_eq!(kokoro_sid("zh", "45", 103), 45);
        assert_eq!(kokoro_sid("en", "45", 103), 45);
        assert_eq!(kokoro_sid("zh", "999", 103), 102);
        assert_eq!(kokoro_sid("zh", "-3", 103), 0);
        assert_eq!(kokoro_sid("zh", "MALE", 103), 58);
    }

    /// 非 kokoro 模型：数字 voice 直接选 sid（多说话人 VITS/Kitten），标签保持 0。
    #[test]
    fn picks_numeric_sid_for_other_models() {
        assert_eq!(pick_sid("vits", "zh", "2", 4), 2);
        assert_eq!(pick_sid("kitten", "zh", "1", 2), 1);
        // 越界收口，负数落到 0。
        assert_eq!(pick_sid("vits", "zh", "99", 4), 3);
        assert_eq!(pick_sid("vits", "zh", "-3", 4), 0);
        // 语义标签 / 空值：单说话人路径，保持 0。
        assert_eq!(pick_sid("vits", "zh", "male", 4), 0);
        assert_eq!(pick_sid("vits", "zh", "", 4), 0);
        // 单说话人模型，填任何数字都收口到 0。
        assert_eq!(pick_sid("matcha", "zh", "5", 1), 0);
        assert_eq!(pick_sid("vits", "zh", "5", 0), 0);
        // kokoro 仍走语义映射。
        assert_eq!(pick_sid("kokoro", "zh", "male", 103), 58);
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

    /// 仓库没跑 `git lfs pull` 时，随包音素表只是 LFS 指针：要报「先 git lfs pull」，
    /// 而不是一句「音素与 tokens.txt 不匹配」。
    #[test]
    fn zipvoice_lfs_pointer_is_reported_as_such() {
        let dir = std::env::temp_dir().join(format!("lingchat-lfs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let lex_dir = dir.join("third_party/zipvoice");
        std::fs::create_dir_all(&lex_dir).expect("建临时 data 目录失败");
        std::fs::write(
            lex_dir.join("lexicon-zh-en.txt"),
            "version https://git-lfs.github.com/spec/v1\n\
             oid sha256:c94b049d970e971acdc2a7e5e7b23ca594222a9d640e417c711f2ad1e7bb7d46\n\
             size 1727147\n",
        )
        .expect("写 LFS 指针失败");

        let err = bundled_zipvoice_lexicon(&dir).expect_err("指针文件应报错");
        assert!(
            err.to_string().contains("git lfs pull"),
            "错误信息应提示 git lfs pull: {err}"
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
}
