//! 端侧音色微调：数据集导入、列表与删除。
//!
//! 只负责数据侧，训练与导出在后续接入。数据集存放在
//! `data/sherpa_onnx_finetune/<dataset_id>/`，与训练模块共用同一目录约定。

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use candle_core::{DType, Device};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::ai_service::tts::finetune::{
    BaseDecoder, BaseDecoderConfig, BaseEncoderConfig, BaseTrainingConfig, BaseTrainingOutcome,
    CorpusConfig, DEFAULT_SAMPLE_RATE, DatasetManifest, DatasetStats, ImportReport, MelConfig,
    TextFrontend, TrainingProgress, base_training, build_corpus, export_tuned_model, finetune_root,
    import_zip, list_datasets, load_manifest, remove_dataset, train_base,
};
use candle_nn::{VarBuilder, VarMap};

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
) -> Result<FinetuneTrainOutcome, String> {
    let root = finetune_root().join(&dataset_id);
    if !root.is_dir() {
        return Err(format!("数据集不存在: {dataset_id}"));
    }

    let base = PathBuf::from(&base_model_dir);
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
    let encoder_config = BaseEncoderConfig {
        n_vocab: frontend.tokens().len(),
        n_mels: mel.n_mels,
        ..Default::default()
    };
    let decoder_config = BaseDecoderConfig {
        n_mels: mel.n_mels,
        ..Default::default()
    };
    let config = BaseTrainingConfig {
        epochs: epochs.unwrap_or(20).max(1),
        learning_rate: learning_rate.unwrap_or(1e-3),
        ..Default::default()
    };

    let checkpoint = root.join("checkpoint.safetensors");
    let checkpoint_display = checkpoint.display().to_string();

    let token = CancellationToken::new();
    *active_train()
        .lock()
        .map_err(|_| "训练状态锁失效".to_string())? = Some(token.clone());

    let result =
        tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<BaseTrainingOutcome> {
            let device = Device::Cpu;
            train_base(
                &corpus,
                &encoder_config,
                &decoder_config,
                &config,
                &onnx_path,
                Some(&checkpoint),
                Some(&token),
                &device,
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
    let base = PathBuf::from(&base_model_dir);
    if !base.is_dir() {
        return Err(format!("基座目录不存在: {}", base.display()));
    }

    let checkpoint = root.join("checkpoint.safetensors");
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

    let device = Device::Cpu;
    // 组装解码器 VarMap：结构与基座一致，再从检查点装载微调参数。
    let mut var_map = VarMap::new();
    let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
    BaseDecoder::new(&BaseDecoderConfig::default(), vb)
        .map_err(|error| format!("构造解码器失败: {error}"))?;
    base_training::load_checkpoint(&mut var_map, &checkpoint)
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
            export_tuned_model(&var_map, &onnx_path, &weight_map, &tmp)
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
