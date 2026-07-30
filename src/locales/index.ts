import { createI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/modules/settings'
import zhCN from './zh-CN'
import ja from './ja'

/** 支持的界面语言 */
export const SUPPORTED_LOCALES = [
  { value: 'zh-CN', label: '中文' },
  { value: 'ja', label: '日本語' },
] as const

export type AppLocale = (typeof SUPPORTED_LOCALES)[number]['value']

/** 与 stores/plugins/persist.ts 一致的统一设置存储键 */
const SETTINGS_STORAGE_KEY = 'lingchat-settings'

/** 从统一设置存储（stores/modules/settings，persist 插件）读取已保存的语言 */
function detectLocale(): AppLocale {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY)
    const saved = raw ? (JSON.parse(raw)?.display?.locale as string | undefined) : undefined
    if (saved === 'zh-CN' || saved === 'ja') return saved
  } catch {
    /* 解析失败退回默认语言 */
  }
  return 'zh-CN'
}

type MessageSchema = typeof zhCN

export const i18n = createI18n<[MessageSchema], AppLocale>({
  legacy: false,
  locale: detectLocale(),
  fallbackLocale: 'zh-CN',
  // ja 词条由分批迁移逐步补齐，缺失键运行时经 fallbackLocale 回落中文
  messages: { 'zh-CN': zhCN, ja: ja as MessageSchema },
})

/** 全局 composer 的 locale 引用（legacy:false 下运行时为可写 Ref） */
const globalLocale = i18n.global.locale as unknown as { value: AppLocale }

document.documentElement.lang = globalLocale.value

/** 切换界面语言：立即生效，经统一设置 store 持久化（persist 插件自动写 localStorage） */
export function setLocale(locale: AppLocale) {
  globalLocale.value = locale
  document.documentElement.lang = locale
  try {
    useSettingsStore().setUiLocale(locale)
  } catch (e) {
    console.warn('写入统一设置存储失败（非致命）:', e)
  }
}

/** 当前是否为日文界面（对话内容显示日语译文的开关） */
export function isJaLocale(): boolean {
  return globalLocale.value === 'ja'
}
