import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  /** 风格簇；空表示未聚类。 */
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

export function listFinetuneDatasets() {
  return invoke<FinetuneDatasetSummary[]>("tts_finetune_list");
}

export function importFinetuneZip(zipPath: string, datasetId: string, overwrite = false) {
  return invoke<FinetuneImportReport>("tts_finetune_import", {
    zipPath,
    datasetId,
    overwrite,
  });
}

export function getFinetuneStats(datasetId: string) {
  return invoke<FinetuneDatasetStats>("tts_finetune_stats", { datasetId });
}

/** 无监督风格聚类，簇 id 写回 dataset.json。 */
export function clusterFinetuneStyles(datasetId: string, clusters = 4) {
  return invoke<FinetuneStyleReport>("tts_finetune_cluster_styles", {
    datasetId,
    clusters,
  });
}

export function removeFinetuneDataset(datasetId: string) {
  return invoke<void>("tts_finetune_remove", { datasetId });
}

export interface FinetuneTrainingProgress {
  epoch: number;
  total_epochs: number;
  step: number;
  total_steps: number;
  loss: number;
  /** 验证集平均损失，epoch 末上报。 */
  val_loss?: number;
}

export interface FinetuneTrainOutcome {
  steps: number;
  initial_loss: number;
  final_loss: number;
  cancelled: boolean;
  checkpoint: string;
  /** 续训起始轮次；0 表示从头。 */
  resumed_from: number;
  /** 使用的风格簇；空表示全部数据。 */
  style_cluster?: number | null;
}

export interface FinetuneExportOutcome {
  model_name: string;
  model_path: string;
}

/** 导出为独立的 Sherpa 音色模型目录。 */
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

/** 开始训练；基座目录需含 tokens.txt 与 lexicon.txt。 */
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

export function cancelFinetuneTraining() {
  return invoke<void>("tts_finetune_cancel");
}

export interface FinetuneActiveState {
  training: boolean;
  dataset_id: string;
  latest: FinetuneTrainingProgress | null;
}

/** 查询训练状态，供页面重建后恢复 UI。 */
export function getActiveTraining() {
  return invoke<FinetuneActiveState>("tts_finetune_active");
}

export function onFinetuneProgress(
  callback: (progress: FinetuneTrainingProgress) => void,
): Promise<UnlistenFn> {
  return listen<FinetuneTrainingProgress>("tts:finetune-progress", (event) =>
    callback(event.payload),
  );
}

/** 订阅训练结束（含取消）。 */
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

/** 用云端 ASR 标注缺失文本的条目。 */
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

export function onFinetuneAnnotateProgress(
  callback: (progress: FinetuneAnnotateProgress) => void,
): Promise<UnlistenFn> {
  return listen<FinetuneAnnotateProgress>("tts:finetune-annotate-progress", (event) =>
    callback(event.payload),
  );
}
