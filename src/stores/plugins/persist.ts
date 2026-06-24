/**
 * Pinia 持久化插件
 * 自动将 store 状态同步到 localStorage
 */
import type { PiniaPluginContext } from 'pinia'

// 持久化配置
interface PersistOptions {
  key?: string // 自定义存储键名
  exclude?: string[] // 排除的字段
}

// 扩展 Pinia 的 DefineStoreOptions
declare module 'pinia' {
  export interface DefineStoreOptionsBase<S, Store> {
    persist?: boolean | PersistOptions
  }
}

// 深度合并：target 的默认值 + source 的持久化值
function deepMerge(target: Record<string, any>, source: Record<string, any>): Record<string, any> {
  for (const key of Object.keys(source)) {
    if (
      source[key] &&
      typeof source[key] === 'object' &&
      !Array.isArray(source[key]) &&
      target[key] &&
      typeof target[key] === 'object' &&
      !Array.isArray(target[key])
    ) {
      deepMerge(target[key], source[key])
    } else {
      target[key] = source[key]
    }
  }
  return target
}

export function persist({ store, options }: PiniaPluginContext) {
  // 只有明确配置了 persist: true 的 store 才持久化
  if (!options.persist) return

  const persistOptions = typeof options.persist === 'object' ? options.persist : {}
  const storageKey = persistOptions.key || `lingchat-${store.$id}`
  const excludeFields = persistOptions.exclude || []

  // 页面加载时：从 localStorage 恢复
  const saved = localStorage.getItem(storageKey)
  if (saved) {
    try {
      const parsed = JSON.parse(saved)
      // 过滤掉排除的字段
      const filtered = excludeFields.length
        ? Object.fromEntries(Object.entries(parsed).filter(([key]) => !excludeFields.includes(key)))
        : parsed
      // 深度合并：确保新增的默认字段不会因旧持久化数据丢失
      const merged = deepMerge(JSON.parse(JSON.stringify(store.$state)), filtered)
      store.$patch(merged)
    } catch (e) {
      console.error(`恢复设置失败 (${storageKey}):`, e)
    }
  }

  // 变化时：自动保存到 localStorage
  store.$subscribe((mutation, state) => {
    try {
      // 过滤掉排除的字段
      const toSave = excludeFields.length
        ? Object.fromEntries(Object.entries(state).filter(([key]) => !excludeFields.includes(key)))
        : state
      localStorage.setItem(storageKey, JSON.stringify(toSave))
    } catch (e) {
      console.error(`保存设置失败 (${storageKey}):`, e)
    }
  })
}
