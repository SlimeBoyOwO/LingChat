import { defineStore } from 'pinia'
import {
  listLlmProviders,
  saveLlmProvider,
  deleteLlmProvider,
  setLlmRole,
  switchLlm,
  type LlmProviderConfig,
} from '@/api/services/llm-providers'

export const useLlmProvidersStore = defineStore('llm-providers', {
  state: () => ({
    providers: [] as LlmProviderConfig[],
    chatProviderId: null as string | null,
    translateProviderId: null as string | null,
    godAgentProviderId: null as string | null,
    loaded: false,
  }),
  getters: {
    chatProvider: (state) =>
      state.providers.find((p) => p.id === state.chatProviderId) ?? null,
    translateProvider: (state) =>
      state.providers.find((p) => p.id === state.translateProviderId) ?? null,
    godAgentProvider: (state) =>
      state.providers.find((p) => p.id === state.godAgentProviderId) ?? null,
    effectiveGodAgentProvider: (state) => {
      if (state.godAgentProviderId) {
        return (
          state.providers.find((p) => p.id === state.godAgentProviderId) ??
          null
        )
      }
      return state.providers.find((p) => p.id === state.chatProviderId) ?? null
    },
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
      base_url: 'https://api.deepseek.com',
      temperature: null,
      top_p: null,
      enable_thinking: false,
      reasoning_effort: null,
    }),
  },
  actions: {
    async load() {
      try {
        const data = await listLlmProviders()
        this.providers = data.providers
        this.chatProviderId = data.chat_provider_id
        this.translateProviderId = data.translate_provider_id
        this.godAgentProviderId = data.god_agent_provider_id
        this.loaded = true
      } catch (e) {
        console.error('Failed to load LLM providers:', e)
      }
    },
    async saveProvider(provider: LlmProviderConfig) {
      await saveLlmProvider(provider)
      await this.load()
      // 任何变更（首次添加、修改 url/key/model）都触发热切换
      await switchLlm()
    },
    async deleteProvider(id: string) {
      await deleteLlmProvider(id)
      await this.load()
      // 删除后触发热切换（可能删除了正在使用的模型）
      await switchLlm()
    },
    async assignRole(role: 'chat' | 'translate' | 'god_agent', providerId: string | null) {
      await setLlmRole(role, providerId)
      await this.load()
      // 角色分配变更后触发热切换
      await switchLlm()
    },
  },
})
