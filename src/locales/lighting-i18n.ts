/**
 * 内置光影预设的显示文案词条化。
 *
 * 预设的名字 / 说明 / 心情关键词的单一真相源在 Rust 的 lighting_store（设置面板、
 * 剧本、AI 工具都从它取），所以这里不改数据、只做显示翻译：词条存在就用词条，
 * 查不到就回落原文 —— 自建预设没有词条，因此永远显示用户自己起的名字。
 * 沿用 schema-i18n 的先例。
 */
import { i18n } from "@/locales";
import zhCNSettings from "./zh-CN/settings";

const BASE = "settings.background.lighting";

/** 词条存在用词条，否则回落原文 */
const text = (key: string, fallback: string) =>
  i18n.global.te(key) ? String(i18n.global.t(key)) : fallback;

/** 中文标签 → 词条 slug：从简体词条反查，省掉一张容易和词条走样的映射表 */
const MOOD_SLUG: Record<string, string> = Object.fromEntries(
  Object.entries(zhCNSettings.background.lighting.moodTags).map(([slug, zh]) => [zh, slug]),
);

/** 预设里跟显示有关的字段（自建预设同样适用，只是全部走回落） */
export interface PresetTextLike {
  id: string;
  name: string;
  description?: string;
  mood?: string[];
}

export const presetNameOf = (p: PresetTextLike) => text(`${BASE}.presets.${p.id}.name`, p.name);

export const presetDescriptionOf = (p: PresetTextLike) =>
  text(`${BASE}.presets.${p.id}.description`, p.description ?? "");

export const presetMoodOf = (p: PresetTextLike) =>
  (p.mood ?? []).map((tag) => text(`${BASE}.moodTags.${MOOD_SLUG[tag]}`, tag));

/** 按 id 取显示名；查不到这个 id（预设已删）时回落到传进来的兜底文本。 */
export const presetNameById = (presets: PresetTextLike[], id: string, fallback = id) => {
  const preset = presets.find((p) => p.id === id);
  return preset ? presetNameOf(preset) : fallback;
};

/**
 * 搜索用：原文和当前语言的译文都拼进去，这样中文界面输英文、英文界面输中文
 * 都能命中（后端和 LLM 只认原文，界面显示的是译文，两边不能各搜各的）。
 */
export const presetSearchTextOf = (p: PresetTextLike) =>
  [
    p.name,
    presetNameOf(p),
    p.description ?? "",
    presetDescriptionOf(p),
    (p.mood ?? []).join(" "),
    presetMoodOf(p).join(" "),
  ]
    .join(" ")
    .toLowerCase();
