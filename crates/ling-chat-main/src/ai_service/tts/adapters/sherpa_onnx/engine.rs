//! 引擎构造：按模型类型定位文件并填 sherpa 配置。

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

/// 按模型类型创建引擎。
pub(super) fn create_engine(
    model_type: &str,
    model_dir: &Path,
    provider: Option<&str>,
    tuning: SherpaTuning,
) -> Result<OfflineTts> {
    match model_type {
        "vits" | "fastspeech2" => create_vits_tts(model_dir, provider),
        "matcha" => create_matcha_tts(model_dir, provider),
        "kokoro" => create_kokoro_tts(model_dir, provider),
        "kitten" => create_kitten_tts(model_dir, provider),
        "zipvoice" => create_zipvoice_tts(model_dir, provider, tuning.guidance_scale),
        "pocket" => create_pocket_tts(model_dir, provider),
        "supertonic" => create_supertonic_tts(model_dir, provider),
        other => Err(anyhow!("Sherpa-ONNX 不支持的模型类型: {other}")),
    }
}

/// TTS 推理线程数上限。
///
/// 短句合成里固定调度开销占比更高，线程开到 4 个以上收益不再增加，反而可能因
/// 过度订阅变慢；同时留出核心给音频播放、UI 和其它推理任务，避免移动端发热和卡顿。
const MAX_TTS_THREADS: usize = 4;

/// 本次进程可用的 TTS 推理线程数。
///
/// sherpa 的 Rust 绑定 `Default` 会把 `num_threads` 留成 0，C API 再回退成 1；
/// 这里显式填成按 CPU 核数限制后的值，让 ORT 的 intra/inter-op 线程池真正并行。
fn tts_num_threads() -> i32 {
    // 调试用覆盖，方便对比 1/2/4/8 线程的耗时；正式配置仍走自动值。
    if let Ok(raw) = std::env::var("LINGCHAT_TTS_THREADS") {
        if let Ok(n) = raw.trim().parse::<usize>() {
            let n = n.clamp(1, 32) as i32;
            tracing::debug!("Sherpa-ONNX TTS 线程数由 LINGCHAT_TTS_THREADS 覆盖: {n}");
            return n;
        }
    }

    let n = std::thread::available_parallelism()
        .map(|n| n.get().min(MAX_TTS_THREADS))
        .unwrap_or(1) as i32;
    tracing::debug!("Sherpa-ONNX TTS 推理线程数: {n}（CPU 核数上限 {MAX_TTS_THREADS}）");
    n
}

/// 各模型共用的执行提供方与线程设置（CPU 时 provider 为空）。
fn base_model_config(provider: Option<&str>) -> OfflineTtsModelConfig {
    OfflineTtsModelConfig {
        provider: provider.map(str::to_string),
        num_threads: tts_num_threads(),
        ..Default::default()
    }
}

/// 各模型共用的引擎配置（切句、停顿缩放、规则 FST）。
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
        model: Some(path_string(model_file)),
        lexicon: optional_file_string(model_dir, &["lexicon.txt"]),
        tokens: Some(path_string(tokens_file)),
        // sherpa-onnx 2.0 起忽略 data_dir（espeak-ng 已移除）。
        data_dir: None,
        noise_scale: 0.667,
        noise_scale_w: 0.8,
        length_scale: 1.0,
        dict_dir: optional_dir_string(model_dir, &["dict-dir", "dict"]),
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
        acoustic_model: Some(path_string(acoustic_model)),
        vocoder: Some(path_string(vocoder)),
        lexicon: optional_file_string(model_dir, &["lexicon.txt"]),
        tokens: Some(path_string(tokens_file)),
        // sherpa-onnx 2.0 起忽略 data_dir（espeak-ng 已移除）。
        data_dir: None,
        noise_scale: 0.667,
        length_scale: 1.0,
        dict_dir: optional_dir_string(model_dir, &["dict"]),
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

/// Kokoro 多语言：`voices.bin` 选音色，`lexicon*.txt` 决定可读文字。
/// sherpa-onnx 2.0 起 `data_dir` / `lang` 被忽略，这里传入所有能找到的词表。
fn create_kokoro_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
    let model = find_file(model_dir, &["model.onnx", "model.int8.onnx"])?;
    let voices = find_file(model_dir, &["voices.bin"])?;
    // Android/arm64 上 kokoro 的 int8 量化会输出静音/垃圾样本（上游 sherpa-onnx#3754，
    // 实机验证换 fp32 即恢复），这里直接报错，避免用户只听到静音。
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
    let kokoro_config = OfflineTtsKokoroModelConfig {
        model: Some(path_string(model)),
        voices: Some(path_string(voices)),
        tokens: optional_file_string(model_dir, &["tokens.txt"]),
        length_scale: 1.0,
        dict_dir: optional_dir_string(model_dir, &["dict"]),
        lexicon: kokoro_lexicons(model_dir),
        ..Default::default()
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

/// ZipVoice 零样本克隆：encoder/decoder/vocoder + 音素表。
fn create_zipvoice_tts(
    model_dir: &Path,
    provider: Option<&str>,
    guidance_scale: f32,
) -> Result<OfflineTts> {
    let encoder = find_file(model_dir, &["text_encoder.onnx"])?;
    let decoder = find_file(model_dir, &["fm_decoder.onnx"])?;
    let vocoder = find_file(model_dir, &["vocos_24khz.onnx", "vocoder.onnx"])?;

    // ZipVoice 只需要 tokens + lexicon；lexicon 为空时原生侧会 `_Exit(-1)`，
    // 这里先拦成普通错误。
    let lexicon = zipvoice_lexicon(model_dir)?;

    let zipvoice_config = OfflineTtsZipvoiceModelConfig {
        tokens: optional_file_string(model_dir, &["tokens.txt"]),
        encoder: Some(path_string(encoder)),
        decoder: Some(path_string(decoder)),
        vocoder: Some(path_string(vocoder)),
        // sherpa-onnx 2.0 起忽略 data_dir（espeak-ng 已移除）。
        data_dir: None,
        lexicon: Some(path_string(lexicon)),
        // 必须显式写：Rust 绑定默认全是 0.0，而原生侧要求这些值 > 0（0 会被判为非法参数）。
        // 数值取上游默认：feat_scale=0.1、t_shift=0.5、target_rms=0.1、guidance_scale=1.0。
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
        model: Some(path_string(model)),
        voices: Some(path_string(voices)),
        tokens: optional_file_string(model_dir, &["tokens.txt"]),
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
        lm_flow: Some(file_string(
            model_dir,
            &["lm_flow.int8.onnx", "lm_flow.onnx"],
        )?),
        lm_main: Some(file_string(
            model_dir,
            &["lm_main.int8.onnx", "lm_main.onnx"],
        )?),
        encoder: Some(file_string(model_dir, &["encoder.onnx"])?),
        decoder: Some(file_string(
            model_dir,
            &["decoder.int8.onnx", "decoder.onnx"],
        )?),
        text_conditioner: Some(file_string(model_dir, &["text_conditioner.onnx"])?),
        vocab_json: Some(file_string(model_dir, &["vocab.json"])?),
        token_scores_json: Some(file_string(model_dir, &["token_scores.json"])?),
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
        duration_predictor: Some(file_string(model_dir, &["duration_predictor.onnx"])?),
        text_encoder: Some(file_string(model_dir, &["text_encoder.onnx"])?),
        vector_estimator: Some(file_string(model_dir, &["vector_estimator.onnx"])?),
        vocoder: Some(file_string(
            model_dir,
            &["vocoder.onnx", "vocoder_f16.onnx"],
        )?),
        tts_json: Some(file_string(model_dir, &["tts.json"])?),
        unicode_indexer: optional_file_string(model_dir, &["unicode_indexer.json"]),
        voice_style: optional_file_string(model_dir, &["voice_style.onnx"]),
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
    find_file_optional(dir, candidates).ok_or_else(|| {
        anyhow!(
            "在 {} 中未找到模型文件，候选: {:?}",
            dir.display(),
            candidates
        )
    })
}

/// 能拿来读 metadata 的主模型文件，按优先级排列。
/// 多文件模型没有单一主模型，会走报错分支由调用方回退。
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
/// 中文模型靠它在分词前改写数字/日期；不挂载时 CharacterLexicon 会丢词。
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

/// 若模型目录内附带文本规则 FST，挂载到 `OfflineTtsConfig`。
fn with_rule_files(mut config: OfflineTtsConfig, model_dir: &Path) -> OfflineTtsConfig {
    let fsts: Vec<String> = rule_fst_paths(model_dir)
        .into_iter()
        .map(path_string)
        .collect();
    if !fsts.is_empty() {
        // sherpa 侧按逗号切分，依次 Normalize。
        config.rule_fsts = Some(fsts.join(","));
    }
    if let Some(far) = find_file_optional(model_dir, &["rulefst.far", "rule.far"]) {
        config.rule_fars = Some(path_string(far));
    }
    config
}

/// 模型目录里没有规则 FST 时需在 Rust 侧改写数字。
/// 是否动手还看当次文本（纯英文台词不改）。
pub(super) fn needs_digit_normalization(model_dir: &Path) -> bool {
    rule_fst_paths(model_dir).is_empty()
}

/// 模型词表是否认拉丁字母（决定英文/罗马音是否要剔除）。
/// 先看文件名里的独立 `en` 段，再看词表内容里是否有拉丁字母键。
pub(super) fn model_reads_latin(model_dir: &Path) -> bool {
    let paths = lexicon_files(model_dir);
    if paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.split(['-', '_', '.']).any(|part| part == "en"))
    }) {
        return true;
    }
    // 词表内容里出现拉丁字母的键，同样说明模型自己认得（查到一个就收工）。
    paths.iter().any(|path| {
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

fn path_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn file_string(dir: &Path, candidates: &[&str]) -> Result<String> {
    Ok(path_string(find_file(dir, candidates)?))
}

fn optional_file_string(dir: &Path, candidates: &[&str]) -> Option<String> {
    find_file_optional(dir, candidates).map(path_string)
}

fn optional_dir_string(dir: &Path, candidates: &[&str]) -> Option<String> {
    find_dir_optional(dir, candidates).map(path_string)
}

fn lexicon_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("lexicon") && name.ends_with(".txt"))
        })
        .collect();
    files.sort();
    files
}

/// 收集 Kokoro 需要的 lexicon（逗号分隔）。
/// 英文只挂一份，避免大量重复词告警；2.0 起 lexicon 是唯一的中日文音素化入口。
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
    for path in lexicon_files(model_dir) {
        if !files.contains(&path) && !ENGLISH.iter().any(|name| path.ends_with(name)) {
            files.push(path);
        }
    }

    (!files.is_empty()).then(|| {
        files
            .into_iter()
            .map(path_string)
            .collect::<Vec<_>>()
            .join(",")
    })
}

/// 随包 ZipVoice 中英音素表相对 data 目录的路径。
/// sherpa 在 lexicon 为空时 `_Exit(-1)`，而镜像包不带该文件，必须随包提供。
const ZIPVOICE_LEXICON_REL_PATH: &str = "third_party/zipvoice/lexicon-zh-en.txt";

/// Git LFS 指针文件首行；未 `git lfs pull` 时会被误当音素表。
const LFS_POINTER_HEADER: &str = "version https://git-lfs.github.com/spec/v1";

/// 读取随包中英音素表，行尾统一成 LF（`\r` 会被当成音素的一部分）。
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

/// 随包资源所在 data 目录；单测未初始化时回退到仓库 `data/`。
fn bundled_assets_data_dir() -> PathBuf {
    crate::data_dir::try_get_data_dir()
        .cloned()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
}

/// 定位 ZipVoice lexicon：优先模型自带，缺失时写入随包音素表。
fn zipvoice_lexicon(model_dir: &Path) -> Result<PathBuf> {
    if let Some(path) = find_file_optional(model_dir, &["lexicon.txt"]) {
        // 0 字节 lexicon 等于没有：空文件会让每个字变 OOV，按缺失处理。
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

/// 校验随包音素表的音素都被模型 `tokens.txt` 收录，避免误配。
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

/// kokoro 的说话人下标（顺序随模型版本变化）。
/// 103 音色：中文女声从 3 起、男声从 58 起；53 音色：45/49。数字 voice 原样采用，标签按语言映射。
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

/// 选说话人下标：kokoro 按语言/标签映射，其它模型只认数字 voice。
/// 数字统一收口到有效范围，越界不会让 sherpa 报错。
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

    /// 规则 FST 的收集顺序与有无判断。
    #[test]
    fn collects_rule_fsts_in_order() {
        let dir = std::env::temp_dir().join(format!("lingchat-digit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录失败");

        // 没带 FST 时需在 Rust 侧改写数字。
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

        // 直接写数字：原样采用（越界收口）。
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

    /// 造一个假 ZipVoice 模型目录（只写 tokens.txt）。
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

    /// 仓库没跑 `git lfs pull` 时，报错要提示先 pull。
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
