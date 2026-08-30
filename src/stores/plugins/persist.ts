/**
 * Pinia 持久化插件
 * 自动将 store 状态同步到 localStorage
 */
import type { PiniaPluginContext } from 'pinia'

// 持久化配置
interface PersistOptions {
  key?: string // 自定义存储键名
  exclude?: string[] // 排除的字段（仅顶层 key）
  /** 点路径深度排除（如 `settings.provider_configs`）——exclude 只过滤顶层 key，
   *  嵌套对象里含敏感字段（api_key 等）时必须用路径排除才能真剔除 */
  excludePaths?: string[]
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

// 深度剔除指定点路径（含嵌套路径，如 'settings.provider_configs'）。
// 数组不递归（Pinia state 里的数组无需路径排除）。
function deepExclude(target: Record<string, any>, paths: string[]): Record<string, any> {
  if (!paths.length) return target
  const pathSet = new Set(paths)
  const strip = (obj: any, prefix: string): any => {
    if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return obj
    const out: Record<string, any> = {}
    for (const [key, value] of Object.entries(obj)) {
      const full = prefix ? `${prefix}.${key}` : key
      if (pathSet.has(full)) continue
      out[key] = strip(value, full)
    }
    return out
  }
  return strip(target, '')
}

export function persist({ store, options }: PiniaPluginContext) {
  // 只有明确配置了 persist: true 的 store 才持久化
  if (!options.persist) return

  const persistOptions = typeof options.persist === 'object' ? options.persist : {}
  const storageKey = persistOptions.key || `lingchat-${store.$id}`
  const excludeFields = persistOptions.exclude || []
  const excludePathFields = persistOptions.excludePaths || []

  // 页面加载时：从 localStorage 恢复
  const saved = localStorage.getItem(storageKey)
  if (saved) {
    try {
      const parsed = JSON.parse(saved)
      // 过滤掉排除的字段（顶层 + 点路径深度剔除）
      const filtered = excludeFields.length
        ? Object.fromEntries(Object.entries(parsed).filter(([key]) => !excludeFields.includes(key)))
        : parsed
      // 深度合并：确保新增的默认字段不会因旧持久化数据丢失
      const merged = deepMerge(JSON.parse(JSON.stringify(store.$state)), deepExclude(filtered, excludePathFields))
      store.$patch(merged)
    } catch (e) {
      console.error(`恢复设置失败 (${storageKey}):`, e)
    }
  }

  // 变化时：自动保存到 localStorage
  store.$subscribe((mutation, state) => {
    try {
      // 过滤掉排除的字段（顶层 + 点路径深度剔除）
      const filtered = excludeFields.length
        ? Object.fromEntries(Object.entries(state).filter(([key]) => !excludeFields.includes(key)))
        : state
      localStorage.setItem(storageKey, JSON.stringify(deepExclude(filtered, excludePathFields)))
    } catch (e) {
      console.error(`保存设置失败 (${storageKey}):`, e)
    }
  })
}
