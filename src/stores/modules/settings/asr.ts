import { defineStore } from 'pinia'
import {
  asrGetSettings,
  asrListProviders,
  asrSetSettings,
  type AsrPhase,
  type AsrResult,
  type AsrSettings,
  type AsrSource,
  type ProviderInfo,
  type VadEvent,
} from '@/api/services/asr'

const DEFAULT_SETTINGS: AsrSettings = {
  active_provider: 'openai-whisper',
  auto_listen: false,
  hotkey_enabled: false,
  hotkey_combination: 'Ctrl+Shift+Space',
  send_mode: 'fill_only',
  provider_configs: {},
}

export const useAsrStore = defineStore('asr', {
  state: () => ({
    settings: { ...DEFAULT_SETTINGS } as AsrSettings,
    phase: 'idle' as AsrPhase,
    activeSource: null as AsrSource | null,
    lastResult: null as AsrResult | null,
    lastError: null as string | null,
    providers: [] as ProviderInfo[],
    micState: 'idle' as 'idle' | 'recording' | 'denied',
    vadLoaded: false,
  }),
  actions: {
    async load() {
      try {
        this.settings = await asrGetSettings()
        this.providers = await asrListProviders()
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
    onTurnCandidate(_e: VadEvent) {
      /* 由 useAsrInput 处理 */
    },
    onTurnSealed(_e: VadEvent) {
      /* 由 useAsrInput 处理 */
    },
    onSpeechStarted() {
      /* 由 useAsrInput 处理 */
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
  persist: true,
})
