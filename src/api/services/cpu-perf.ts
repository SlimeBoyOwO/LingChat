import { invoke } from '@tauri-apps/api/core'

/** 性能等级枚举（与 Rust 端保持一致） */
export type PerfTier = 'Internet' | 'Low' | 'Medium' | 'High'

/** CPU 信息接口 */
export interface CpuInfo {
  /** CPU 品牌字符串，例如 "Intel(R) Core(TM) i7-8550U CPU @ 1.80GHz" */
  brand: string
  /** 性能等级 */
  tier: PerfTier
  /** 是否为 ARM 等非 x86 无法识别的 CPU */
  is_unknown: boolean
  /** 未知 CPU 时的友好提示（仅在 is_unknown 为 true 时有值） */
  unknown_message: string | null
}

/** 获取 CPU 信息（优先使用缓存，首次运行时自动检测） */
export async function getCpuInfo(): Promise<CpuInfo> {
  return invoke<CpuInfo>('get_cpu_info')
}

/** 重新检测 CPU 性能（清除缓存后重新检测） */
export async function redetectCpu(): Promise<CpuInfo> {
  return invoke<CpuInfo>('redetect_cpu')
}

/** 获取性能等级的中文描述 */
export function getTierLabel(tier: PerfTier): string {
  const labels: Record<PerfTier, string> = {
    Internet: '上网本',
    Low: '低性能',
    Medium: '中性能',
    High: '高性能',
  }
  return labels[tier] ?? tier
}

/** 获取性能等级对应的颜色（CSS 颜色值） */
export function getPerfTierColor(tier: PerfTier): string {
  const colors: Record<PerfTier, string> = {
    Internet: '#ef4444',
    Low: '#f97316',
    Medium: '#eab308',
    High: '#22c55e',
  }
  return colors[tier] ?? '#888888'
}

/** 获取性能等级的建议帧率上限 */
export function getSuggestedMaxFps(tier: PerfTier): number {
  const fpsMap: Record<PerfTier, number> = {
    Internet: 15,
    Low: 25,
    Medium: 30,
    High: 60,
  }
  return fpsMap[tier] ?? 30
}

/** 获取性能等级的建议粒子比例 */
export function getSuggestedParticleScale(tier: PerfTier): number {
  const scaleMap: Record<PerfTier, number> = {
    Internet: 0.2,
    Low: 0.5,
    Medium: 0.8,
    High: 1.0,
  }
  return scaleMap[tier] ?? 0.5
}

/** 推荐的特效开关（根据性能等级自动关闭高开销特效） */
export interface RecommendedEffects {
  mainMenuStarsEnabled: boolean
  mainMenuMeteorsEnabled: boolean
  globalMouseTrailEnabled: boolean
  clickAnimationEnabled: boolean
}

export function getRecommendedEffects(tier: PerfTier): RecommendedEffects {
  switch (tier) {
    case 'Internet':
      return {
        mainMenuStarsEnabled: false,
        mainMenuMeteorsEnabled: false,
        globalMouseTrailEnabled: false,
        clickAnimationEnabled: false,
      }
    case 'Low':
      return {
        mainMenuStarsEnabled: true,
        mainMenuMeteorsEnabled: false,
        globalMouseTrailEnabled: false,
        clickAnimationEnabled: true,
      }
    case 'Medium':
    case 'High':
      return {
        mainMenuStarsEnabled: true,
        mainMenuMeteorsEnabled: true,
        globalMouseTrailEnabled: true,
        clickAnimationEnabled: true,
      }
  }
}
