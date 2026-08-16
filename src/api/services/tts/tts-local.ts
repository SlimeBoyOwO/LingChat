import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type { AssetKind, CatalogAsset } from '@/api/services/tts/tts-catalog'
import {
  createProgressBus,
  type DownloadProgress,
  type ProgressListener,
} from '@/api/services/tts/download-progress'

export type { AssetKind, CatalogAsset, DownloadProgress }

const progressBus = createProgressBus()
let progressUnlisten: UnlistenFn | null = null
let progressSubscription: Promise<void> | null = null

async function ensureProgressSubscription(): Promise<void> {
  if (progressUnlisten) return
  if (!progressSubscription) {
    progressSubscription = listen<DownloadProgress>(
      'tts://download-progress',
      (event) => {
        progressBus.dispatch(event.payload)
      },
    ).then((unlisten) => {
      progressUnlisten = unlisten
      if (progressBus.listenerCount === 0) {
        progressUnlisten()
        progressUnlisten = null
      }
    }).finally(() => {
      progressSubscription = null
    })
  }
  await progressSubscription
}

export function onDownloadProgress(listener: ProgressListener): () => void {
  void ensureProgressSubscription()
  const unsubscribe = progressBus.subscribe(listener)
  return () => {
    unsubscribe()
    if (progressBus.listenerCount === 0 && progressUnlisten) {
      progressUnlisten()
      progressUnlisten = null
    }
  }
}

export function listCatalog(): Promise<readonly CatalogAsset[]> {
  return invoke<readonly CatalogAsset[]>('tts_local_list_catalog')
}

export function download(assetId: string): Promise<TtsLocalImportResult[]> {
  return invoke<TtsLocalImportResult[]>('tts_local_download', { assetId })
}


export interface VoiceRecord {
  voice_id: string
  kind: string
  size_bytes: number
  path: string
  language: string | null
  display_name: string | null
  source: string | null
  has_style_vectors: boolean
}

export interface AssetRecord {
  asset_id: string
  kind: string
  size_bytes: number
  path: string
  language: string | null
  display_name: string | null
  source: string | null
}

export interface TtsLocalStatus {
  ready: boolean
  deberta_installed: boolean
  installed_voice_count: number
}

export interface LocalTtsSwitchStatus {
  configured_enabled: boolean
  effective_enabled: boolean
}

export interface TtsLocalInstallSnapshot {
  assets: AssetRecord[]
  voices: VoiceRecord[]
}

export interface TtsLocalImportResult {
  asset_id: string
  voice_id: string | null
  path: string
  bytes: number
  message: string
}

export interface ImportOptions {
  voiceId?: string
  assetId?: 'deberta' | 'deberta-tokenizer'
}

export function status(): Promise<TtsLocalStatus> {
  return invoke<TtsLocalStatus>('tts_local_status')
}

export function getEnabled(): Promise<LocalTtsSwitchStatus> {
  return invoke<LocalTtsSwitchStatus>('tts_local_get_enabled')
}

export function setEnabled(enabled: boolean): Promise<LocalTtsSwitchStatus> {
  return invoke<LocalTtsSwitchStatus>('tts_local_set_enabled', { enabled })
}

/** 热切换本地 TTS 推理设备（"cpu" | "gpu" | "npu" | "device:<id>"；DirectML 仅 Windows） */
export function setDevice(device: string): Promise<void> {
  return invoke<void>('tts_local_set_device', { device })
}

/** 获取当前推理设备（持久化配置） */
export function getDevice(): Promise<string> {
  return invoke<string>('tts_local_get_device')
}

/** 枚举 DirectML 推理设备（GPU 列表，供用户选择特定显卡） */
export interface InferenceDeviceInfo {
  id: number
  name: string
  vendorId: number
  deviceId: number
}

export function listDevices(): Promise<InferenceDeviceInfo[]> {
  return invoke<InferenceDeviceInfo[]>('tts_local_list_devices')
}

export function listInstalled(): Promise<TtsLocalInstallSnapshot> {
  return invoke<TtsLocalInstallSnapshot>('tts_local_list_installed')
}

export function importFromPath(
  path: string,
  options: ImportOptions = {},
): Promise<TtsLocalImportResult> {
  return invoke<TtsLocalImportResult>('tts_local_import_from_path', {
    path,
    voiceId: options.voiceId ?? null,
    assetId: options.assetId ?? null,
  })
}

export async function deleteVoice(voiceId: string): Promise<void> {
  await invoke('tts_local_delete_voice', { voiceId })
}

export function importStyleVectors(
  voiceId: string,
  path: string,
): Promise<TtsLocalImportResult> {
  return invoke<TtsLocalImportResult>('tts_local_import_style_vectors', {
    voiceId,
    path,
  })
}

export function synthesizePreview(params: {
  text: string
  voiceId: string
  lengthScale: number
  sdpRatio: number
}): Promise<Uint8Array> {
  return invoke<Uint8Array>('tts_local_synthesize_preview', params)
}
