import { defineStore } from 'pinia'
import {
  asrGetSettings,
  asrListModels,
  asrListProviders,
  asrSetSettings,
  type AsrPhase,
  type AsrResult,
  type AsrSettings,
  type AsrSource,
  type ModelInfo,
  type ProviderInfo,
  type VadEvent,
} from '@/api/services/asr'

const DEFAULT_SETTINGS: AsrSettings = {
  active_provider: 'openai-whisper',
  auto_listen: false,
  hotkey_enabled: false,
  hotkey_combination: 'Ctrl+Shift+Space',
  send_mode: 'fill_only',
  stream_enabled: false,
  provider_configs: {},
}

export const useAsrStore = defineStore('asr', {
  state: () => ({
    settings: { ...DEFAULT_SETTINGS } as AsrSettings,
    phase: 'idle' as AsrPhase,
    activeSource: null as AsrSource | null,
    lastResult: null as AsrResult | null,
    lastError: null as string | null,
    vadEvent: null as VadEvent | null,
    providers: [] as ProviderInfo[],
    models: [] as ModelInfo[],
    micState: 'idle' as 'idle' | 'recording' | 'denied',
    vadLoaded: false,
  }),
  actions: {
    async load() {
      try {
        this.settings = await asrGetSettings()
        this.providers = await asrListProviders()
        // 模型清单（按 active provider 拉取；provider 切换时由 SettingsAsr 重新拉）
        this.models = await asrListModels(this.settings.active_provider).catch(() => [])
      } catch (e) {
        console.warn('[ASR] load failed:', e)
      }
    },
    async save(s: AsrSettings) {
      try {
        await asrSetSettings(s)
        this.settings = s
      } catch (e) {
        console.warn('[ASR] save failed:', e)
        throw e
      }
    },
    onTurnCandidate(e: VadEvent) {
      this.vadEvent = e
    },
    onTurnSealed(e: VadEvent) {
      this.vadEvent = e
    },
    onSpeechStarted() {
      this.micState = 'recording'
    },
    onResult(r: AsrResult) {
      this.lastResult = r
    },
    onError(code: string) {
      this.lastError = code
    },
    setMicState(s: 'idle' | 'recording' | 'denied') {
      this.micState = s
    },
    setVadLoaded(v: boolean) {
      this.vadLoaded = v
    },
  },
  // api_key 唯一真相在后端 settings.json（tauri_plugin_store），
  // 不从 localStorage 持久化 provider_configs，避免明文 key 双副本。
  persist: { key: 'lingchat-asr', exclude: ['provider_configs'] },
})
