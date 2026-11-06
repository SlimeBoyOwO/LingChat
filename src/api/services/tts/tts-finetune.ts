import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// 端侧音色微调的数据集接口。训练与导出尚未接入，这里只覆盖导入与管理。

export interface FinetuneDatasetSummary {
  dataset_id: string;
  items: number;
  items_with_text: number;
  total_secs: number;
  meets_minimum: boolean;
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
export function importFinetuneZip(
  zipPath: string,
  datasetId: string,
  overwrite = false,
) {
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
}

export interface FinetuneTrainOutcome {
  steps: number;
  initial_loss: number;
  final_loss: number;
  cancelled: boolean;
  checkpoint: string;
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
) {
  return invoke<FinetuneExportOutcome>("tts_finetune_export", {
    datasetId,
    baseModelDir,
    modelName,
  });
}

/** 开始训练，基座目录需包含 tokens.txt 与 lexicon.txt。 */
export function trainFinetuneDataset(
  datasetId: string,
  baseModelDir: string,
  epochs = 20,
  learningRate = 0.001,
  cleanupEvery = 50,
) {
  return invoke<FinetuneTrainOutcome>("tts_finetune_train", {
    datasetId,
    baseModelDir,
    epochs,
    learningRate,
    cleanupEvery,
  });
}

/** 取消正在进行的训练。 */
export function cancelFinetuneTraining() {
  return invoke<void>("tts_finetune_cancel");
}

/** 订阅训练进度。 */
export function onFinetuneProgress(
  callback: (progress: FinetuneTrainingProgress) => void,
): Promise<UnlistenFn> {
  return listen<FinetuneTrainingProgress>("tts:finetune-progress", (event) =>
    callback(event.payload),
  );
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
  return listen<FinetuneAnnotateProgress>(
    "tts:finetune-annotate-progress",
    (event) => callback(event.payload),
  );
}
