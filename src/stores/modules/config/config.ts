import { defineStore } from 'pinia'
import type { StructuredConfig } from '../../../api/services/config'
import { fetchEnvConfig, saveEnvConfig } from '../../../api/services/config'

type FlatConfig = Record<string, string>

function flattenStructuredConfig(structured: StructuredConfig): FlatConfig {
  const flat: FlatConfig = {}
  for (const categoryName of Object.keys(structured || {})) {
    const category = structured[categoryName]
    const subcategories = category?.subcategories || {}
    for (const subName of Object.keys(subcategories)) {
      const sub = subcategories[subName]
      const settings = sub?.settings || []
      for (const setting of settings) {
        if (setting?.key != null && setting?.value != null) {
          flat[String(setting.key)] = String(setting.value)
        }
      }
    }
  }
  return flat
}

export const useConfigStore = defineStore('config', {
  state: () => ({
    structured: {} as StructuredConfig,
    flat: {} as FlatConfig,
    loaded: false,
  }),
  getters: {
    getValue:
      (state) =>
      (key: string): string | undefined =>
        state.flat[key],
  },
  actions: {
    applyStructured(structured: StructuredConfig) {
      this.structured = structured
      this.flat = flattenStructuredConfig(structured)
      this.loaded = true
    },
    applyFlat(updates: FlatConfig) {
      this.flat = { ...this.flat, ...updates }
    },
    async loadFromServer() {
      const structured = await fetchEnvConfig()
      this.applyStructured(structured)
    },
    async saveValues(values: FlatConfig) {
      const res = await saveEnvConfig(values)
      this.applyFlat(values)
      return res
    },
  },
})
