/**
 * 粒子特效注册表 —— 软件内置特效的**单一真相源**。
 *
 * 编辑器的「背景特效」下拉从这里取值，作者只能从列表里选，杜绝拼错大小写
 * 导致特效被静默清空（上游复核要求：从前端获取粒子列表、防范输入错误）。
 *
 * 新增粒子：在此加一项 + 在 GameBackground.vue 加对应渲染分支即可，
 * 不必改后端（后端 background_effect_event 仅做大小写纠错 warn）。
 *
 * 暂未让 GameBackground 读这个表（本轮不动正式游玩渲染层）；粒子稳定后
 * 可让 GameBackground 也改读此处，消除硬编码 v-if。
 */
export interface ParticleEffect {
  /** 写进 YAML 的值，与 GameBackground 的 v-if 分支、引擎的 KNOWN_EFFECTS 对应 */
  key: string
  /** 编辑器下拉里给作者看的中文名 */
  label: string
}

export const PARTICLE_EFFECTS: ParticleEffect[] = [
  { key: 'StarField', label: '星空' },
  { key: 'Rain', label: '雨' },
  { key: 'Sakura', label: '樱花' },
  { key: 'Snow', label: '雪' },
  { key: 'Fireworks', label: '烟花' },
]

/**
 * 给编辑器下拉用的选项：首项「无特效」对应引擎的清空值 None，
 * 其余为各粒子。返回 { value, label } 以便下拉显示中文、写入英文 key。
 */
export const particleEffectOptions = (): { value: string; label: string }[] => [
  { value: 'None', label: '无特效' },
  ...PARTICLE_EFFECTS.map((p) => ({ value: p.key, label: p.label })),
]

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
  const v = value.trim()
  if (!v) return 'None'
  if (v.toLowerCase() === 'none') return 'None'
  return (
    PARTICLE_EFFECTS.find((p) => p.key.toLowerCase() === v.toLowerCase())?.key ?? null
  )
}
