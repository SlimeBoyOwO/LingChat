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
 * 内置词条加载器。只有默认语言（zh-CN）同步进首屏，其余语言包在切换到
 * 对应语言时才动态加载，避免中文用户为首屏用不到的三份词条付费。
 */
const BUNDLED_LOADERS: Record<AppLocale, () => Promise<Record<string, unknown>>> = {
  "zh-CN": async () => zhCN as Record<string, unknown>,
  "zh-HK": () => import("./zh-HK").then((m) => m.default as Record<string, unknown>),
  ja: () => import("./ja").then((m) => m.default as Record<string, unknown>),
  en: () => import("./en").then((m) => m.default as Record<string, unknown>),
};

const bundledCache = new Map<AppLocale, Record<string, unknown>>([
  ["zh-CN", zhCN as Record<string, unknown>],
]);
const bundledPending = new Map<AppLocale, Promise<Record<string, unknown>>>();

async function loadBundled(locale: AppLocale): Promise<Record<string, unknown>> {
  const cached = bundledCache.get(locale);
  if (cached) return cached;
  let pending = bundledPending.get(locale);
  if (!pending) {
    pending = BUNDLED_LOADERS[locale]()
      .then((messages) => {
        bundledCache.set(locale, messages);
        return messages;
      })
      .finally(() => bundledPending.delete(locale));
    bundledPending.set(locale, pending);
  }
  return pending;
}

/**
 * 内置词条版本：按语言分别对内置词条做轻量 hash。
 * 后端据它与 data/locales/*.json 里的版本比对——版本不一致（即该语言词条有更新）
 * 时自动用新内置词条重新播种，避免用户环境里早期播种的旧词条永远覆盖新词条。
 * 用户手动编辑词条不改变内置版本，编辑内容仍会被保留。
 *
 * 按语言分版本（而非全语言合并成一个 hash）既能在语言包按需加载后计算，
 * 也避免只改了某一种语言时把其他语言的用户编辑一并冲掉。
 */
const versionCache = new Map<AppLocale, string>();

function bundleVersion(locale: AppLocale, messages: Record<string, unknown>): string {
  const cached = versionCache.get(locale);
  if (cached) return cached;
  let h = 0;
  const s = JSON.stringify(messages);
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  const version = h.toString(36);
  versionCache.set(locale, version);
  return version;
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
  // 其余语言包在首次切换到该语言时才加载并挂载；类型上仍按完整语言表声明
  messages: { "zh-CN": zhCN } as unknown as Record<AppLocale, MessageSchema>,
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
  let bundled: Record<string, unknown>;
  try {
    bundled = await loadBundled(locale);
  } catch (e) {
    console.warn(`加载语言包失败（使用中文兜底）: ${locale}`, e);
    return;
  }
  // 先把内置词条挂上，后端读取失败时界面也能正常显示（而非回落到中文）
  i18n.global.setLocaleMessage(locale, bundled as unknown as MessageSchema);
  try {
    const json = await invoke<string>("get_locale_messages", {
      locale,
      // 缩进格式播种，方便用户直接编辑；__locale_version 仅供后端版本比对
      seedContent: JSON.stringify(
        { __locale_version: bundleVersion(locale, bundled), ...bundled },
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

/** 载入指定语言的内置词条（繁体界面下同时预热转换器）；供首屏挂载前调用 */
export async function prepareLocale(locale: AppLocale): Promise<void> {
  if (locale === "zh-HK") void ensureHkConverter();
  try {
    const bundled = await loadBundled(locale);
    i18n.global.setLocaleMessage(locale, bundled as unknown as MessageSchema);
  } catch (e) {
    console.warn(`加载语言包失败，暂用中文兜底: ${locale}`, e);
  }
}

/** 首屏语言就绪：内置词条已挂载后才 resolve（非中文界面会等待语言包动态加载） */
export const localeReady: Promise<void> = prepareLocale(globalLocale.value).finally(() => {
  // 语言文件读取较慢，不阻塞挂载；读到后覆盖用户编辑过的词条
  void loadLocaleMessages(globalLocale.value);
});

/** 切换界面语言：加载语言包后再切换，避免界面短暂回落到中文 */
export function setLocale(locale: AppLocale) {
  try {
    useSettingsStore().setUiLocale(locale);
  } catch (e) {
    console.warn("写入统一设置存储失败（非致命）:", e);
  }
  if (locale === "zh-HK") void ensureHkConverter();
  // 先载入该语言词条再切换；切完再重读语言文件，用户刚编辑的内容立即生效
  void prepareLocale(locale).finally(() => {
    globalLocale.value = locale;
    document.documentElement.lang = locale;
    void loadLocaleMessages(locale);
  });
}

/** 当前是否为日文界面（对话内容显示日语译文的开关） */
export function isJaLocale(): boolean {
  return globalLocale.value === "ja";
}

/**
 * 简→繁（港）转换器。整包 opencc-js 只在繁体界面下才有用，
 * 这里按需加载，中文/英文/日文用户不再为首屏解析它的词表。
 */
type HkConverter = (text: string) => string;
let toHk: HkConverter | null = null;
let hkPending: Promise<void> | null = null;

function ensureHkConverter(): Promise<void> {
  if (toHk) return Promise.resolve();
  if (!hkPending) {
    hkPending = import("opencc-js")
      .then(({ default: OpenCC }) => {
        toHk = OpenCC.Converter({ from: "cn", to: "hk" });
      })
      .catch((e) => {
        console.warn("加载繁简转换器失败，繁体界面暂按简体显示", e);
      });
  }
  return hkPending;
}

/** 繁体（香港）界面下将文本转为繁体；其他界面或空文本原样返回 */
export function hkify<T extends string | undefined>(text: T): T {
  if (!text || globalLocale.value !== "zh-HK") return text;
  // 转换器尚未加载完成时先原样返回，避免阻塞对话渲染
  return toHk ? (toHk(text) as T) : text;
}
