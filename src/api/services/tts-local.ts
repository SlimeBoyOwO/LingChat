import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type AssetKind = 'bert' | 'voice'

export interface AssetEntry {
  id: string
  kind: AssetKind
  display_name: string
  language: string
  size_bytes: number
  sha256: string
  download_url: string
  source: string
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

export interface DownloadProgress {
  asset_id: string
  bytes_done: number
  total_bytes: number
  percent: number
}

export interface ImportOptions {
  voiceId?: string
  assetId?: 'deberta' | 'deberta-tokenizer'
}

export function status(): Promise<TtsLocalStatus> {
  return invoke<TtsLocalStatus>('tts_local_status')
}

export function listCatalog(): Promise<AssetEntry[]> {
  return invoke<AssetEntry[]>('tts_local_list_catalog')
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

export function download(assetId: string): Promise<TtsLocalImportResult> {
  return invoke<TtsLocalImportResult>('tts_local_download', { assetId })
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
}): Promise<number[]> {
  return invoke<number[]>('tts_local_synthesize_preview', params)
}

export function onDownloadProgress(
  callback: (progress: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>('tts://download-progress', (event) => callback(event.payload))
}
