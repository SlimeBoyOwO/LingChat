import { invoke } from '@tauri-apps/api/core'

export type AsrSource = 'button' | 'hotkey' | 'auto'
export type SendMode = 'fill_only' | 'auto_send' | 'queue'
export type AsrPhase = 'idle' | 'recording' | 'recognizing'

export interface AsrResult {
  text: string
  language?: string
  confidence?: number
  provider_id: string
}

export interface ProviderConfig {
  api_key: string
  endpoint: string
  extra?: Record<string, string>
}

export interface AsrSettings {
  active_provider: string
  auto_listen: boolean
  hotkey_enabled: boolean
  hotkey_combination: string
  send_mode: SendMode
  provider_configs: Record<string, ProviderConfig>
}

export type ConfigFieldKind =
  | { name: 'text'; placeholder: string }
  | { name: 'secret' }
  | { name: 'url'; placeholder: string }

export interface AsrConfigField {
  key: string
  label: string
  kind: ConfigFieldKind
  required: boolean
}

export interface ProviderInfo {
  id: string
  display_name: string
  config_fields: AsrConfigField[]
}

export interface VadEvent {
  type: 'speech_started' | 'silence_started' | 'turn_candidate' | 'turn_sealed'
  silence_ms?: number
}

export const asrStartListening = (source: AsrSource) =>
  invoke<void>('asr_start_listening', { source })

export const asrStopListening = (source: AsrSource) =>
  invoke<void>('asr_stop_listening', { source })

export const asrVadProcessChunk = (pcm: number[]) => invoke<void>('asr_vad_process_chunk', { pcm })

export const asrRecognizeWav = (params: {
  providerId: string
  wavBytes: number[]
  languageHint?: string | null
}) =>
  invoke<AsrResult>('asr_recognize_wav', {
    providerId: params.providerId,
    wavBytes: params.wavBytes,
    languageHint: params.languageHint ?? null,
  })

export const asrCancel = () => invoke<void>('asr_cancel')

export const asrListProviders = () => invoke<ProviderInfo[]>('asr_list_providers')

export const asrGetSettings = () => invoke<AsrSettings>('asr_get_settings')

export const asrSetSettings = (settings: AsrSettings) =>
  invoke<void>('asr_set_settings', { settings })

export const asrTestProvider = (providerId: string) =>
  invoke<void>('asr_test_provider', { providerId })
