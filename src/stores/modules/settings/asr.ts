import { defineStore } from 'pinia'
import {
  asrGetSettings,
  asrListModels,
  asrListProviders,
  asrSetSettings,
  type AsrSettings,
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
    // 会话运行态（phase/activeSource）由 useAsrInput 模块级状态持有——
    // 它与录音采集私有变量强绑定，放 store 会分裂为两份状态。
    // store 只放跨组件 UI 需要的状态：micState / vadLoaded / lastError。
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
        // 合并默认值：后端 settings.json 是权威（含 energy_warmup_ms 等全部
        // 设置字段，schema 已统一）；persist 恢复值只作未加载前的占位，
        // 后端数据 spread 在最后覆盖。localStorage 里旧的前端私有字段
        // 不参与决策（除被 excludePaths 剔除的 provider_configs）。
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
  // 注意：exclude 只滤顶层 key，provider_configs 嵌在 settings 里，
  // 必须用 excludePaths 深度剔除（否则 api_key 明文落 localStorage）。
  persist: {
    key: 'lingchat-asr',
    exclude: ['provider_configs'],
    excludePaths: ['settings.provider_configs'],
  },
})
