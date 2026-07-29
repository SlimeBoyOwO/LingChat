import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type { AssetKind, CatalogAsset } from '@/api/services/tts-catalog'
import {
  createProgressBus,
  type DownloadProgress,
  type ProgressListener,
} from '@/api/services/download-progress'

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

export function download(assetId: string): Promise<TtsLocalImportResult> {
  return invoke<TtsLocalImportResult>('tts_local_download', { assetId })
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
