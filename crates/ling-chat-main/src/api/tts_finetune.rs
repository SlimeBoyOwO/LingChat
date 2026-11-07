//! 端侧音色微调：数据集导入、列表与删除。
//!
//! 只负责数据侧，训练与导出在后续接入。数据集存放在
//! `data/sherpa_onnx_finetune/<dataset_id>/`，与训练模块共用同一目录约定。

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::ai_service::tts::finetune::burn_decoder::BurnBaseDecoder;
use crate::ai_service::tts::finetune::burn_training::{
    BaseTrainingConfig as BurnBaseTrainingConfig, BaseTrainingOutcome, load_checkpoint,
    train_base as burn_train_base,
};
use crate::ai_service::tts::finetune::{
    CorpusConfig, DEFAULT_SAMPLE_RATE, DatasetItem, DatasetManifest, DatasetStats, ImportReport,
    MelConfig, TextFrontend, TrainingProgress, build_corpus, export_tuned_model_burn, finetune_root,
    import_zip, list_datasets, load_manifest, persist_text_updates, remove_dataset,
};

/// 数据集摘要，供前端列表展示。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FinetuneDatasetSummary {
    pub dataset_id: String,
    pub items: usize,
    pub items_with_text: usize,
    pub total_secs: f64,
    pub meets_minimum: bool,
}

fn summary_from(manifest: &DatasetManifest) -> FinetuneDatasetSummary {
    FinetuneDatasetSummary {
        dataset_id: manifest.dataset_id.clone(),
        items: manifest.stats.total_items,
        items_with_text: manifest.stats.items_with_text,
        total_secs: manifest.stats.total_secs,
        meets_minimum: manifest.stats.meets_minimum,
    }
}

/// 导入压缩包。
#[tauri::command]
pub async fn tts_finetune_import(
    app: AppHandle,
    zip_path: String,
    dataset_id: String,
    overwrite: Option<bool>,
) -> Result<ImportReport, String> {
    // Android 的 content:// 先落到 cache 成真实路径；桌面端原路径直通。
    let src =
        crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &zip_path)
            .await?;
    if !src.path.is_file() {
        return Err(format!("压缩包不存在: {zip_path}"));
    }

    let import_path = src.path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        import_zip(
            &import_path,
            &dataset_id,
            DEFAULT_SAMPLE_RATE,
            overwrite.unwrap_or(false),
        )
    })
    .await
    .map_err(|error| format!("导入任务失败: {error}"))?
    .map_err(|error| format!("导入失败: {error:#}"));

    // Android 的 content:// 暂存文件用完即删，避免遗留到 cache。
    if src.cleanup_after_import {
        let _ = std::fs::remove_file(&src.path);
    }
    result
}

/// 列出已导入的数据集。
#[tauri::command]
pub fn tts_finetune_list() -> Result<Vec<FinetuneDatasetSummary>, String> {
    let ids = list_datasets().map_err(|error| format!("列出数据集失败: {error:#}"))?;
    Ok(ids
        .iter()
        .filter_map(|id| load_manifest(id).ok())
        .map(|manifest| summary_from(&manifest))
        .collect())
}

/// 读取数据集统计。
#[tauri::command]
pub fn tts_finetune_stats(dataset_id: String) -> Result<DatasetStats, String> {
    let manifest =
        load_manifest(&dataset_id).map_err(|error| format!("读取数据集失败: {error:#}"))?;
    Ok(manifest.stats)
}

/// 删除数据集。
#[tauri::command]
pub fn tts_finetune_remove(dataset_id: String) -> Result<(), String> {
    remove_dataset(&dataset_id).map_err(|error| format!("删除数据集失败: {error:#}"))
}

/// 训练结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FinetuneTrainOutcome {
    pub steps: usize,
    pub initial_loss: f32,
    pub final_loss: f32,
    pub cancelled: bool,
    pub checkpoint: String,
    /// 从第几轮开始续训；0 表示从头训练。
    pub resumed_from: usize,
}

fn active_train() -> &'static Mutex<Option<CancellationToken>> {
    static ACTIVE: OnceLock<Mutex<Option<CancellationToken>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(None))
}

/// 开始训练。
///
/// 基座目录需要提供 tokens.txt 与 lexicon.txt，用于把文本转成和基座一致的符号 id。
#[tauri::command]
pub async fn tts_finetune_train(
    app: AppHandle,
    dataset_id: String,
    base_model_dir: String,
    epochs: Option<usize>,
    learning_rate: Option<f64>,
    cleanup_every: Option<usize>,
    max_frames_per_step: Option<usize>,
) -> Result<FinetuneTrainOutcome, String> {
    let root = finetune_root().join(&dataset_id);
    if !root.is_dir() {
        return Err(format!("数据集不存在: {dataset_id}"));
    }

    let base = resolve_base_dir(&base_model_dir);
    let tokens_path = base.join("tokens.txt");
    if !tokens_path.is_file() {
        return Err(format!("基座目录缺少 tokens.txt: {}", base.display()));
    }
    let lexicon_path = base.join("lexicon.txt");
    let lexicon = lexicon_path.is_file().then_some(lexicon_path);

    // 基座 ONNX：目录下形如 model-steps-3.onnx 的声学模型。
    let onnx_path = find_base_onnx(&base)?;

    let frontend = TextFrontend::load(&tokens_path, lexicon.as_deref())
        .map_err(|error| format!("加载基座词表失败: {error:#}"))?;
    let corpus = build_corpus(&root, &frontend, &CorpusConfig::default())
        .map_err(|error| format!("组装语料失败: {error:#}"))?;

    let mel = MelConfig::default();
    let encoder_config = crate::ai_service::tts::finetune::burn_encoder::BurnBaseEncoderConfig {
        n_vocab: frontend.tokens().len(),
        n_mels: mel.n_mels,
        ..Default::default()
    };
    let decoder_config = crate::ai_service::tts::finetune::burn_decoder::BurnBaseDecoderConfig {
        n_mels: mel.n_mels,
        ..Default::default()
    };
    let config = BurnBaseTrainingConfig {
        epochs: epochs.unwrap_or(20).max(1),
        learning_rate: learning_rate.unwrap_or(1e-3),
        cleanup_every: cleanup_every.unwrap_or(50),
        max_frames_per_step: max_frames_per_step.unwrap_or(512),
        ..Default::default()
    };

    let checkpoint = root.join("checkpoint.bpk");
    let checkpoint_display = checkpoint.display().to_string();
    // 匿名投影权重映射表放在运行时数据目录（随程序数据分发），不要用编译期路径。
    let map_path = crate::data_dir::get_data_dir().join("tts-training/decoder_weight_map.json");

    let token = CancellationToken::new();
    *active_train()
        .lock()
        .map_err(|_| "训练状态锁失效".to_string())? = Some(token.clone());

    let result =
        tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<BaseTrainingOutcome> {
            burn_train_base(
                &corpus,
                &encoder_config,
                &decoder_config,
                &config,
                &onnx_path,
                &map_path,
                Some(&checkpoint),
                Some(&token),
                |progress| {
                    // 每步都发事件会淹没前端，按步数节流。
                    if progress.step % 5 != 0 && progress.step != progress.total_steps {
                        return;
                    }
                    // 借用 training 的进度类型，把基座训练进度转成它再发射。
                    let legacy = TrainingProgress {
                        epoch: progress.epoch,
                        total_epochs: progress.total_epochs,
                        step: progress.step,
                        total_steps: progress.total_steps,
                        loss: progress.loss,
                    };
                    let _ = app.emit("tts:finetune-progress", legacy);
                },
            )
        })
        .await;

    if let Ok(mut guard) = active_train().lock() {
        *guard = None;
    }

    let outcome = result
        .map_err(|error| format!("训练任务失败: {error}"))?
        .map_err(|error| format!("训练失败: {error:#}"))?;

    Ok(FinetuneTrainOutcome {
        steps: outcome.steps,
        initial_loss: outcome.initial_loss,
        final_loss: outcome.final_loss,
        cancelled: outcome.cancelled,
        checkpoint: checkpoint_display,
        resumed_from: outcome.resumed_from,
    })
}

/// 在基座目录里查找声学 ONNX（形如 model-*.onnx，排除非模型文件）。
fn find_base_onnx(base: &PathBuf) -> Result<PathBuf, String> {
    let mut found: Vec<_> = std::fs::read_dir(base)
        .map_err(|error| format!("读取基座目录失败: {error}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            entry.path().is_file() && name.starts_with("model-") && name.ends_with(".onnx")
        })
        .collect();
    found.sort_by_key(|entry| entry.file_name());
    found
        .first()
        .map(|entry| entry.path())
        .ok_or_else(|| format!("基座目录缺少声学 ONNX: {}", base.display()))
}

/// 把前端传入的「基座模型目录」参数归一化为目录路径。
///
/// 前端给的 `model.path` 是模型目录里的声学 ONNX 文件（如
/// `…/matcha-icefall-zh-baker/model-steps-3.onnx`），而训练/导出需要的是**整个
/// 模型目录**（含 tokens.txt / lexicon.txt / voCos）。若传入的是文件则取父目录，
/// 兼容两种传法。
fn resolve_base_dir(base_model_dir: &str) -> PathBuf {
    let p = PathBuf::from(base_model_dir);
    if p.is_file() {
        p.parent().map(PathBuf::from).unwrap_or(p)
    } else {
        p
    }
}

/// 导出结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FinetuneExportOutcome {
    /// 生成的可选音色模型目录（相对 sherpa_onnx_models 的名字）。
    pub model_name: String,
    /// 模型目录的绝对路径。
    pub model_path: String,
}

/// 把训练好的解码器导出为独立的 Sherpa 音色模型目录。
///
/// 读取数据集检查点，把微调后的解码器权重写回基座声学 ONNX，再把导出的声学
/// 模型与基座配套文件（tokens/lexicon/dict/规则 FST/声码器）组装成一个自洽的
/// `sherpa_onnx_models/<model_name>/` 目录。目录一落地即被模型管理器自动识别为
/// 一个可选 matcha 音色，设置页可直接选中。
#[tauri::command]
pub async fn tts_finetune_export(
    dataset_id: String,
    base_model_dir: String,
    model_name: String,
) -> Result<FinetuneExportOutcome, String> {
    let model_name = model_name.trim().to_string();
    // 复用导入暂存文件的命名净化规则，保证目录名安全且可用于文件系统。
    let sanitized =
        crate::ai_service::tts::local::saf_bridge::sanitize_staged_filename(&model_name)
            .ok_or_else(|| "请给音色起个名字".to_string())?;

    let root = finetune_root().join(&dataset_id);
    if !root.is_dir() {
        return Err(format!("数据集不存在: {dataset_id}"));
    }
    let base = resolve_base_dir(&base_model_dir);
    if !base.is_dir() {
        return Err(format!("基座目录不存在: {}", base.display()));
    }

    let checkpoint = root.join("checkpoint.bpk");
    if !checkpoint.is_file() {
        return Err(format!(
            "数据集尚未训练，缺少检查点: {}",
            checkpoint.display()
        ));
    }
    let weight_map = crate::data_dir::get_data_dir().join("tts-training/decoder_weight_map.json");
    if !weight_map.is_file() {
        return Err(format!("缺少权重映射表: {}", weight_map.display()));
    }
    let onnx_path = find_base_onnx(&base)?;

    let device = crate::ai_service::tts::finetune::burn_backend::train_device();
    // 组装解码器骨架并从检查点装载微调参数。
    let config = crate::ai_service::tts::finetune::burn_decoder::BurnBaseDecoderConfig::default();
    let decoder = BurnBaseDecoder::init(&config, &device);
    let decoder = load_checkpoint(&decoder, &checkpoint)
        .map_err(|error| format!("读取检查点失败: {error:#}"))?;

    let dest_dir = crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root()
        .join(&sanitized);
    tokio::fs::create_dir_all(&dest_dir)
        .await
        .map_err(|error| format!("创建模型目录失败: {error}"))?;

    // 1) 导出写进临时文件，再原子搬到模型目录的 model-steps-*.onnx。
    let tmp = root.join(".export-tmp.onnx");
    tokio::task::spawn_blocking({
        let weight_map = weight_map.clone();
        let onnx_path = onnx_path.clone();
        let tmp = tmp.clone();
        move || {
            export_tuned_model_burn(&decoder, &onnx_path, &weight_map, &tmp)
                .map_err(|error| error.to_string())
        }
    })
    .await
    .map_err(|error| format!("导出任务失败: {error}"))??;
    let acoustic_name = "model-steps-3.onnx";
    tokio::fs::rename(&tmp, dest_dir.join(acoustic_name))
        .await
        .map_err(|error| format!("移动导出声学模型失败: {error}"))?;

    // 2) 复用基座配套文件（词表、音素表、dict、规则 FST、声码器）。
    for name in [
        "tokens.txt",
        "lexicon.txt",
        "vocos-22khz-univ.onnx",
        "phone.fst",
        "date.fst",
        "number.fst",
    ] {
        let src = base.join(name);
        if !src.is_file() {
            continue;
        }
        tokio::fs::copy(&src, dest_dir.join(name))
            .await
            .map_err(|error| format!("复制 {} 失败: {error}", src.display()))?;
    }
    if base.join("dict").is_dir() {
        let proto = copy_dir_tree;
        let src = base.join("dict");
        let dst = dest_dir.join("dict");
        tokio::task::spawn_blocking(move || proto(&src, &dst))
            .await
            .map_err(|error| format!("复制 dict 失败: {error}"))??;
    }

    // 3) 写 config.json：声明为 matcha，供模型管理器识别 type/语音。
    let config = serde_json::json!({
        "model_type": "matcha",
        "language": "zh",
        "voice": sanitized,
    });
    tokio::fs::write(
        dest_dir.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .await
    .map_err(|error| format!("写 config.json 失败: {error}"))?;

    Ok(FinetuneExportOutcome {
        model_name: sanitized.clone(),
        model_path: dest_dir.to_string_lossy().to_string(),
    })
}

/// 递归拷贝一个目录树。
fn copy_dir_tree(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|error| format!("创建目录 {} 失败: {error}", dst.display()))?;
    for entry in std::fs::read_dir(src)
        .map_err(|error| format!("读取目录 {} 失败: {error}", src.display()))?
    {
        let entry = entry.map_err(|error| format!("读取目录项失败: {error}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ty = entry
            .file_type()
            .map_err(|error| format!("读取文件类型失败: {error}"))?;
        if ty.is_dir() {
            copy_dir_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|error| format!("复制 {} 失败: {error}", from.display()))?;
        }
    }
    Ok(())
}

/// 取消正在进行的训练。
#[tauri::command]
pub fn tts_finetune_cancel() {
    if let Ok(guard) = active_train().lock() {
        if let Some(token) = guard.as_ref() {
            token.cancel();
        }
    }
}

/// 把 `%f32` 单声道采样序列化为 16-bit PCM 单声道 WAV 字节（供 ASR 识别用）。
///
/// ASR provider 统一吃 16kHz mono PCM16（见 `asr_recognize_wav`），而微调数据集
/// 的 WAV 是 22.05kHz，需先重采样再打包成字节。直接复用 hound 输出为内存缓冲，
/// 不落盘临时文件。
fn wav_bytes_from_mono(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut buf, spec)
        .map_err(|e| format!("创建 WAV 编码器失败: {e}"))?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer
            .write_sample(value)
            .map_err(|e| format!("写 WAV 样本失败: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("编码 WAV 失败: {e}"))?;
    Ok(buf.into_inner())
}

/// 自动标注结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FinetuneAnnotateReport {
    /// 本次识别成功并写入文本的样本数。
    pub annotated: usize,
    /// 识别失败（网络/API 报错）的样本数。
    pub failed: usize,
    /// 因读取/重采样问题被跳过的样本数。
    pub skipped: usize,
    /// 重算后的数据集统计。
    pub stats: DatasetStats,
}

/// 用云端 ASR 为缺失文本的音频自动标注。
///
/// 遍历数据集里 `text == None` 的样本，把（22.05kHz）WAV 重采样成 16kHz 后逐个
/// 调 provider 识别，识别成功的文本写回清单并重算统计。只补缺文本的条目，
/// 已有标注的一律不动。
///
/// `provider_id` 必须是已配置的 ASR provider（qwen-asr / llama-asr 等）；
/// `language_hint` 可选语言码（如 zh / en），透传给 provider。
#[tauri::command]
pub async fn tts_finetune_auto_annotate(
    app: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    dataset_id: String,
    provider_id: String,
    language_hint: Option<String>,
) -> Result<FinetuneAnnotateReport, String> {
    // 语言提示参数保留（前端仍传），但上游 ASR API 已移除语言字段，这里不再透传。
    let _ = language_hint;
    let root = finetune_root().join(&dataset_id);
    if !root.is_dir() {
        return Err(format!("数据集不存在: {dataset_id}"));
    }

    // 本地优先、云端回退：只要共享模型目录里已装 SenseVoice，就用本地 sherpa
    // （零配置、无凭据）；否则回退到前端传来的云端 provider。即使前端显式要
    // 云端，装了本地模型也优先走本地，未装时自然落回云端。
    let local_sherpa = provider_id
        == crate::ai_service::asr::providers::sherpa::SherpaAsrProvider::ID
        || crate::ai_service::asr::providers::sherpa::SherpaAsrProvider::model_installed();
    let session = if !local_sherpa {
        // 取 ASR 会话。未初始化（未启动/init 失败）时给明确提示。
        let guard = state.asr_state.session.lock().await;
        Some(guard.as_ref().cloned().ok_or_else(|| {
            "ASR 未初始化，请先配置并初始化语音识别（本地 sherpa 模型则不需要）".to_string()
        })?)
    } else {
        None
    };

    let mut manifest = load_manifest(&dataset_id).map_err(|e| format!("读取数据集失败: {e:#}"))?;
    // 收集缺文本的样本 id，逐个处理。
    let pending: Vec<DatasetItem> = manifest
        .items
        .iter()
        .filter(|item| item.text.is_none())
        .cloned()
        .collect();

    let opts = crate::ai_service::asr::provider::AsrOptions {
        hotwords: Vec::new(),
    };
    let mut annotated = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    // 单条过长音频容易拖垮识别请求，超过 32 秒直接跳过。
    const MAX_ASR_SECS: f64 = 32.0;

    for item in &pending {
        let wav_path = root.join(&item.wav);
        // 读取 + 重采样在阻塞线程做，避免拖住 async 运行时。
        let read = tauri::async_runtime::spawn_blocking({
            let wav_path = wav_path.clone();
            move || crate::ai_service::tts::finetune::normalize::read_wav_mono(&wav_path)
        })
        .await
        .map_err(|e| format!("读取音频任务失败: {e}"))?;
        let (samples, rate) = match read {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            },
        };
        if samples.len() as f64 / f64::from(rate) > MAX_ASR_SECS {
            skipped += 1;
            continue;
        }
        // 统一重采样成 16kHz（ASR 约定输入）。
        let samples = crate::ai_service::tts::finetune::resample::resample_mono(&samples, rate, 16_000);
        let wav_bytes = match wav_bytes_from_mono(&samples, 16_000) {
            Ok(b) => b,
            Err(_) => {
                skipped += 1;
                continue;
            },
        };

        let recognized = {
            if let Some(s) = &session {
                s.recognize_wav(provider_id.clone(), wav_bytes.clone(), &opts)
                    .await
            } else {
                let http = reqwest::Client::new();
                let cred: crate::ai_service::asr::provider_types::ProviderCredentials =
                    Default::default();
                let provider = crate::ai_service::asr::registry::get_provider(
                    crate::ai_service::asr::providers::sherpa::SherpaAsrProvider::ID,
                    &cred,
                    &http,
                )
                .await;
                match provider {
                    Ok(p) => p.recognize(wav_bytes.clone(), &opts).await,
                    Err(e) => Err(e),
                }
            }
        };
        match recognized {
            Ok(result) => {
                let text = result.text.trim().to_string();
                if !text.is_empty() {
                    if let Some(target) = manifest.items.iter_mut().find(|i| i.id == item.id) {
                        target.text = Some(text);
                    }
                    annotated += 1;
                } else {
                    failed += 1;
                }
            },
            Err(_) => failed += 1,
        }
        // 逐条推进度。
        let _ = app.emit(
            "tts:finetune-annotate-progress",
            serde_json::json!({
                "done": annotated + failed + skipped,
                "total": pending.len(),
                "id": item.id,
            }),
        );
    }

    // 至少成功一项才落盘；完全失败则不改动清单，避免把半成品写进去。
    let stats = if annotated > 0 {
        persist_text_updates(&root, &mut manifest).map_err(|e| format!("保存标注失败: {e}"))?
    } else {
        crate::ai_service::tts::finetune::import::summarize(&manifest.items)
    };

    Ok(FinetuneAnnotateReport {
        annotated,
        failed,
        skipped,
        stats,
    })
}
