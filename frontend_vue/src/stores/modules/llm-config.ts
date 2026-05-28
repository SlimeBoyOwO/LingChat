import { defineStore } from 'pinia'
import type { LlmConfigSummary, LlmConfigScheme } from '../../api/services/llm-config'
import {
  listConfigs,
  getActiveConfig,
  getConfig as getConfigApi,
  switchConfig as switchConfigApi,
  saveConfig as saveConfigApi,
  deleteConfig as deleteConfigApi,
} from '../../api/services/llm-config'

export const useLlmConfigStore = defineStore('llm-config', {
  state: () => ({
    configs: [] as LlmConfigSummary[],
    activeName: '',
    activeConfig: null as LlmConfigScheme | null,
    loading: false,
  }),

  getters: {
    activeSummary: (state): LlmConfigSummary | undefined =>
      state.configs.find((c) => c.is_active),

    hasConfigs: (state) => state.configs.length > 0,
  },

  actions: {
    async load() {
      this.loading = true
      try {
        const [configList, active] = await Promise.all([
          listConfigs(),
          getActiveConfig(),
        ])
        this.configs = configList
        this.activeName = active.name
        this.activeConfig = active.config
      } catch (e) {
        console.error('加载 LLM 配置失败:', e)
      } finally {
        this.loading = false
      }
    },

    async switchTo(name: string) {
      await switchConfigApi(name)
      await this.load()
    },

    async save(name: string, scheme: LlmConfigScheme) {
      await saveConfigApi(name, scheme)
      await this.load()
    },

    async getConfig(name: string): Promise<LlmConfigScheme> {
      const res = await getConfigApi(name)
      return res.config
    },

    async remove(name: string) {
      await deleteConfigApi(name)
      await this.load()
    },

    emptyScheme(): LlmConfigScheme {
      return {
        config_name: '',
        config_description: '',
        main: {
          provider: 'webllm',
          model: '',
          api_key: '',
          base_url: 'https://api.deepseek.com/v1',
          proxy: '',
          temperature: 1.3,
          top_p: 0.9,
          enable_thinking: 'none',
        },
        translator: {
          provider: 'none',
          model: '',
          api_key: '',
          base_url: '',
        },
      }
    },
  },
})
