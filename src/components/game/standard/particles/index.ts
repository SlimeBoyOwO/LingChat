/**
 * 粒子特效注册表 —— 软件内置特效的**单一真相源**。
 *
 * 编辑器的「背景特效」下拉从这里取值，作者只能从列表里选，杜绝拼错大小写
 * 导致特效被静默清空（上游复核要求：从前端获取粒子列表、防范输入错误）。
 *
 * 新增粒子：在此加一项 + 在 GameBackground.vue 加对应渲染分支 + 补两份词条。
 * 若还想让剧本能写这个特效，需同步 Rust 的 `KNOWN_EFFECTS`
 * （`background_effect_event.rs`），否则校验器会判它非法。
 *
 * 设置页与剧本编辑器的下拉均已改读此处；GameBackground 仍是硬编码 v-if
 * （每个粒子的 props 各不相同），新增粒子时两边都要动。
 *
 * 特效分两层：氛围层（PARTICLE_EFFECTS）与天气层（WEATHER_EFFECTS），
 * 两者各自单选、互不干扰，可以同时开着。天气天生是叠加的（雷阵雨 = 暴雨 + 闪电，
 * 以后还有雾），不该和雪、樱花抢同一个槽位。
 */
export interface ParticleEffect {
  /** 写进 YAML 的值，与 GameBackground 的 v-if 分支、引擎的 KNOWN_EFFECTS 对应 */
  key: string;
  /** 下拉里给作者看的中文名（i18n 词条缺失时的兜底） */
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
  { key: "Snow", label: "雪", i18n: "snow" },
  { key: "Fireworks", label: "烟花", i18n: "fireworks" },
  // 星辉：原桌宠专属粒子，现已并入主界面可选项（i18n 沿用桌宠既有的「星辉 / Starglow」）
  { key: "BA", label: "星辉", i18n: "ba", petSupported: true },
];

/**
 * 天气层。设置页里是独立的一组按钮，词条前缀也因此不同
 * （`settings.background.weather.<i18n>`，氛围层是 `...particle.<i18n>`）。
 * 天气都是全屏量级的，不进桌宠头像，所以没有 petSupported。
 *
 * 雨从氛围层挪到了这里：它和雷阵雨本质是同一种天气的两个档位，
 * 分属两层就会出现「同时开着雨和雷阵雨」这种没人想要的状态。
 */
export const WEATHER_EFFECTS: ParticleEffect[] = [
  { key: "Rain", label: "雨", i18n: "rain" },
  { key: "Thunderstorm", label: "雷阵雨", i18n: "thunderstorm" },
  { key: "Fog", label: "雾", i18n: "fog" },
];

/** 全部内置特效。给编辑器下拉与大小写纠错用，不区分它属于哪一层。 */
export const ALL_EFFECTS: ParticleEffect[] = [...PARTICLE_EFFECTS, ...WEATHER_EFFECTS];

/** 这个特效是不是天气层的。剧本与存档只记一个特效值，靠它决定落到哪一层。 */
export const isWeatherEffect = (key: string): boolean => WEATHER_EFFECTS.some((p) => p.key === key);

/**
 * 给编辑器下拉用的选项：首项「无特效」对应引擎的清空值 None，
 * 其余为各特效。返回 { value, label } 以便下拉显示中文、写入英文 key。
 *
 * 两层都列出来：剧本作者不必关心引擎把它们放在哪一层。
 */
export const particleEffectOptions = (): { value: string; label: string }[] => [
  { value: "None", label: "无特效" },
  ...ALL_EFFECTS.map((p) => ({ value: p.key, label: p.label })),
];

/**
 * 大小写自动纠错：把任意写法（starfield/STARFIELD…）映射到注册表里的规范 key
 * （大小写不敏感匹配）。上游明确要求「直接大小写自动纠错，在前端识别上实现」，
 * 而不是只告警——AI 写剧本或手改 YAML 常产出错误大小写，打开章节时在此纠回。
 *
 * - 命中已知粒子：返回规范 key（如 'StarField'）。
 * - 'none'/空：返回 'None'（清空值，不算纠错）。
 * - 未命中任何已知粒子：返回 null（交给 validate/runtime 的 warn，不强行改写）。
 */
export const canonicalEffectKey = (value: string): string | null => {
  const v = value.trim();
  if (!v) return "None";
  if (v.toLowerCase() === "none") return "None";
  return ALL_EFFECTS.find((p) => p.key.toLowerCase() === v.toLowerCase())?.key ?? null;
};
