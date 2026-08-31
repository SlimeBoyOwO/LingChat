import { i18n } from '@/locales'
import type { Achievement } from '@/stores/modules/ui/achievement'

// i18n.global.te/t 的键类型是字面量联合，动态拼接的 key 需要降级为 string 调用
const teRaw = i18n.global.te as unknown as (key: string) => boolean
const tRaw = i18n.global.t as unknown as (key: string) => string

/**
 * 内置成就的本地化标题。语言包按成就 id 提供译文（settings.achievement.items.<id>）；
 * 冒险脚本注册的动态成就不在语言包中，回退为后端下发的原文。
 */
export function achievementTitle(achievement: Pick<Achievement, 'id' | 'title'>): string {
  const key = `settings.achievement.items.${achievement.id}.title`
  return teRaw(key) ? tRaw(key) : achievement.title
}

/** 内置成就的本地化描述；动态成就回退为后端下发的原文 */
export function achievementDescription(
  achievement: Pick<Achievement, 'id' | 'description'>,
): string {
  const key = `settings.achievement.items.${achievement.id}.description`
  return teRaw(key) ? tRaw(key) : achievement.description
}
