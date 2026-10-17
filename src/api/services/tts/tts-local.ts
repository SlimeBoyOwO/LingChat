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

/** 热切换本地 TTS 推理设备（"cpu" | "gpu" | "npu" | "device:<id>"；DirectML 仅 Windows） */
export function setDevice(device: string): Promise<void> {
  return invoke<void>("tts_local_set_device", { device });
}

/** 获取当前推理设备（持久化配置） */
export function getDevice(): Promise<string> {
  return invoke<string>("tts_local_get_device");
}

/** 枚举 DirectML 推理设备（GPU 列表，供用户选择特定显卡） */
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

/** 删除 DeBERTa 共享模型（deberta.onnx + tokenizer.json），并卸载引擎 */
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

// ---------------------------------------------------------------------------
// Sherpa-ONNX 本地模型管理
// ---------------------------------------------------------------------------

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

/** 订阅 Sherpa 模型下载进度；返回取消订阅函数。 */
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

export function downloadSherpaModel(modelId: string): Promise<void> {
  return invoke<void>("sherpa_download_model", { modelId });
}

export function deleteSherpaModel(modelId: string): Promise<void> {
  return invoke<void>("sherpa_delete_model", { modelId });
}

/** 参考音频导入结果。 */
export interface SherpaRefAudioImport {
  /** 落盘后的本地路径（写回 `sherpa_onnx_ref_audio_path`）。 */
  path: string;
  /** 展示用文件名。 */
  file_name: string;
}

/**
 * 导入零样本克隆的参考音频。
 *
 * 后端会把文件复制进应用数据目录并统一转成 16-bit PCM WAV（sherpa 只吃这种格式，
 * Android 上 dialog 给的 `content://` URI 也无法直接读）；失败时可能抛出
 * `SHERPA_REF_AUDIO_UNSUPPORTED` / `SHERPA_REF_AUDIO_TOO_LONG` 两个错误码，
 * 由调用方翻译成提示。
 */
export function importSherpaRefAudio(path: string): Promise<SherpaRefAudioImport> {
  return invoke<SherpaRefAudioImport>("import_sherpa_ref_audio", { path });
}

/** 本次会话里已迁移过的 `content://` 参考音频（同一个 URI 只导入一次，别重复落盘）。 */
const migratedRefAudio = new Map<string, SherpaRefAudioImport>();

/**
 * 迁移历史遗留的 `content://` 参考音频路径（Android）。
 *
 * 早期版本把 dialog 给的路径原样写进角色设置，Android 上那是 `content://` URI：
 * Rust 的 `std::fs` 读不了，试听直接报错、聊天时的零样本克隆会静默退化。
 * 这里借已有的导入命令把它落到应用数据目录——Rust 侧经 SAF（ContentResolver）
 * 拿到真实文件名，并统一转码成 16-bit PCM WAV。
 *
 * 不是 `content://`、或迁移失败（URI 授权已失效等）时返回 `null`，调用方保留原值。
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

// ---------------------------------------------------------------------------
// Sherpa-ONNX 模型目录状态（目录固定为应用数据目录 data/sherpa_onnx_models，
// 无需外部存储权限，granted 恒为 true，仅用于向用户展示模型存放位置）
// ---------------------------------------------------------------------------

export interface SherpaStorageStatus {
  /** 是否可访问模型目录。目录位于应用数据目录，恒为 true。 */
  granted: boolean;
  /** 模型根目录（用于向用户展示模型存放位置）。 */
  model_root: string;
}

/** 查询 Sherpa-ONNX 模型目录状态（granted + 模型根目录路径）。 */
export function checkSherpaStoragePermission(): Promise<SherpaStorageStatus> {
  return invoke<SherpaStorageStatus>("check_sherpa_storage_permission");
}

/** 兼容保留：模型目录无需权限，恒返回已授权。 */
export function requestSherpaStoragePermission(): Promise<SherpaStorageStatus> {
  return invoke<SherpaStorageStatus>("request_sherpa_storage_permission");
}
