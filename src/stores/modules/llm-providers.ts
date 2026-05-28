import { defineStore } from 'pinia'
import {
  listLlmProviders,
  saveLlmProvider,
  deleteLlmProvider,
  setLlmRole,
  type LlmProviderConfig,
} from '@/api/services/llm-providers'

export const useLlmProvidersStore = defineStore('llm-providers', {
  state: () => ({
    providers: [] as LlmProviderConfig[],
    chatProviderId: null as string | null,
    translateProviderId: null as string | null,
    loaded: false,
  }),
  getters: {
    chatProvider: (state) =>
      state.providers.find((p) => p.id === state.chatProviderId) ?? null,
    translateProvider: (state) =>
      state.providers.find((p) => p.id === state.translateProviderId) ?? null,
    effectiveTranslateProvider: (state) => {
      if (state.translateProviderId) {
        return (
          state.providers.find((p) => p.id === state.translateProviderId) ??
          null
        )
      }
      return state.providers.find((p) => p.id === state.chatProviderId) ?? null
    },
    emptyProvider: () => (): LlmProviderConfig => ({
      id: '',
      label: '',
      provider: 'openai',
      model: '',
      api_key: '',
      base_url: '',
      temperature: null,
      top_p: null,
      enable_thinking: false,
    }),
  },
  actions: {
    async load() {
      try {
        const data = await listLlmProviders()
        this.providers = data.providers
        this.chatProviderId = data.chat_provider_id
        this.translateProviderId = data.translate_provider_id
        this.loaded = true
      } catch (e) {
        console.error('Failed to load LLM providers:', e)
      }
    },
    async saveProvider(provider: LlmProviderConfig) {
      await saveLlmProvider(provider)
      await this.load()
    },
    async deleteProvider(id: string) {
      await deleteLlmProvider(id)
      await this.load()
    },
    async assignRole(role: 'chat' | 'translate', providerId: string | null) {
      await setLlmRole(role, providerId)
      await this.load()
    },
  },
})
