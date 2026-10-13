/**
 * 粒子特效注册表 —— 软件内置背景粒子的**单一真相源**。
 *
 * 设置页与桌宠的「背景特效」下拉从这里取值，作者只能从列表里选，杜绝拼错大小写
 * 导致特效被静默清空（上游复核要求：从前端获取粒子列表、防范输入错误）。
 *
 * 新增粒子：在此加一项 + 在 GameBackground.vue 加对应渲染分支 + 补两份词条。
 * 剧本能写的特效清单由本文件生成到 Rust（`pnpm gen:effects`，已挂在提交钩子上），
 * 不需要手抄。不跑生成也不会报错，只是编辑器里少了「这不是内置特效」那条警告。
 *
 * 设置页与剧本编辑器的下拉均已改读此处；GameBackground 仍是硬编码 v-if
 * （每个粒子的 props 各不相同），新增粒子时两边都要动。
 *
 * 特效分三层：氛围层（PARTICLE_EFFECTS）、天气层（WEATHER_EFFECTS）与恐怖层
 * （HORROR_EFFECTS，剧本演出的 DDLC 式恐怖特效，设置页不暴露但剧本可写）。
 * 氛围与天气各自单选、互不干扰，可以同时开着；恐怖层是叠加件，写在剧本的
 * background_effect 里（支持 "+" 组合多个恐怖特效），不抢前两层的槽位。
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
  { key: "Sakura", label: "樱花", i18n: "sakura" },
  { key: "Fireworks", label: "烟花", i18n: "fireworks" },
  // 星辉：原桌宠专属粒子，现已并入主界面可选项（i18n 沿用桌宠既有的「星辉 / Starglow」）
  { key: "BA", label: "星辉", i18n: "ba", petSupported: true },
  // 萤火虫：夜场氛围件，不依赖天气。桌宠圆头像里装不下它那套游走与落点，故不进桌宠
  { key: "Fireflies", label: "萤火虫", i18n: "fireflies" },
  // 流星雨：夜场氛围件，只画流星不带星空 —— 夜空背景多半本来就画了星星，再叠一层会重
  { key: "MeteorShower", label: "流星雨", i18n: "meteorShower" },
];

/**
 * 天气层。设置页里是独立的一组按钮，词条前缀也因此不同
 * （`settings.background.weather.<i18n>`，氛围层是 `...particle.<i18n>`）。
 * 天气都是全屏量级的，不进桌宠头像，所以没有 petSupported。
 *
 * 雨和雪从氛围层挪到了这里：同一种天气的不同档位分属两层，
 * 就会出现「小雨和雷阵雨同时开着」这种没人想要的状态。把同一族放在一起，
 * 单选本身就替我们排除了这种组合。
 */
export const WEATHER_EFFECTS: ParticleEffect[] = [
  { key: "Drizzle", label: "小雨", i18n: "drizzle" },
  { key: "Rain", label: "雨", i18n: "rain" },
  { key: "Thunderstorm", label: "雷阵雨", i18n: "thunderstorm" },
  { key: "Snow", label: "雪", i18n: "snow" },
  { key: "Blizzard", label: "雪暴", i18n: "blizzard" },
  { key: "Fog", label: "雾", i18n: "fog" },
];

/**
 * 恐怖层：剧本演出的 DDLC 式恐怖特效（Glitch/血滴/蓝屏…）。
 * 设置页刻意不暴露 —— 玩家不该给日常场景手动挂血滴；但剧本编辑器要能选、
 * 校验器要能认，所以并入 ALL_EFFECTS 一并生成到 Rust。
 * 全是全屏量级的叠加件，配合 background_effect 的 "+" 组合使用，
 * 不抢氛围层/天气层的单选槽位，也没有 petSupported。
 */
export const HORROR_EFFECTS: ParticleEffect[] = [
  { key: "Glitch", label: "画面故障", i18n: "glitch" },
  { key: "Shake", label: "画面震动", i18n: "shake" },
  { key: "Flash", label: "红色闪光", i18n: "flash" },
  { key: "Blackout", label: "舞台熄灯", i18n: "blackout" },
  { key: "Tear", label: "画面撕裂", i18n: "tear" },
  { key: "Static", label: "电视雪花", i18n: "static" },
  { key: "Invert", label: "颜色反转", i18n: "invert" },
  { key: "BloodDrip", label: "血滴", i18n: "bloodDrip" },
  { key: "Veins", label: "血管", i18n: "veins" },
  { key: "BSOD", label: "蓝屏", i18n: "bsod" },
  { key: "UiCorrupt", label: "界面损坏", i18n: "uiCorrupt" },
  { key: "BloodUI", label: "血色界面", i18n: "bloodUI" },
];

/** 恐怖特效的 key 列表：UI 侧用来识别「当前值里有没有恐怖特效」并做残留清理。 */
export const HORROR_EFFECT_KEYS = HORROR_EFFECTS.map((effect) => effect.key);

/** 全部内置特效。给编辑器下拉、大小写纠错与 Rust 清单生成用，不区分它属于哪一层。 */
export const ALL_EFFECTS: ParticleEffect[] = [...PARTICLE_EFFECTS, ...WEATHER_EFFECTS, ...HORROR_EFFECTS];

/** 这个特效是不是天气层的。剧本与存档只记一个特效值，靠它决定落到哪一层。 */
export const isWeatherEffect = (key: string): boolean =>
  WEATHER_EFFECTS.some((p) => p.key.toLowerCase() === key.toLowerCase());

/**
 * 给编辑器下拉用的选项：首项「无特效」对应引擎的清空值 None，
 * 其余为各特效。返回 { value, label } 以便下拉显示中文、写入英文 key。
 *
 * 三层都列出来：剧本作者不必关心引擎把它们放在哪一层。
 */
export const particleEffectOptions = (): { value: string; label: string }[] => [
  { value: "None", label: "无特效" },
  ...ALL_EFFECTS.map((p) => ({ value: p.key, label: p.label })),
];

/** 大小写不敏感地映射到特效注册表中的规范 key；组合特效（"+" 连接）逐段规范化。 */
export const canonicalEffectKey = (value: string): string | null => {
  const raw = value.trim();
  if (!raw || raw.toLowerCase() === "none") return "None";
  const canonical: string[] = [];
  for (const part of raw.split("+").map((item) => item.trim())) {
    const match = ALL_EFFECTS.find((effect) => effect.key.toLowerCase() === part.toLowerCase());
    if (!match) return null;
    canonical.push(match.key);
  }
  return canonical.join("+");
};
