import effectManifest from "../../../../../shared/script-effects.json";

/**
 * 粒子特效注册表 —— 软件内置背景粒子的**单一真相源**。
 *
 * 设置页与桌宠的「背景特效」下拉从这里取值，作者只能从列表里选，杜绝拼错大小写
 * 导致特效被静默清空（上游复核要求：从前端获取粒子列表、防范输入错误）。
 *
 * 新增粒子：在此加一项 + 在 GameBackground.vue 加对应渲染分支 + 补两份词条。
 * 若还想让剧本能写这个特效，需同步 `shared/script-effects.json`
 * （Rust 与 Vue 共用的剧本特效清单，见下方 SCRIPT_EFFECTS），否则校验器会判它非法。
 *
 * GameBackground 仍是硬编码 v-if（每个粒子的 props 各不相同），新增粒子时两边都要动。
 */
export interface ParticleEffect {
  /** 写进设置的值，与 GameBackground 的 v-if 分支对应 */
  key: string;
  /** 下拉里给用户看的中文名（i18n 词条缺失时的兜底） */
  label: string;
  /**
   * 词条后缀：`settings.background.particle.<i18n>`（设置页）
   * 与 `scriptEditor.schema.particle.<i18n>`（剧本编辑器）共用。
   * 新增粒子时两处词条都要补。
   */
  i18n: string;
  /** 桌宠头像内是否渲染：桌宠只有一个圆形头像，全屏量级的特效（雨/樱花/雪/烟花）不适用 */
  petSupported?: boolean;
}

export const PARTICLE_EFFECTS: ParticleEffect[] = [
  { key: "StarField", label: "星空", i18n: "starField", petSupported: true },
  { key: "Rain", label: "雨", i18n: "rain" },
  { key: "Sakura", label: "樱花", i18n: "sakura" },
  { key: "Snow", label: "雪", i18n: "snow" },
  { key: "Fireworks", label: "烟花", i18n: "fireworks" },
  // 星辉：原桌宠专属粒子，现已并入主界面可选项（i18n 沿用桌宠既有的「星辉 / Starglow」）
  { key: "BA", label: "星辉", i18n: "ba", petSupported: true },
];

/** 剧本特效清单条目：前后端共用，唯一来源为 shared/script-effects.json。 */
export interface ScriptEffect {
  key: string;
  label: string;
  layer: "background" | "horror";
  horror: boolean;
}

/** 剧本可写的全部背景/恐怖特效（含设置页不暴露的恐怖特效，故与 PARTICLE_EFFECTS 分开）。 */
export const SCRIPT_EFFECTS: readonly ScriptEffect[] = effectManifest as ScriptEffect[];
export const BACKGROUND_EFFECTS = SCRIPT_EFFECTS.filter(
  (effect) => effect.layer === "background",
);
export const HORROR_EFFECT_KEYS = SCRIPT_EFFECTS.filter((effect) => effect.horror).map(
  (effect) => effect.key,
);

/** 编辑器下拉使用完整清单，避免前端用几项普通粒子覆盖 Rust 的恐怖特效选项。 */
export const particleEffectOptions = (): { value: string; label: string }[] => [
  { value: "None", label: "无特效" },
  ...SCRIPT_EFFECTS.map((effect) => ({ value: effect.key, label: effect.label })),
];

/** 大小写不敏感地映射到共享清单中的规范 key；组合特效逐段规范化。 */
export const canonicalEffectKey = (value: string): string | null => {
  const raw = value.trim();
  if (!raw || raw.toLowerCase() === "none") return "None";
  const canonical: string[] = [];
  for (const part of raw.split("+").map((item) => item.trim())) {
    const match = SCRIPT_EFFECTS.find(
      (effect) => effect.key.toLowerCase() === part.toLowerCase(),
    );
    if (!match) return null;
    canonical.push(match.key);
  }
  return canonical.join("+");
};
