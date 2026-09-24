import { invoke } from "@tauri-apps/api/core";
import { createI18n } from "vue-i18n";
import { useSettingsStore } from "@/stores/modules/settings";
import zhCN from "./zh-CN";

/** 支持的界面语言 */
export const SUPPORTED_LOCALES = [
  { value: "zh-CN", label: "中文" },
  { value: "zh-HK", label: "繁體中文（香港）" },
  { value: "ja", label: "日本語" },
  { value: "en", label: "English" },
] as const;

export type AppLocale = (typeof SUPPORTED_LOCALES)[number]["value"];

/**
 * 内置词条（打包进前端，作为兜底与播种源）。
 * 只有基准语言 zh-CN 静态打包；其余语言动态 import（按需加载，降低主包体积与常驻内存）。
 */
const BUNDLED: Record<AppLocale, Record<string, unknown>> = {
  "zh-CN": zhCN as Record<string, unknown>,
  "zh-HK": undefined as unknown as Record<string, unknown>,
  ja: undefined as unknown as Record<string, unknown>,
  en: undefined as unknown as Record<string, unknown>,
};

/** 非基准语言的动态加载器（Vite 会将其编译为按需异步 chunk） */
const LAZY_LOADERS: Partial<
  Record<AppLocale, () => Promise<{ default: Record<string, unknown> }>>
> = {
  "zh-HK": () => import("./zh-HK"),
  ja: () => import("./ja"),
  en: () => import("./en"),
};

/** 取某语言的内置词条；非基准语言首次调用时动态加载并缓存 */
export async function ensureBundled(locale: AppLocale): Promise<Record<string, unknown>> {
  if (BUNDLED[locale]) return BUNDLED[locale];
  const loader = LAZY_LOADERS[locale];
  if (!loader) return BUNDLED["zh-CN"];
  const mod = await loader();
  BUNDLED[locale] = mod.default;
  return BUNDLED[locale];
}

/**
 * 内置词条版本：对单个语言的内置词条做轻量 hash。
 * 后端据它与 data/locales/<locale>.json 里的版本比对——版本不一致（即词条有更新）
 * 时自动用新内置词条重新播种，避免用户环境里早期播种的旧词条永远覆盖新词条。
 * 用户手动编辑词条不改变内置版本，编辑内容仍会被保留。
 */
function bundleVersionOf(messages: Record<string, unknown>): string {
  let h = 0;
  const s = JSON.stringify(messages);
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return h.toString(36);
}

/** 与 stores/plugins/persist.ts 一致的统一设置存储键 */
const SETTINGS_STORAGE_KEY = "lingchat-settings";

/** 从统一设置存储（stores/modules/settings，persist 插件）读取已保存的语言 */
function detectLocale(): AppLocale {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    const saved = raw ? (JSON.parse(raw)?.display?.locale as string | undefined) : undefined;
    if (SUPPORTED_LOCALES.some((l) => l.value === saved)) return saved as AppLocale;
  } catch {
    /* 解析失败退回默认语言 */
  }
  return "zh-CN";
}

type MessageSchema = typeof zhCN;

export const i18n = createI18n<[MessageSchema], AppLocale>({
  legacy: false,
  locale: detectLocale(),
  fallbackLocale: "zh-CN",
  // 各语言词条以 zh-CN 为基准 schema；缺失键运行时经 fallbackLocale 回落中文。
  // 非基准语言词条在 ensureBundled/loadLocaleMessages 完成前暂缺，回落显示中文。
  // 仅静态注册基准语言（其余运行时 setLocaleMessage 注入）；断言为完整表以满足泛型签名
  messages: { "zh-CN": zhCN } as Record<AppLocale, MessageSchema>,
});

/** 全局 composer 的 locale 引用（legacy:false 下运行时为可写 Ref） */
const globalLocale = i18n.global.locale as unknown as { value: AppLocale };

document.documentElement.lang = globalLocale.value;

/** 深合并：override 覆盖 base（嵌套对象递归，其余直接覆盖，不修改 base） */
function deepMergeMessages(base: any, override: any): any {
  const out: Record<string, any> = { ...base };
  for (const [k, v] of Object.entries(override ?? {})) {
    if (v && typeof v === "object" && !Array.isArray(v) && out[k] && typeof out[k] === "object") {
      out[k] = deepMergeMessages(out[k], v);
    } else {
      out[k] = v;
    }
  }
  return out;
}

/**
 * 从数据目录 data/locales/<locale>.json 加载语言文件并与内置词条深合并。
 * 文件不存在时后端会用内置词条播种；用户编辑过的内容优先，缺失键用内置兜底。
 * 播种内容带 __locale_version 标记：后端发现内置词条版本变化时会自动重新播种，
 * 修复旧版本残留词条覆盖新词条的问题（详见后端 api/locale.rs）。
 */
async function loadLocaleMessages(locale: AppLocale) {
  try {
    const bundled = await ensureBundled(locale);
    const json = await invoke<string>("get_locale_messages", {
      locale,
      // 缩进格式播种，方便用户直接编辑；__locale_version 仅供后端版本比对
      seedContent: JSON.stringify(
        { __locale_version: bundleVersionOf(bundled), ...bundled },
        null,
        2,
      ),
    });
    const fileMsgs = JSON.parse(json);
    // 版本标记是内部字段，不进界面词条
    delete fileMsgs.__locale_version;
    i18n.global.setLocaleMessage(locale, deepMergeMessages(bundled, fileMsgs));
  } catch (e) {
    console.warn(`加载语言文件失败（使用内置词条）: ${locale}`, e);
  }
}

// 启动时只异步加载当前语言的文件（加载完成前基准语言外的界面暂回落中文）
void loadLocaleMessages(globalLocale.value);

/** 切换界面语言：立即生效，经统一设置 store 持久化（persist 插件自动写 localStorage） */
export function setLocale(locale: AppLocale) {
  globalLocale.value = locale;
  document.documentElement.lang = locale;
  try {
    useSettingsStore().setUiLocale(locale);
  } catch (e) {
    console.warn("写入统一设置存储失败（非致命）:", e);
  }
  // 切语言时重读语言文件，用户刚编辑的内容立即生效
  void loadLocaleMessages(locale);
  // 繁体（香港）界面需要 OpenCC 转换器，切换时按需预热（懒加载，见 hkify）
  if (locale === "zh-HK") void ensureHkConverter();
}

/** 当前是否为日文界面（对话内容显示日语译文的开关） */
export function isJaLocale(): boolean {
  return globalLocale.value === "ja";
}

/**
 * 简→繁（港）转换器：繁体（香港）界面下把对话内容转繁体显示（仅显示层，不改数据）。
 * OpenCC 及其词典体积可观且仅 zh-HK 界面使用，因此动态加载、首次使用前预热；
 * 转换器就绪前 hkify 原样返回简体文本（短暂回落，不影响功能）。
 */
let hkConverter: ((text: string) => string) | null = null;
let hkConverterPromise: Promise<void> | null = null;

async function ensureHkConverter(): Promise<void> {
  if (hkConverter) return;
  hkConverterPromise ??= import("opencc-js").then((OpenCC) => {
    hkConverter = OpenCC.default.Converter({ from: "cn", to: "hk" });
  });
  await hkConverterPromise;
}

/** 繁体（香港）界面下将文本转为繁体；其他界面或空文本原样返回 */
export function hkify<T extends string | undefined>(text: T): T {
  if (!text || globalLocale.value !== "zh-HK") return text;
  // 转换器未就绪（首次切换到繁体后的一瞬）先原样返回简体
  return (hkConverter ? hkConverter(text) : text) as T;
}

// 启动即为繁体界面时预热转换器（与语言文件加载并行，不阻塞启动）
if (globalLocale.value === "zh-HK") void ensureHkConverter();
