import { invoke } from '@tauri-apps/api/core'
import { createI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/modules/settings'
import zhCN from './zh-CN'
import zhHK from './zh-HK'
import ja from './ja'

/** 支持的界面语言 */
export const SUPPORTED_LOCALES = [
  { value: 'zh-CN', label: '中文' },
  { value: 'zh-HK', label: '繁體中文（香港）' },
  { value: 'ja', label: '日本語' },
] as const

export type AppLocale = (typeof SUPPORTED_LOCALES)[number]['value']

/** 内置词条（打包进前端，作为兜底与播种源） */
const BUNDLED: Record<AppLocale, Record<string, unknown>> = {
  'zh-CN': zhCN as Record<string, unknown>,
  'zh-HK': zhHK as Record<string, unknown>,
  ja: ja as Record<string, unknown>,
}

/** 与 stores/plugins/persist.ts 一致的统一设置存储键 */
const SETTINGS_STORAGE_KEY = 'lingchat-settings'

/** 从统一设置存储（stores/modules/settings，persist 插件）读取已保存的语言 */
function detectLocale(): AppLocale {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY)
    const saved = raw ? (JSON.parse(raw)?.display?.locale as string | undefined) : undefined
    if (SUPPORTED_LOCALES.some((l) => l.value === saved)) return saved as AppLocale
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
  // 各语言词条以 zh-CN 为基准 schema；缺失键运行时经 fallbackLocale 回落中文
  messages: { 'zh-CN': zhCN, 'zh-HK': zhHK as MessageSchema, ja: ja as MessageSchema },
})

/** 全局 composer 的 locale 引用（legacy:false 下运行时为可写 Ref） */
const globalLocale = i18n.global.locale as unknown as { value: AppLocale }

document.documentElement.lang = globalLocale.value

/** 深合并：override 覆盖 base（嵌套对象递归，其余直接覆盖，不修改 base） */
function deepMergeMessages(base: any, override: any): any {
  const out: Record<string, any> = { ...base }
  for (const [k, v] of Object.entries(override ?? {})) {
    if (v && typeof v === 'object' && !Array.isArray(v) && out[k] && typeof out[k] === 'object') {
      out[k] = deepMergeMessages(out[k], v)
    } else {
      out[k] = v
    }
  }
  return out
}

/**
 * 从数据目录 data/locales/<locale>.json 加载语言文件并与内置词条深合并。
 * 文件不存在时后端会用内置词条播种；用户编辑过的内容优先，缺失键用内置兜底。
 */
async function loadLocaleMessages(locale: AppLocale) {
  try {
    const json = await invoke<string>('get_locale_messages', {
      locale,
      // 缩进格式播种，方便用户直接编辑
      seedContent: JSON.stringify(BUNDLED[locale], null, 2),
    })
    const fileMsgs = JSON.parse(json)
    i18n.global.setLocaleMessage(locale, deepMergeMessages(BUNDLED[locale], fileMsgs))
  } catch (e) {
    console.warn(`加载语言文件失败（使用内置词条）: ${locale}`, e)
  }
}

// 启动时异步加载全部语言文件（加载完成前界面暂用内置词条）
for (const opt of SUPPORTED_LOCALES) void loadLocaleMessages(opt.value)

/** 切换界面语言：立即生效，经统一设置 store 持久化（persist 插件自动写 localStorage） */
export function setLocale(locale: AppLocale) {
  globalLocale.value = locale
  document.documentElement.lang = locale
  try {
    useSettingsStore().setUiLocale(locale)
  } catch (e) {
    console.warn('写入统一设置存储失败（非致命）:', e)
  }
  // 切语言时重读语言文件，用户刚编辑的内容立即生效
  void loadLocaleMessages(locale)
}

/** 当前是否为日文界面（对话内容显示日语译文的开关） */
export function isJaLocale(): boolean {
  return globalLocale.value === 'ja'
}
