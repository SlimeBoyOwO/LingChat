//! 端侧音色微调：数据集导入、列表与删除。
//!
//! 只负责数据侧，训练与导出在后续接入。数据集存放在
//! `data/sherpa_onnx_finetune/<dataset_id>/`，与训练模块共用同一目录约定。

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use candle_core::Device;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::ai_service::tts::finetune::{
    BaseDecoderConfig, BaseEncoderConfig, BaseTrainingConfig, BaseTrainingOutcome, CorpusConfig, DEFAULT_SAMPLE_RATE, DatasetManifest, DatasetStats,
    ImportReport, MelConfig, TextFrontend, TrainingProgress, build_corpus, finetune_root,
    import_zip, list_datasets, load_manifest, remove_dataset, train_base,
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
    let src = crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &zip_path)
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

    let result = tauri::async_runtime::spawn_blocking(
        move || -> anyhow::Result<BaseTrainingOutcome> {
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
        },
    )
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

/// 取消正在进行的训练。
#[tauri::command]
pub fn tts_finetune_cancel() {
    if let Ok(guard) = active_train().lock() {
        if let Some(token) = guard.as_ref() {
            token.cancel();
        }
    }
}
