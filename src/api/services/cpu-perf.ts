import { invoke } from '@tauri-apps/api/core'

/** 性能等级枚举（与 Rust 端保持一致） */
export type PerfTier = 'Internet' | 'Low' | 'Medium' | 'High'

/** CPU 信息接口 */
export interface CpuInfo {
  brand: string
  tier: PerfTier
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

/** 获取性能等级的建议帧率上限 */
export function getSuggestedMaxFps(tier: PerfTier): number {
  const fpsMap: Record<PerfTier, number> = {
    Internet: 15,
    Low: 30,
    Medium: 60,
    High: 120,
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
