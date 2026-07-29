// ============================================================
//  彩蛋文字数据模块
//  集中管理 LoadingTransition 中展示的加载台词 & 随机小贴士
//  具体文案词条见 locales/{zh-CN,ja}/api.ts 的 easterEggs 分组
// ============================================================

import { i18n } from '@/locales'

const t = (key: string): string => i18n.global.t(key)

// ---- 加载状态台词（在 50% / 70% / 90% 时随机切换） ----
export const statusTexts: string[] = [
  t('api.easterEggs.status.0'),
  t('api.easterEggs.status.1'),
  t('api.easterEggs.status.2'),
  t('api.easterEggs.status.3'),
  t('api.easterEggs.status.4'),
  t('api.easterEggs.status.5'),
  t('api.easterEggs.status.6'),
  t('api.easterEggs.status.7'),
  t('api.easterEggs.status.8'),
  t('api.easterEggs.status.9'),
  t('api.easterEggs.status.10'),
  t('api.easterEggs.status.11'),
]

/** 从 statusTexts 中随机选取一条不同于 currentText 的台词 */
export function pickRandomStatusText(currentText: string): string {
  const unused = statusTexts.filter((s) => s !== currentText)
  if (unused.length === 0) return currentText
  return unused[Math.floor(Math.random() * unused.length)]
}

// ---- 随机小贴士（按权重分类） ----
export interface TipCategory {
  name: string
  weight: number
  messages: string[]
}

export const tipCategories: TipCategory[] = [
  {
    name: '游戏提示',
    weight: 0.4,
    messages: [
      t('api.easterEggs.tips.game.0'),
      t('api.easterEggs.tips.game.1'),
      t('api.easterEggs.tips.game.2'),
      t('api.easterEggs.tips.game.3'),
      t('api.easterEggs.tips.game.4'),
      t('api.easterEggs.tips.game.5'),
      t('api.easterEggs.tips.game.6'),
      t('api.easterEggs.tips.game.7'),
      t('api.easterEggs.tips.game.8'),
      t('api.easterEggs.tips.game.9'),
      t('api.easterEggs.tips.game.10'),
      t('api.easterEggs.tips.game.11'),
      t('api.easterEggs.tips.game.12'),
      t('api.easterEggs.tips.game.13'),
      t('api.easterEggs.tips.game.14'),
      t('api.easterEggs.tips.game.15'),
      t('api.easterEggs.tips.game.16'),
      t('api.easterEggs.tips.game.17'),
      t('api.easterEggs.tips.game.18'),
      t('api.easterEggs.tips.game.19'),
      t('api.easterEggs.tips.game.20'),
      t('api.easterEggs.tips.game.21'),
      t('api.easterEggs.tips.game.22'),
    ],
  },
  {
    name: '求情广告',
    weight: 0.1,
    messages: [
      t('api.easterEggs.tips.ads.0'),
      t('api.easterEggs.tips.ads.1'),
      t('api.easterEggs.tips.ads.2'),
      t('api.easterEggs.tips.ads.3'),
      t('api.easterEggs.tips.ads.4'),
      t('api.easterEggs.tips.ads.5'),
      t('api.easterEggs.tips.ads.6'),
      t('api.easterEggs.tips.ads.7'),
    ],
  },
  {
    name: '开发者彩蛋',
    weight: 0.2,
    messages: [
      t('api.easterEggs.tips.dev.0'),
      t('api.easterEggs.tips.dev.1'),
      t('api.easterEggs.tips.dev.2'),
      t('api.easterEggs.tips.dev.3'),
      t('api.easterEggs.tips.dev.4'),
      t('api.easterEggs.tips.dev.5'),
      t('api.easterEggs.tips.dev.6'),
      t('api.easterEggs.tips.dev.7'),
      t('api.easterEggs.tips.dev.8'),
      t('api.easterEggs.tips.dev.9'),
      t('api.easterEggs.tips.dev.10'),
      t('api.easterEggs.tips.dev.11'),
      t('api.easterEggs.tips.dev.12'),
      t('api.easterEggs.tips.dev.13'),
      t('api.easterEggs.tips.dev.14'),
      t('api.easterEggs.tips.dev.15'),
      t('api.easterEggs.tips.dev.16'),
      t('api.easterEggs.tips.dev.17'),
      t('api.easterEggs.tips.dev.18'),
      t('api.easterEggs.tips.dev.19'),
      t('api.easterEggs.tips.dev.20'),
      t('api.easterEggs.tips.dev.21'),
      t('api.easterEggs.tips.dev.22'),
      t('api.easterEggs.tips.dev.23'),
      t('api.easterEggs.tips.dev.24'),
      t('api.easterEggs.tips.dev.25'),
      t('api.easterEggs.tips.dev.26'),
      t('api.easterEggs.tips.dev.27'),
    ],
  },
]

/** 按权重随机选分类，再从分类中随机选一条消息 */
export function pickRandomTip(): string {
  const totalWeight = tipCategories.reduce((sum, c) => sum + c.weight, 0)
  let r = Math.random() * totalWeight
  for (const cat of tipCategories) {
    r -= cat.weight
    if (r <= 0) {
      return cat.messages[Math.floor(Math.random() * cat.messages.length)]
    }
  }
  // fallback
  return tipCategories[0].messages[0]
}
