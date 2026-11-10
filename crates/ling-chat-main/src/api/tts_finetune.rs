//! 端侧音色微调：数据集导入、列表与删除。
//!
//! 只负责数据侧，训练与导出在后续接入。数据集存放在
//! `data/sherpa_onnx_finetune/<dataset_id>/`，与训练模块共用同一目录约定。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio_util::sync::CancellationToken;

use crate::ai_service::tts::finetune::burn_decoder::BurnBaseDecoder;
use crate::ai_service::tts::finetune::burn_encoder::{
    BurnBaseEncoderConfig, from_onnx_tensors as load_encoder_from_onnx,
};
use crate::ai_service::tts::finetune::burn_training::{
    BaseTrainingConfig as BurnBaseTrainingConfig, BaseTrainingOutcome, apply_linear_lora,
    duration_adapter_path, inspect_checkpoint_adapters, inspect_duration_adapter_rank,
    load_checkpoint, load_duration_adapter, load_train_state, train_base as burn_train_base,
};
use crate::ai_service::tts::finetune::{
    CorpusConfig, DEFAULT_SAMPLE_RATE, DEFAULT_STYLE_CLUSTERS, DatasetItem, DatasetManifest,
    DatasetStats, ImportReport, MelConfig, StyleClusterReport, StyleClusterSummary, TextFrontend,
    TrainingProgress, build_corpus, cluster_dataset, export_tuned_model_burn_with_adapters,
    finetune_root, import_zip, list_datasets, load_manifest, persist_text_updates,
    read_initializers, remove_dataset, summarize_clusters,
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
    /// 已分配的风格簇；空表示尚未做无监督聚类。
    pub style_clusters: Vec<StyleClusterSummary>,
}

fn summary_from(manifest: &DatasetManifest) -> FinetuneDatasetSummary {
    FinetuneDatasetSummary {
        dataset_id: manifest.dataset_id.clone(),
        items: manifest.stats.total_items,
        items_with_text: manifest.stats.items_with_text,
        total_secs: manifest.stats.total_secs,
        meets_minimum: manifest.stats.meets_minimum,
        style_clusters: summarize_clusters(&manifest.items),
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

/// 对数据集做无监督风格聚类，把簇 id 写回清单。
///
/// `clusters` 是期望的簇数量，缺省 4，最终会自动收缩到样本数以内。
#[tauri::command]
pub async fn tts_finetune_cluster_styles(
    dataset_id: String,
    clusters: Option<usize>,
) -> Result<StyleClusterReport, String> {
    let root = finetune_root().join(&dataset_id);
    if !root.is_dir() {
        return Err(format!("数据集不存在: {dataset_id}"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        cluster_dataset(&root, clusters.unwrap_or(DEFAULT_STYLE_CLUSTERS))
    })
    .await
    .map_err(|error| format!("风格聚类任务失败: {error}"))?
    .map_err(|error| format!("风格聚类失败: {error:#}"))
}

/// 删除数据集。
#[tauri::command]
pub fn tts_finetune_remove(dataset_id: String) -> Result<(), String> {
    remove_dataset(&dataset_id).map_err(|error| format!("删除数据集失败: {error:#}"))
}

/// 数据集内的检查点路径。
///
/// 未选择风格簇时沿用 `checkpoint.bpk`；选择簇时每个簇一份独立检查点，避免
/// 后训的簇覆盖前一个簇。
fn dataset_checkpoint_path(root: &Path, style_cluster: Option<u32>) -> PathBuf {
    match style_cluster {
        Some(cluster) => root.join(format!("checkpoint-style-{cluster}.bpk")),
        None => root.join("checkpoint.bpk"),
    }
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
    /// 本次训练使用的风格簇；None 表示使用全部数据。
    pub style_cluster: Option<u32>,
}

/// 当前训练活动状态：进程内共享，供页面重建后恢复训练侧信息。
struct ActiveTrainState {
    /// 取消令牌；Some 表示正在训练。
    token: Option<CancellationToken>,
    /// 正在训练的数据集 id（无训练时为空）。
    dataset_id: Option<String>,
    /// 最近一次上报的进度；训练中逐步更新，结束后保留最后值。
    latest: Option<TrainingProgress>,
}

impl Default for ActiveTrainState {
    fn default() -> Self {
        Self {
            token: None,
            dataset_id: None,
            latest: None,
        }
    }
}

fn active_train() -> &'static Mutex<ActiveTrainState> {
    static ACTIVE: OnceLock<Mutex<ActiveTrainState>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(ActiveTrainState::default()))
}

/// 当前的训练活动状态查询结果。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct FinetuneActiveState {
    /// 是否有训练正在运行。
    pub training: bool,
    /// 正在训练的数据集 id（无训练时为空）。
    pub dataset_id: String,
    /// 最近一次上报的进度（无则为空）。
    pub latest: Option<TrainingProgress>,
}

/// 查询当前是否有训练在跑及其最新进度，供页面重建后恢复 UI 状态。
#[tauri::command]
pub fn tts_finetune_active() -> FinetuneActiveState {
    let guard = active_train().lock();
    let state = match guard {
        Ok(g) => g,
        Err(_) => return FinetuneActiveState::default(),
    };
    FinetuneActiveState {
        training: state.token.is_some(),
        dataset_id: state.dataset_id.clone().unwrap_or_default(),
        latest: state.latest.clone(),
    }
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
    lora_enabled: Option<bool>,
    lora_rank: Option<usize>,
    adapter_only: Option<bool>,
    duration_adapter: Option<bool>,
    style_cluster: Option<u32>,
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
    let corpus_config = CorpusConfig {
        style_cluster,
        ..Default::default()
    };
    let corpus = build_corpus(&root, &frontend, &corpus_config)
        .map_err(|error| format!("组装语料失败: {error:#}"))?;

    let mel = MelConfig::default();
    let lora_enabled = lora_enabled.unwrap_or(false);
    let lora_rank = lora_rank.unwrap_or(8).max(1);
    // 逐 token 时长适配默认开启：只在时长分支挂很小的旁路，导出时折叠，对
    // 低端设备也只是一次额外的轻量前向。秩取 LoRA 秩的一半并封顶 8。
    let duration_adapter = duration_adapter.unwrap_or(true);
    let duration_rank = (lora_rank / 2).clamp(1, 8);
    let encoder_config = crate::ai_service::tts::finetune::burn_encoder::BurnBaseEncoderConfig {
        n_vocab: frontend.tokens().len(),
        n_mels: mel.n_mels,
        duration_lora_rank: if duration_adapter { duration_rank } else { 0 },
        ..Default::default()
    };
    let decoder_config = crate::ai_service::tts::finetune::burn_decoder::BurnBaseDecoderConfig {
        n_mels: mel.n_mels,
        // 开 LoRA 时，ResBlock 与采样/收尾卷积也挂同秩旁路；导出时折叠进卷积权重。
        conv_lora_rank: if lora_enabled { lora_rank } else { 0 },
        ..Default::default()
    };
    let config = BurnBaseTrainingConfig {
        epochs: epochs.unwrap_or(20).max(1),
        learning_rate: learning_rate.unwrap_or(1e-3),
        cleanup_every: cleanup_every.unwrap_or(25),
        max_frames_per_step: max_frames_per_step.unwrap_or(192),
        lora_enabled,
        lora_rank,
        adapter_only: adapter_only.unwrap_or(false),
        duration_adapter,
        duration_adapter_rank: duration_rank,
        ..Default::default()
    };

    let checkpoint = dataset_checkpoint_path(&root, style_cluster);
    let checkpoint_display = checkpoint.display().to_string();
    // 匿名投影权重映射表放在运行时数据目录（随程序数据分发），不要用编译期路径。
    let map_path = crate::data_dir::get_data_dir().join("tts-training/decoder_weight_map.json");

    let token = CancellationToken::new();
    {
        let mut guard = active_train()
            .lock()
            .map_err(|_| "训练状态锁失效".to_string())?;
        *guard = ActiveTrainState {
            token: Some(token.clone()),
            dataset_id: Some(dataset_id.clone()),
            latest: None,
        };
    }

    // 开始训练时发一条通知，提示用户后台训练已启动（页面若已崩也能感知）。
    notify_train(&app, "start", &dataset_id, None);

    let ds_for_closure = dataset_id.clone();
    let app_for_closure = app.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<BaseTrainingOutcome> {
            let ds = ds_for_closure;
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
                        val_loss: progress.val_loss,
                    };
                    // 记录最近进度，供页面重建后恢复；每个 epoch 末（val_loss 上报）刷新一次通知。
                    if let Ok(mut guard) = active_train().lock() {
                        guard.latest = Some(legacy.clone());
                    }
                    if legacy.val_loss.is_some() {
                        notify_train(&app_for_closure, "progress", &ds, Some(&legacy));
                    }
                    let _ = app_for_closure.emit("tts:finetune-progress", legacy);
                },
            )
        })
        .await;

    if let Ok(mut guard) = active_train().lock() {
        // 结束保留最近进度供查询，只清空活动令牌与数据集 id。
        guard.token = None;
        guard.dataset_id = None;
    }

    let outcome = result
        .map_err(|error| format!("训练任务失败: {error}"))?
        .map_err(|error| format!("训练失败: {error:#}"))?;

    // 通知前端训练已结束（含被取消），前端据此清理训练中 UI。
    let finished = FinetuneTrainOutcome {
        steps: outcome.steps,
        initial_loss: outcome.initial_loss,
        final_loss: outcome.final_loss,
        cancelled: outcome.cancelled,
        checkpoint: checkpoint_display.clone(),
        resumed_from: outcome.resumed_from,
        style_cluster,
    };
    let _ = app.emit("tts:finetune-finished", &finished);
    let notify_kind = if outcome.cancelled {
        "cancelled"
    } else {
        "finished"
    };
    notify_train(&app, notify_kind, &dataset_id, None);

    Ok(finished)
}

/// 把训练阶段用系统通知呈现给用户。
///
/// 用固定 id 让进度阶段覆盖同一条通知，不堆积。kind 取值 start / progress /
/// cancelled / finished；progress 阶段用最近一次进度拼 body。
fn notify_train(
    app: &AppHandle,
    kind: &str,
    dataset_id: &str,
    progress: Option<&TrainingProgress>,
) {
    let title = match kind {
        "start" => "训练已开始".to_string(),
        "progress" => "训练进行中".to_string(),
        "cancelled" => "训练已取消".to_string(),
        _ => "训练已完成".to_string(),
    };
    let body = match kind {
        "progress" => match progress {
            Some(p) => format!(
                "{}：第 {}/{} 轮 · loss {:.4}",
                dataset_id, p.epoch, p.total_epochs, p.loss
            ),
            None => format!("{}：训练中", dataset_id),
        },
        _ => format!("{}", dataset_id),
    };
    // 通知失败（如 Android 未授权）不打断训练，仅忽略。
    let _ = app
        .notification()
        .builder()
        .id(3721)
        .title(title)
        .body(body)
        .show();
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

/// 估计所选风格簇相对基座时长预测的语速倍数。
///
/// 用数据集清单里的文本与真实音频时长，对基座编码器的时长分支做前向，
/// 取 `真实秒数 / 预测秒数` 的中位数。这样不需要额外标签，也不会被个别
/// 异常样本拉偏。结果限制在 0.7～1.4，避免数据噪声把语速改得离谱。
fn estimate_style_duration_scale(
    base: &Path,
    root: &Path,
    style_cluster: Option<u32>,
    encoder: &crate::ai_service::tts::finetune::burn_encoder::BurnBaseEncoder,
) -> Result<Option<f32>, String> {
    let manifest_path = root.join("dataset.json");
    let bytes =
        std::fs::read(&manifest_path).map_err(|error| format!("读取数据集清单失败: {error}"))?;
    let manifest: DatasetManifest =
        serde_json::from_slice(&bytes).map_err(|error| format!("解析数据集清单失败: {error}"))?;

    let tokens_path = base.join("tokens.txt");
    if !tokens_path.is_file() {
        return Ok(None);
    }
    let lexicon_path = base.join("lexicon.txt");
    let lexicon = lexicon_path.is_file().then_some(lexicon_path);
    let frontend = TextFrontend::load(&tokens_path, lexicon.as_deref())
        .map_err(|error| format!("加载基座词表失败: {error:#}"))?;

    let device = crate::ai_service::tts::finetune::burn_backend::train_device();

    let mut ratios: Vec<f32> = Vec::new();
    for item in &manifest.items {
        // 最多用 32 条样本估计语速，避免低性能设备导出前跑太久。
        if ratios.len() >= 32 {
            break;
        }
        if let Some(cluster) = style_cluster {
            if item.style_cluster != Some(cluster) {
                continue;
            }
        }
        let Some(text) = item.text.as_deref() else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        let token_ids = frontend.text_to_ids(text);
        if token_ids.is_empty() {
            continue;
        }
        let predicted = encoder.predict_duration_secs(&token_ids, &device)?;
        if predicted > 1e-3 && item.duration_secs > 0.0 {
            ratios.push((item.duration_secs as f32 / predicted).clamp(0.7, 1.4));
        }
    }
    if ratios.is_empty() {
        return Ok(None);
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let scale = ratios[ratios.len() / 2];
    tracing::info!(
        "[导出] 风格簇 {:?} 语速校准倍数 {scale:.3}（{} 条样本）",
        style_cluster,
        ratios.len()
    );
    Ok(Some(scale))
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
    style_cluster: Option<u32>,
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

    let checkpoint = dataset_checkpoint_path(&root, style_cluster);
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

    use burn::module::Module;

    let device = crate::ai_service::tts::finetune::burn_backend::train_device();
    // 组装解码器骨架并从检查点装载微调参数。LoRA 检查点里带旁路参数，
    // 骨架必须先按训练状态重建同样的旁路，否则 partial load 会把它们丢掉。
    let state = load_train_state(&checkpoint);
    // 旧检查点可能没有 train-state.json；从 burnpack 键名和形状补判旁路配置。
    let detected = if state.is_none() {
        inspect_checkpoint_adapters(&checkpoint)
    } else {
        None
    };
    let lora_enabled = state
        .as_ref()
        .map(|state| state.lora_enabled)
        .or_else(|| detected.map(|adapters| adapters.lora))
        .unwrap_or(false);
    let lora_rank = state
        .as_ref()
        .map(|state| state.lora_rank)
        .filter(|rank| *rank > 0)
        .or_else(|| detected.map(|adapters| adapters.lora_rank))
        .unwrap_or(8)
        .max(1);
    let conv_lora_rank = state
        .as_ref()
        .map(|state| state.conv_lora_rank)
        .filter(|rank| *rank > 0)
        .or_else(|| detected.map(|adapters| adapters.conv_lora_rank))
        .unwrap_or(0);
    // 时长旁路是独立文件；旧状态文件没有对应字段时按文件是否存在判断。
    let duration_enabled = state
        .as_ref()
        .map(|state| state.duration_adapter)
        .unwrap_or(false)
        || duration_adapter_path(&checkpoint).is_file();
    let duration_rank = state
        .as_ref()
        .map(|state| state.duration_adapter_rank)
        .filter(|rank| *rank > 0)
        .or_else(|| inspect_duration_adapter_rank(&duration_adapter_path(&checkpoint)))
        .unwrap_or(4);
    let config = crate::ai_service::tts::finetune::burn_decoder::BurnBaseDecoderConfig {
        conv_lora_rank,
        ..Default::default()
    };
    let mut decoder = BurnBaseDecoder::init(&config, &device);
    if lora_enabled {
        decoder = apply_linear_lora(decoder, lora_rank);
    }
    let decoder = if lora_enabled {
        // LoRA 检查点必须完整装载，不能退回 partial 后把旁路静默丢掉。
        match decoder.clone().try_load_file(&checkpoint) {
            Ok(loaded) => loaded,
            Err(strict_error) => {
                // 旧版本的 LoRA 检查点没有后来新增的采样/收尾卷积旁路。只要
                // 线性 LoRA 的秩能对上，就允许 partial 装载，让缺失的新旁路
                // 保持零初始化（折叠后等于基座），同时把情况写进日志。
                let compatible = inspect_checkpoint_adapters(&checkpoint)
                    .map(|adapters| adapters.lora && adapters.lora_rank == lora_rank)
                    .unwrap_or(false);
                if !compatible {
                    return Err(format!("读取 LoRA 检查点失败: {strict_error:#}"));
                }
                tracing::warn!(
                    "[导出] 检查点缺少新增卷积旁路，按旧版 LoRA 布局装载（新旁路为零初始化）"
                );
                let record = burn::store::ModuleRecord::load(&checkpoint)
                    .map_err(|error| format!("读取 LoRA 检查点失败: {error:#}"))?
                    .allow_partial(true);
                decoder
                    .try_load_record(record)
                    .map_err(|error| format!("读取 LoRA 检查点失败: {error:#}"))?
            },
        }
    } else {
        load_checkpoint(&decoder, &checkpoint)
            .map_err(|error| format!("读取检查点失败: {error:#}"))?
    };

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
        let base = base.clone();
        let root = root.clone();
        let checkpoint = checkpoint.clone();
        move || {
            let device = crate::ai_service::tts::finetune::burn_backend::train_device();
            // 先按基座重建编码器并装回时长旁路，再做语速校准：校准必须看到
            // 训练后的逐 token 时长，而不是基座预测。
            let encoder = if duration_enabled {
                let tokens_path = base.join("tokens.txt");
                if !tokens_path.is_file() {
                    return Err(format!(
                        "基座目录缺少 tokens.txt，无法导出时长适配: {}",
                        base.display()
                    ));
                }
                let lexicon_path = base.join("lexicon.txt");
                let lexicon = lexicon_path.is_file().then_some(lexicon_path);
                let frontend = TextFrontend::load(&tokens_path, lexicon.as_deref())
                    .map_err(|error| format!("加载基座词表失败: {error:#}"))?;
                let tensors = read_initializers(&onnx_path)
                    .map_err(|error| format!("读取基座权重失败: {error:#}"))?;
                let config = BurnBaseEncoderConfig {
                    n_vocab: frontend.tokens().len(),
                    duration_lora_rank: duration_rank,
                    ..Default::default()
                };
                let mut loaded = load_encoder_from_onnx(&config, &tensors, &device)?;
                loaded.set_duration_lora(
                    load_duration_adapter(&config, duration_rank, &checkpoint, &device)
                        .map_err(|error| format!("读取时长适配器失败: {error:#}"))?,
                );
                Some(loaded)
            } else {
                None
            };

            // 按所选风格簇估计语速倍数；导出时折进时长预测器偏置。
            // 校准只是增强项，失败（例如换用参数名不同的基座）不应阻断导出。
            let duration_scale = match &encoder {
                Some(encoder) => {
                    match estimate_style_duration_scale(&base, &root, style_cluster, encoder) {
                        Ok(scale) => scale,
                        Err(error) => {
                            tracing::warn!("[导出] 语速校准跳过: {error}");
                            None
                        },
                    }
                },
                None => None,
            };
            let encoder_weights = encoder
                .as_ref()
                .map(|encoder| encoder.folded_duration_weights())
                .unwrap_or_default();
            export_tuned_model_burn_with_adapters(
                &decoder,
                &encoder_weights,
                &onnx_path,
                &weight_map,
                &tmp,
                duration_scale,
            )
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
        if let Some(token) = guard.token.as_ref() {
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
    let mut writer =
        hound::WavWriter::new(&mut buf, spec).map_err(|e| format!("创建 WAV 编码器失败: {e}"))?;
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
        let samples =
            crate::ai_service::tts::finetune::resample::resample_mono(&samples, rate, 16_000);
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
