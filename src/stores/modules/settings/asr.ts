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
  active_provider: 'qwen-asr',
  auto_listen: false,
  send_mode: 'fill_only',
  stream_enabled: false,
  // 默认关闭：仅兜底全新用户（无 localStorage 记录时）；后端 load 结果与
  // persist 恢复值都会覆盖它
  voice_input_enabled: false,
  vad_silence_ms: 800,
  energy_warmup_ms: 100,
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
        // 合并默认值：后端 settings.json 只含后端字段（如 vad_silence_ms），
        // 纯前端参数（如 energy_warmup_ms）以 persist 恢复值为准（localStorage
        // 权威），缺失时才用默认值兜底——保证设置框与 useAsrInput 读取总有值，
        // 且用户改过的前端参数不被后端数据覆盖。
        this.settings = { ...DEFAULT_SETTINGS, ...this.settings, ...(await asrGetSettings()) }
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
