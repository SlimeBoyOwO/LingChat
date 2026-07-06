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

/** localStorage 键名 */
const STORAGE_KEY = 'lingchat-cpu-perf'
/** 标记是否已完成首次自动配置 */
const CONFIGURED_KEY = 'lingchat-cpu-perf-configured'

/** 从 localStorage 读取缓存的 CPU 信息 */
function loadFromCache(): CpuInfo | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    return JSON.parse(raw) as CpuInfo
  } catch {
    return null
  }
}

/** 将 CPU 信息写入 localStorage */
function saveToCache(info: CpuInfo): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(info))
  } catch {
    // localStorage 不可用时静默失败
  }
}

/** 清除 localStorage 缓存 */
function clearCache(): void {
  try {
    localStorage.removeItem(STORAGE_KEY)
  } catch {
    // 静默失败
  }
}

/**
 * 获取 CPU 信息（优先使用 localStorage 缓存）
 *
 * 首次调用时调用 Tauri 后端检测，结果存入 localStorage；
 * 后续启动直接从 localStorage 读取，不再调用后端。
 */
export async function getCpuInfo(): Promise<CpuInfo> {
  // 优先读取 localStorage 缓存
  const cached = loadFromCache()
  if (cached) {
    return cached
  }

  // 缓存不存在，调后端检测
  const info = await invoke<CpuInfo>('get_cpu_info')
  saveToCache(info)
  return info
}

/**
 * 重新检测 CPU 性能（清除 localStorage 缓存后重新检测）
 */
export async function redetectCpu(): Promise<CpuInfo> {
  clearCache()
  const info = await invoke<CpuInfo>('redetect_cpu')
  saveToCache(info)
  return info
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

/**
 * 根据 CPU 性能等级自动调整画质设定（供 main.ts 初始化调用）
 *
 * - 仅首次启动时生效（缓存已在 localStorage）
 * - 不覆盖用户手动调整的值（仅当当前值仍为默认值时覆盖）
 * - 低性能设备自动关闭高开销特效
 */
export async function autoConfigureCpuPerformance(): Promise<void> {
  // 仅首次启动时执行自动配置（由 localStorage 标记控制）
  if (localStorage.getItem(CONFIGURED_KEY)) {
    return
  }

  try {
    const info = await getCpuInfo()
    const fps = getSuggestedMaxFps(info.tier)

    // 延迟导入避免循环依赖，等 pinia store 就绪
    const { useSettingsStore, DEFAULT_SETTINGS } = await import(
      '../../stores/modules/settings'
    )
    const settingsStore = useSettingsStore()

    if (settingsStore.display.meteorFps === DEFAULT_SETTINGS.display.meteorFps) {
      settingsStore.setMeteorFps(Math.min(fps, 60))
    }
    if (settingsStore.display.starsFps === DEFAULT_SETTINGS.display.starsFps) {
      settingsStore.setStarsFps(Math.min(fps, 60))
    }

    // 低性能设备自动关闭高开销特效
    if (info.tier === 'Internet' || info.tier === 'Low') {
      const effects = getRecommendedEffects(info.tier)
      if (
        settingsStore.display.mainMenuStarsEnabled ===
        DEFAULT_SETTINGS.display.mainMenuStarsEnabled
      ) {
        settingsStore.setMainMenuStarsEnabled(effects.mainMenuStarsEnabled)
      }
      if (
        settingsStore.display.mainMenuMeteorsEnabled ===
        DEFAULT_SETTINGS.display.mainMenuMeteorsEnabled
      ) {
        settingsStore.setMainMenuMeteorsEnabled(effects.mainMenuMeteorsEnabled)
      }
      if (
        settingsStore.display.globalMouseTrailEnabled ===
        DEFAULT_SETTINGS.display.globalMouseTrailEnabled
      ) {
        settingsStore.setGlobalMouseTrailEnabled(effects.globalMouseTrailEnabled)
      }
      if (
        settingsStore.display.clickAnimationEnabled ===
        DEFAULT_SETTINGS.display.clickAnimationEnabled
      ) {
        settingsStore.setClickAnimationEnabled(effects.clickAnimationEnabled)
      }
    }

    // 标记已完成首次自动配置
    localStorage.setItem(CONFIGURED_KEY, '1')

    console.log(
      `[CPU-Perf] ${info.brand} → ${info.tier}, 建议帧率 ${fps}FPS, 粒子比例 ${getSuggestedParticleScale(info.tier)}`,
    )
  } catch (e) {
    console.warn('[CPU-Perf] 自动配置失效，使用默认画质', e)
  }
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
