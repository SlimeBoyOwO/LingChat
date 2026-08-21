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
  model: string
  extra?: Record<string, string>
}

export interface ModelInfo {
  id: string
  display_name: string
  supports_streaming: boolean
  is_default: boolean
}

export interface AsrSettings {
  active_provider: string
  auto_listen: boolean
  hotkey_enabled: boolean
  hotkey_combination: string
  send_mode: SendMode
  stream_enabled: boolean
  hotkey_toggle_auto_listen: boolean
  provider_configs: Record<string, ProviderConfig>
}

/** 与后端 `provider.rs` 的 `ConfigFieldKind`（snake_case 字符串）严格对齐 */
export type ConfigFieldKind = 'text' | 'password' | 'number' | 'boolean'

export interface AsrConfigField {
  key: string
  label: string
  kind: ConfigFieldKind
  required: boolean
  default_value?: string
  placeholder?: string
  hint?: string
}

export interface ProviderInfo {
  id: string
  display_name: string
  config_fields: AsrConfigField[]
  supports_streaming: boolean
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

export const asrListModels = (providerId: string) =>
  invoke<ModelInfo[]>('asr_list_models', { providerId })

export const asrGetSettings = () => invoke<AsrSettings>('asr_get_settings')

export const asrSetSettings = (settings: AsrSettings) =>
  invoke<void>('asr_set_settings', { settings })

export const asrTestProvider = (providerId: string) =>
  invoke<void>('asr_test_provider', { providerId })

export const asrStartStreaming = (params: { providerId: string; languageHint?: string | null }) =>
  invoke<void>('asr_start_streaming', {
    providerId: params.providerId,
    languageHint: params.languageHint ?? null,
  })

export const asrStreamAudioChunk = (pcm: number[]) => invoke<void>('asr_stream_audio_chunk', { pcm })

export const asrStopStreaming = () => invoke<AsrResult>('asr_stop_streaming')

/** 丢弃流式会话（异常路径清理用；不影响非流式在飞识别） */
export const asrCancelStreaming = () => invoke<void>('asr_cancel_streaming')

/** 注册系统级全局快捷键（后台也可触发；combo 格式 "Ctrl+Shift+Space"） */
export const asrRegisterHotkey = (combo: string) =>
  invoke<void>('asr_register_hotkey', { combo })

export const asrUnregisterHotkey = () => invoke<void>('asr_unregister_hotkey')
