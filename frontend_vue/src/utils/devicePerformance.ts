/**
 * 设备性能检测工具
 * 通过CPU和GPU基准测试将设备分为三档，移动设备有特调规则
 */
import type { DisplaySettings } from '@/stores/modules/settings'

export interface DeviceProfile {
  tier: 'low' | 'medium' | 'high'
  cpuScore: number // CPU基准测试耗时(ms)，越小越好
  gpuScore: number // GPU基准测试得分，越大越好
  cpuCores: number
  memory: number // GB
  isMobile: boolean
  detectedAt: string // 检测时间戳
}

const STORAGE_KEY = 'lingchat-device-profile'

/**
 * 检测是否为移动设备
 */
export function isMobileDevice(): boolean {
  return /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
    navigator.userAgent
  )
}

/**
 * CPU基准测试：数学运算 + DOM操作（约50-100ms）
 * 返回执行耗时，越小性能越好
 */
/**
 * CPU基准测试（约50-200ms）
 * 测试数学运算和DOM操作性能
 * 返回执行耗时，越小性能越好
 */
export function cpuBenchmark(): number {
  const iterations = 50000 // 减少迭代次数，避免低端设备耗时过长
  const start = performance.now()

  // 数学运算测试（浮点运算密集）
  let result = 0
  for (let i = 0; i < iterations; i++) {
    // 混合使用不同运算，模拟实际JS执行
    result += Math.sqrt(i) * Math.sin(i * 0.1) * Math.cos(i * 0.1)
    if (i % 100 === 0) {
      result += Math.pow(i % 50, 2)
    }
  }

  // DOM操作测试（模拟Vue的DOM更新）
  const testDiv = document.createElement('div')
  testDiv.style.position = 'absolute'
  testDiv.style.visibility = 'hidden'
  document.body.appendChild(testDiv)

  for (let i = 0; i < 500; i++) { // 减少重排次数
    testDiv.style.transform = `translateX(${i}px) translateY(${i}px) scale(${1 + i * 0.001})`
    testDiv.style.opacity = `${0.5 + (i % 50) / 100}`
    // 强制重排
    void testDiv.offsetHeight
  }

  document.body.removeChild(testDiv)

  // 防止结果被优化掉
  console.assert(result > 0, 'CPU benchmark result should be positive')

  return performance.now() - start
}

/**
 * GPU基准测试：Canvas绘制性能（约50-100ms）
 * 返回绘制得分，越大性能越好
 */
export function gpuBenchmark(): number {
  const canvas = document.createElement('canvas')
  canvas.width = 800
  canvas.height = 600
  const ctx = canvas.getContext('2d')

  if (!ctx) {
    // 无法获取Canvas上下文，可能是低端设备
    return 0
  }

  const start = performance.now()
  const testDuration = 80 // 测试持续80ms
  let frames = 0
  let totalObjects = 0

  // 绘制测试：不断增加绘制对象，测试GPU处理能力
  while (performance.now() - start < testDuration) {
    frames++

    // 每帧绘制逐渐增多的对象
    const objectsThisFrame = 50 + frames * 5
    totalObjects += objectsThisFrame

    ctx.clearRect(0, 0, 800, 600)

    // 绘制矩形（测试填充率）
    for (let i = 0; i < objectsThisFrame; i++) {
      ctx.fillStyle = `hsl(${i % 360}, 50%, 50%)`
      ctx.fillRect(
        Math.random() * 800,
        Math.random() * 600,
        10 + Math.random() * 20,
        10 + Math.random() * 20
      )
    }

    // 绘制路径（测试路径渲染）
    for (let i = 0; i < objectsThisFrame / 2; i++) {
      ctx.strokeStyle = `hsl(${(i + 180) % 360}, 50%, 50%)`
      ctx.lineWidth = 2
      ctx.beginPath()
      ctx.arc(Math.random() * 800, Math.random() * 600, 20, 0, Math.PI * 2)
      ctx.stroke()
    }
  }

  // 得分 = 总绘制对象数 / 帧数 * 帧数
  // 高性能GPU能处理更多对象且保持高帧数
  return Math.round(totalObjects * (frames / 10))
}

/**
 * 综合性能评级
 * 基于实际测试数据校准：
 * - 骁龙410（低端）：CPU 414ms，GPU ~5203（旧）
 * - 目标：高端桌面 >100fps GPU测试，低端移动 <30fps
 */
function determineTier(
  cpuScore: number,
  gpuScore: number,
  cpuCores: number,
  memory: number,
  isMobile: boolean
): DeviceProfile['tier'] {
  // === 硬约束快速判定 ===

  // CPU耗时>400ms：JS执行性能严重不足（即使是移动设备）
  if (cpuScore > 400) {
    return 'low'
  }

  // GPU得分<300：无法流畅渲染
  if (gpuScore < 300) {
    return 'low'
  }

  // === 主要判定逻辑 ===

  // CPU评分（耗时越小越好）
  // 移动设备放宽标准（移动端JS引擎优化不同）
  const cpuFactor = isMobile ? 1.3 : 1.0
  const adjustedCpu = cpuScore / cpuFactor

  // <50ms=100, 50-100ms=70, 100-200ms=40, 200-300ms=20, >300ms=10
  let cpuRating = 0
  if (adjustedCpu < 50) cpuRating = 100
  else if (adjustedCpu < 100) cpuRating = 70
  else if (adjustedCpu < 200) cpuRating = 40
  else if (adjustedCpu < 300) cpuRating = 20
  else cpuRating = 10

  // GPU评分（得分越大越好）
  // 基于骁龙410得分~5200调整阈值
  // 桌面：>8000高，3000-8000中，<3000低
  // 移动：>10000可能虚高，需要更严格
  let gpuRating = 0
  if (gpuScore > 8000) gpuRating = 100
  else if (gpuScore > 4000) gpuRating = 70
  else if (gpuScore > 2000) gpuRating = 40
  else gpuRating = 20

  // === 综合评分 ===
  // CPU权重更高（JS执行是瓶颈）
  const totalScore = cpuRating * 0.6 + gpuRating * 0.4

  // === 调整因子 ===

  // 移动设备降档（浏览器性能比桌面差）
  const mobilePenalty = isMobile ? 15 : 0

  // 硬件信息调整（核心数和内存更可靠）
  let hardwareBonus = 0
  if (cpuCores >= 8) hardwareBonus += 10
  else if (cpuCores >= 6) hardwareBonus += 5

  if (memory >= 8) hardwareBonus += 5
  else if (memory >= 6) hardwareBonus += 3

  const finalScore = totalScore + hardwareBonus - mobilePenalty

  // === 分级标准 ===
  // High: 必须CPU性能好 + GPU达标
  if (finalScore >= 75 && adjustedCpu < 100 && gpuScore > 4000) return 'high'
  // Medium: 基本可用
  if (finalScore >= 40) return 'medium'
  return 'low'
}

/**
 * 根据性能级别生成推荐设置
 */
export function getRecommendedSettings(profile: DeviceProfile): Partial<DisplaySettings> {
  const { tier, isMobile } = profile

  const settings: Partial<DisplaySettings> = {
    // 三档基础设置
    mainMenuStarsEnabled: tier !== 'low',
    mainMenuMeteorsEnabled: tier === 'high',
    globalMouseTrailEnabled: tier !== 'low',
    clickAnimationEnabled: tier !== 'low',

    // 帧率设置
    meteorFps: tier === 'low' ? 15 : tier === 'high' ? 45 : 30,
    starsFps: tier === 'low' ? 15 : tier === 'high' ? 45 : 30,
  }

  // 移动设备特调：额外关闭鼠标拖尾
  if (isMobile) {
    settings.globalMouseTrailEnabled = false
  }

  return settings
}

/**
 * 从 localStorage 读取已保存的性能检测结果
 */
export function getSavedProfile(): DeviceProfile | null {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (!saved) return null

    const profile = JSON.parse(saved) as DeviceProfile

    // 验证数据完整性
    if (!profile.tier || !profile.cpuScore || !profile.gpuScore || !profile.detectedAt) {
      return null
    }

    return profile
  } catch {
    return null
  }
}

/**
 * 保存性能检测结果到 localStorage
 */
export function saveProfile(profile: DeviceProfile): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(profile))
  } catch (error) {
    console.warn('无法保存性能检测结果:', error)
  }
}

/**
 * 清除已保存的性能检测结果
 * 用于重新检测或手动重置
 */
export function clearSavedProfile(): void {
  try {
    localStorage.removeItem(STORAGE_KEY)
    console.log('[性能检测] 已清除检测结果')
  } catch (error) {
    console.warn('无法清除性能检测结果:', error)
  }
}

/**
 * 获取当前设备性能等级的描述信息
 */
export function getTierDescription(tier: DeviceProfile['tier'], isMobile: boolean): string {
  const descriptions = {
    high: '高性能设备',
    medium: '中等性能设备',
    low: '低性能设备'
  }

  let desc = descriptions[tier]

  if (isMobile) {
    desc += ' (移动端)'
  }

  return desc
}

/**
 * 执行完整的设备性能检测（约100-200ms）
 */
export async function detectDevicePerformance(): Promise<DeviceProfile> {
  // 硬件信息
  const cpuCores = navigator.hardwareConcurrency || 4
  const memory = (navigator as any).deviceMemory || 4
  const isMobile = isMobileDevice()

  // 基准测试
  const cpuScore = cpuBenchmark()
  const gpuScore = gpuBenchmark()

  // 综合评级
  const tier = determineTier(cpuScore, gpuScore, cpuCores, memory, isMobile)

  const profile: DeviceProfile = {
    tier,
    cpuScore,
    gpuScore,
    cpuCores,
    memory,
    isMobile,
    detectedAt: new Date().toISOString()
  }

  console.log('[性能检测] 完成:', {
    等级: tier,
    CPU耗时: `${cpuScore.toFixed(1)}ms`,
    GPU得分: gpuScore,
    CPU核心: cpuCores,
    内存: `${memory}GB`,
    移动设备: isMobile,
    备注: tier === 'high' ? '全开特效' : tier === 'medium' ? '中等特效' : '最低特效'
  })

  return profile
}

/**
 * 初始化性能检测并应用设置
 * 如果已检测过则跳过，不会重新调整设置
 *
 * @returns 返回检测结果（如果是首次检测）或已保存的结果
 */
export async function initializePerformanceDetection(): Promise<{
  profile: DeviceProfile
  isFirstDetection: boolean
}> {
  // 检查是否已检测过
  const savedProfile = getSavedProfile()

  if (savedProfile) {
    console.log('[性能检测] 使用已保存的结果:', savedProfile.tier)
    return {
      profile: savedProfile,
      isFirstDetection: false
    }
  }

  // 首次检测
  console.log('[性能检测] 开始检测...')
  const profile = await detectDevicePerformance()

  // 保存检测结果
  saveProfile(profile)

  return {
    profile,
    isFirstDetection: true
  }
}