import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AssetKind, CatalogAsset } from "@/api/services/tts/tts-catalog";
import {
  createProgressBus,
  type DownloadProgress,
  type ProgressListener,
} from "@/api/services/tts/download-progress";

export type { AssetKind, CatalogAsset, DownloadProgress };

const progressBus = createProgressBus();
let progressUnlisten: UnlistenFn | null = null;
let progressSubscription: Promise<void> | null = null;

async function ensureProgressSubscription(): Promise<void> {
  if (progressUnlisten) return;
  if (!progressSubscription) {
    progressSubscription = listen<DownloadProgress>("tts://download-progress", (event) => {
      progressBus.dispatch(event.payload);
    })
      .then((unlisten) => {
        progressUnlisten = unlisten;
        if (progressBus.listenerCount === 0) {
          progressUnlisten();
          progressUnlisten = null;
        }
      })
      .finally(() => {
        progressSubscription = null;
      });
  }
  await progressSubscription;
}

export function onDownloadProgress(listener: ProgressListener): () => void {
  void ensureProgressSubscription();
  const unsubscribe = progressBus.subscribe(listener);
  return () => {
    unsubscribe();
    if (progressBus.listenerCount === 0 && progressUnlisten) {
      progressUnlisten();
      progressUnlisten = null;
    }
  };
}

export function listCatalog(): Promise<readonly CatalogAsset[]> {
  return invoke<readonly CatalogAsset[]>("tts_local_list_catalog");
}

export function download(assetId: string): Promise<TtsLocalImportResult[]> {
  return invoke<TtsLocalImportResult[]>("tts_local_download", { assetId });
}

export interface VoiceRecord {
  voice_id: string;
  kind: string;
  size_bytes: number;
  path: string;
  language: string | null;
  display_name: string | null;
  source: string | null;
  has_style_vectors: boolean;
}

export interface AssetRecord {
  asset_id: string;
  kind: string;
  size_bytes: number;
  path: string;
  language: string | null;
  display_name: string | null;
  source: string | null;
}

export interface TtsLocalStatus {
  ready: boolean;
  deberta_installed: boolean;
  installed_voice_count: number;
}

export interface LocalTtsSwitchStatus {
  configured_enabled: boolean;
  effective_enabled: boolean;
}

export interface TtsLocalInstallSnapshot {
  assets: AssetRecord[];
  voices: VoiceRecord[];
}

export interface TtsLocalImportResult {
  asset_id: string;
  voice_id: string | null;
  path: string;
  bytes: number;
  message: string;
}

export function status(): Promise<TtsLocalStatus> {
  return invoke<TtsLocalStatus>("tts_local_status");
}

export function getEnabled(): Promise<LocalTtsSwitchStatus> {
  return invoke<LocalTtsSwitchStatus>("tts_local_get_enabled");
}

export function setEnabled(enabled: boolean): Promise<LocalTtsSwitchStatus> {
  return invoke<LocalTtsSwitchStatus>("tts_local_set_enabled", { enabled });
}

/** 热切换推理设备（cpu/gpu/npu/device:<id>；DirectML 仅 Windows）。 */
export function setDevice(device: string): Promise<void> {
  return invoke<void>("tts_local_set_device", { device });
}

/** 当前推理设备（持久化配置）。 */
export function getDevice(): Promise<string> {
  return invoke<string>("tts_local_get_device");
}

/** 枚举 DirectML 设备。 */
export interface InferenceDeviceInfo {
  id: number;
  name: string;
  vendorId: number;
  deviceId: number;
}

export function listDevices(): Promise<InferenceDeviceInfo[]> {
  return invoke<InferenceDeviceInfo[]>("tts_local_list_devices");
}

export function listInstalled(): Promise<TtsLocalInstallSnapshot> {
  return invoke<TtsLocalInstallSnapshot>("tts_local_list_installed");
}

export function importFromPath(path: string, voiceId?: string): Promise<TtsLocalImportResult> {
  return invoke<TtsLocalImportResult>("tts_local_import_from_path", {
    path,
    voiceId: voiceId ?? null,
  });
}

export async function deleteVoice(voiceId: string): Promise<void> {
  await invoke("tts_local_delete_voice", { voiceId });
}

/** 删除 DeBERTa 共享模型并卸载引擎。 */
export function deleteDeberta(): Promise<void> {
  return invoke("tts_local_delete_deberta");
}

export function importStyleVectors(voiceId: string, path: string): Promise<TtsLocalImportResult> {
  return invoke<TtsLocalImportResult>("tts_local_import_style_vectors", {
    voiceId,
    path,
  });
}

export function synthesizePreview(params: {
  text: string;
  voiceId: string;
  lengthScale: number;
  sdpRatio: number;
}): Promise<Uint8Array> {
  return invoke<Uint8Array>("tts_local_synthesize_preview", params);
}

export interface SherpaOnnxModelRecord {
  id: string;
  display_name: string;
  model_type: string;
  language: string;
  voice: string;
  size_bytes: number;
  installed: boolean;
  path: string;
}

interface SherpaDownloadProgressPayload {
  asset_id: string;
  percent: number;
  bytes_done?: number;
  total_bytes?: number;
}

const sherpaProgressBus = createProgressBus();
let sherpaProgressUnlisten: UnlistenFn | null = null;
let sherpaProgressSubscription: Promise<void> | null = null;

async function ensureSherpaProgressSubscription(): Promise<void> {
  if (sherpaProgressUnlisten) return;
  if (!sherpaProgressSubscription) {
    sherpaProgressSubscription = listen<SherpaDownloadProgressPayload>(
      "tts://sherpa-download-progress",
      (event) => {
        sherpaProgressBus.dispatch({
          asset_id: event.payload.asset_id,
          percent: event.payload.percent,
          bytes_done: 0,
          total_bytes: 0,
        });
      },
    )
      .then((unlisten) => {
        sherpaProgressUnlisten = unlisten;
        if (sherpaProgressBus.listenerCount === 0) {
          sherpaProgressUnlisten();
          sherpaProgressUnlisten = null;
        }
      })
      .finally(() => {
        sherpaProgressSubscription = null;
      });
  }
  await sherpaProgressSubscription;
}

/** 订阅下载进度，返回取消函数。 */
export function onSherpaDownloadProgress(listener: ProgressListener): () => void {
  void ensureSherpaProgressSubscription();
  const unsubscribe = sherpaProgressBus.subscribe(listener);
  return () => {
    unsubscribe();
    if (sherpaProgressBus.listenerCount === 0 && sherpaProgressUnlisten) {
      sherpaProgressUnlisten();
      sherpaProgressUnlisten = null;
    }
  };
}

export function listSherpaModels(): Promise<SherpaOnnxModelRecord[]> {
  return invoke<SherpaOnnxModelRecord[]>("sherpa_list_models");
}

/** 说话人；sid 为合成时传给模型的下标。 */
export interface SherpaSpeaker {
  sid: number;
  /** metadata 中的名字，可能为空串。 */
  name: string;
}

export interface SherpaSpeakersInfo {
  num_speakers: number;
  speakers: SherpaSpeaker[];
}

/** 列出模型自带音色（读 ONNX metadata，不加载模型）；多文件模型会抛错，调用方退回语义档。 */
export function listSherpaSpeakers(modelId: string): Promise<SherpaSpeakersInfo> {
  return invoke<SherpaSpeakersInfo>("sherpa_list_speakers", { modelName: modelId });
}

export function downloadSherpaModel(modelId: string): Promise<void> {
  return invoke<void>("sherpa_download_model", { modelId });
}

export function deleteSherpaModel(modelId: string): Promise<void> {
  return invoke<void>("sherpa_delete_model", { modelId });
}

export interface SherpaRefAudioImport {
  /** 落盘后的本地路径。 */
  path: string;
  file_name: string;
}

/**
 * 导入参考音频：后端复制到应用数据目录并转 16-bit PCM WAV（sherpa 仅支持该格式，
 * Android content:// 无法直读）；失败抛 SHERPA_REF_AUDIO_* 错误码。
 */
export function importSherpaRefAudio(path: string): Promise<SherpaRefAudioImport> {
  return invoke<SherpaRefAudioImport>("import_sherpa_ref_audio", { path });
}

/** 会话内已迁移的 content:// 音频，避免重复落盘。 */
const migratedRefAudio = new Map<string, SherpaRefAudioImport>();

/**
 * 迁移历史 content:// 参考音频（Android）：std::fs 无法读取，经 SAF 落盘并转 PCM WAV。
 * 非 content:// 或迁移失败返回 null，调用方保留原值。
 */
export async function migrateSherpaRefAudio(path: string): Promise<SherpaRefAudioImport | null> {
  if (!path.startsWith("content://")) return null;
  const cached = migratedRefAudio.get(path);
  if (cached) return cached;
  try {
    const imported = await importSherpaRefAudio(path);
    migratedRefAudio.set(path, imported);
    console.log("[tts-local] 已迁移 content:// 参考音频:", path, "->", imported.path);
    return imported;
  } catch (error) {
    console.error("[tts-local] 迁移 content:// 参考音频失败:", error);
    return null;
  }
}

// 模型目录固定为 data/sherpa_onnx_models（应用数据目录，无需外部存储权限）。

export interface SherpaStorageStatus {
  /** 模型目录恒可访问（应用数据目录）。 */
  granted: boolean;
  /** 模型根目录。 */
  model_root: string;
}

/** 查询模型目录状态。 */
export function checkSherpaStoragePermission(): Promise<SherpaStorageStatus> {
  return invoke<SherpaStorageStatus>("check_sherpa_storage_permission");
}

/** 兼容保留：恒返回已授权。 */
export function requestSherpaStoragePermission(): Promise<SherpaStorageStatus> {
  return invoke<SherpaStorageStatus>("request_sherpa_storage_permission");
}
