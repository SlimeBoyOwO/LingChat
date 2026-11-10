import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// 端侧音色微调的数据集接口。训练与导出尚未接入，这里只覆盖导入与管理。

export interface FinetuneStyleCluster {
  cluster: number;
  items: number;
  total_secs: number;
}

export interface FinetuneStyleReport {
  dataset_id: string;
  k: number;
  clusters: FinetuneStyleCluster[];
}

export interface FinetuneDatasetSummary {
  dataset_id: string;
  items: number;
  items_with_text: number;
  total_secs: number;
  meets_minimum: boolean;
  /** 已分配的无监督风格簇；空表示尚未聚类。 */
  style_clusters: FinetuneStyleCluster[];
}

export interface FinetuneDatasetStats {
  total_items: number;
  items_with_text: number;
  total_secs: number;
  min_required_secs: number;
  meets_minimum: boolean;
}

export interface FinetuneSkippedItem {
  name: string;
  reason: string;
}

export interface FinetuneImportReport {
  dataset_id: string;
  root: string;
  stats: FinetuneDatasetStats;
  skipped: FinetuneSkippedItem[];
  metadata_csv: string;
}

/** 列出已导入的数据集。 */
export function listFinetuneDatasets() {
  return invoke<FinetuneDatasetSummary[]>("tts_finetune_list");
}

/** 导入一个仅含音频的压缩包。 */
export function importFinetuneZip(zipPath: string, datasetId: string, overwrite = false) {
  return invoke<FinetuneImportReport>("tts_finetune_import", {
    zipPath,
    datasetId,
    overwrite,
  });
}

/** 读取单个数据集的统计。 */
export function getFinetuneStats(datasetId: string) {
  return invoke<FinetuneDatasetStats>("tts_finetune_stats", { datasetId });
}

/** 对数据集做无监督风格聚类，簇 id 会写回 dataset.json。 */
export function clusterFinetuneStyles(datasetId: string, clusters = 4) {
  return invoke<FinetuneStyleReport>("tts_finetune_cluster_styles", {
    datasetId,
    clusters,
  });
}

/** 删除数据集。 */
export function removeFinetuneDataset(datasetId: string) {
  return invoke<void>("tts_finetune_remove", { datasetId });
}

export interface FinetuneTrainingProgress {
  epoch: number;
  total_epochs: number;
  step: number;
  total_steps: number;
  loss: number;
  /** 本轮验证集平均损失；每个 epoch 末上报一次，中间为 undefined。 */
  val_loss?: number;
}

export interface FinetuneTrainOutcome {
  steps: number;
  initial_loss: number;
  final_loss: number;
  cancelled: boolean;
  checkpoint: string;
  /** 从第几轮开始续训；0 表示从头训练。 */
  resumed_from: number;
  /** 本次训练使用的风格簇；null/undefined 表示使用全部数据。 */
  style_cluster?: number | null;
}

export interface FinetuneExportOutcome {
  model_name: string;
  model_path: string;
}

/** 把训练好的数据集导出为一个独立的 Sherpa 音色模型目录。 */
export function exportFinetuneVoice(
  datasetId: string,
  baseModelDir: string,
  modelName: string,
  styleCluster?: number,
) {
  return invoke<FinetuneExportOutcome>("tts_finetune_export", {
    datasetId,
    baseModelDir,
    modelName,
    styleCluster,
  });
}

/** 开始训练，基座目录需包含 tokens.txt 与 lexicon.txt。 */
export function trainFinetuneDataset(
  datasetId: string,
  baseModelDir: string,
  epochs = 20,
  learningRate = 0.001,
  cleanupEvery = 25,
  maxFramesPerStep = 192,
  loraEnabled = false,
  loraRank = 8,
  adapterOnly = false,
  styleCluster?: number,
  durationAdapter = true,
) {
  return invoke<FinetuneTrainOutcome>("tts_finetune_train", {
    datasetId,
    baseModelDir,
    epochs,
    learningRate,
    cleanupEvery,
    maxFramesPerStep,
    loraEnabled,
    loraRank,
    adapterOnly,
    styleCluster,
    durationAdapter,
  });
}

/** 取消正在进行的训练。 */
export function cancelFinetuneTraining() {
  return invoke<void>("tts_finetune_cancel");
}

export interface FinetuneActiveState {
  /** 是否有训练正在运行。 */
  training: boolean;
  /** 正在训练的数据集 id（无训练时为空）。 */
  dataset_id: string;
  /** 最近一次上报的进度（无则为空）。 */
  latest: FinetuneTrainingProgress | null;
}

/** 查询当前是否有训练在跑及其最新进度，供页面重建后恢复训练 UI。 */
export function getActiveTraining() {
  return invoke<FinetuneActiveState>("tts_finetune_active");
}

/** 订阅训练进度。 */
export function onFinetuneProgress(
  callback: (progress: FinetuneTrainingProgress) => void,
): Promise<UnlistenFn> {
  return listen<FinetuneTrainingProgress>("tts:finetune-progress", (event) =>
    callback(event.payload),
  );
}

/** 订阅训练结束（含被取消）。 */
export function onFinetuneFinished(
  callback: (outcome: FinetuneTrainOutcome) => void,
): Promise<UnlistenFn> {
  return listen<FinetuneTrainOutcome>("tts:finetune-finished", (event) => callback(event.payload));
}

export interface FinetuneAnnotateReport {
  annotated: number;
  failed: number;
  skipped: number;
  stats: FinetuneDatasetStats;
}

export interface FinetuneAnnotateProgress {
  done: number;
  total: number;
  id: string;
}

/** 用云端 ASR 为缺失文本的音频自动标注。仅处理缺文本的条目。 */
export function autoAnnotateFinetuneDataset(
  datasetId: string,
  providerId: string,
  languageHint?: string,
) {
  return invoke<FinetuneAnnotateReport>("tts_finetune_auto_annotate", {
    datasetId,
    providerId,
    languageHint,
  });
}

/** 订阅自动标注逐条进度。 */
export function onFinetuneAnnotateProgress(
  callback: (progress: FinetuneAnnotateProgress) => void,
): Promise<UnlistenFn> {
  return listen<FinetuneAnnotateProgress>("tts:finetune-annotate-progress", (event) =>
    callback(event.payload),
  );
}
