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

/** 开始训练，基座目录需包含 tokens.txt 与 lexicon.txt。 */
export function trainFinetuneDataset(
  datasetId: string,
  baseModelDir: string,
  epochs = 20,
  learningRate = 0.001,
) {
  return invoke<FinetuneTrainOutcome>("tts_finetune_train", {
    datasetId,
    baseModelDir,
    epochs,
    learningRate,
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
