import { onUnmounted, ref, type Ref } from 'vue'

/* ================== 视差倾斜 & 平移效果 ================== */
export interface ParallaxConfig {
  MAX_ANGLE: number
  VERTICAL_SCALE: number
  CHAR_MAX_SHIFT: number
  BG_MAX_SHIFT: number
  STARS_MAX_SHIFT: number
  DAMPING: number
  IDLE_THRESHOLD: number
  // 新增性能配置
  THROTTLE_DELAY: number
  USE_WILL_CHANGE: boolean
  USE_TRANSFORM_3D: boolean
}

export interface ParallaxElements {
  charRef: Ref<HTMLElement | null>
  bgRef: Ref<HTMLElement | null>
  starsLayerRef: Ref<HTMLElement | null>
}

/**
 * 默认视差配置
 */
const DEFAULT_PARALLAX_CONFIG: ParallaxConfig = {
  MAX_ANGLE: 2.5,
  VERTICAL_SCALE: 0.6,
  CHAR_MAX_SHIFT: 10,
  BG_MAX_SHIFT: 6,
  STARS_MAX_SHIFT: 20,
  DAMPING: 0.08,
  IDLE_THRESHOLD: 0.01,
  // 新增性能配置默认值
  THROTTLE_DELAY: 16, // ~60fps 节流延迟
  USE_WILL_CHANGE: true, // 启用 will-change 优化
  USE_TRANSFORM_3D: true, // 使用 3D transform 触发硬件加速
}

/**
 * 视差动画 Hook
 *
 * 用于创建鼠标移动时的视差效果，包括人物、背景、星星层的平移动画
 *
 */
export function useParallaxAnimation(
  elements: ParallaxElements,
  config: Partial<ParallaxConfig> = {},
) {
  // 合并默认配置
  const PARALLAX_CONFIG: ParallaxConfig = {
    ...DEFAULT_PARALLAX_CONFIG,
    ...config,
  }

  // 视差动画状态
  let targetOffsetX = 0
  let targetOffsetY = 0
  let currentOffsetX = 0
  let currentOffsetY = 0
  let parallaxRafId: number | null = null
  let isParallaxRunning = false
  let lastMouseMoveTime = 0
  let isPageVisible = true

  /**
   * 设置元素性能优化属性
   */
  function setupPerformanceOptimizations() {
    const elementsToOptimize = [
      elements.charRef.value,
      elements.bgRef.value,
      elements.starsLayerRef.value,
    ]

    elementsToOptimize.forEach((element) => {
      if (!element) return

      // 启用 will-change 优化
      if (PARALLAX_CONFIG.USE_WILL_CHANGE) {
        element.style.willChange = 'transform'
      }

      // 设置 3D transform 触发硬件加速
      if (PARALLAX_CONFIG.USE_TRANSFORM_3D) {
        element.style.transformStyle = 'preserve-3d'
        element.style.backfaceVisibility = 'hidden'
      }
    })
  }

  /**
   * 清理元素性能优化属性
   */
  function cleanupPerformanceOptimizations() {
    const elementsToOptimize = [
      elements.charRef.value,
      elements.bgRef.value,
      elements.starsLayerRef.value,
    ]

    elementsToOptimize.forEach((element) => {
      if (!element) return

      // 清理 will-change
      if (PARALLAX_CONFIG.USE_WILL_CHANGE) {
        element.style.willChange = 'auto'
      }
    })
  }

  /**
   * 高性能视差变换应用
   */
  function applyParallaxTransforms(offsetX: number, offsetY: number) {
    const charShift = -offsetX * PARALLAX_CONFIG.CHAR_MAX_SHIFT
    const bgShift = -offsetX * PARALLAX_CONFIG.BG_MAX_SHIFT
    const starsShift = -offsetX * PARALLAX_CONFIG.STARS_MAX_SHIFT

    // 使用 3D transform 触发硬件加速
    const transform3D = PARALLAX_CONFIG.USE_TRANSFORM_3D ? ' translateZ(0)' : ''

    if (elements.charRef.value) {
      elements.charRef.value.style.transform = `translate(-50%, -50%) translateX(${charShift}px)${transform3D}`
    }
    if (elements.bgRef.value) {
      elements.bgRef.value.style.transform = `translateX(${bgShift}px)${transform3D}`
    }
    if (elements.starsLayerRef.value) {
      elements.starsLayerRef.value.style.transform = `translateX(${starsShift}px)${transform3D}`
    }
  }

  /**
   * 高性能视差动画循环
   * - 只在有实际移动时启动
   * - 当值收敛到目标时自动停止
   * - 避免空闲时的不必要计算
   * - 页面不可见时暂停
   */
  function parallaxLoop() {
    // 页面可见性检查
    if (!isPageVisible) {
      isParallaxRunning = false
      parallaxRafId = null
      return
    }

    const deltaX = targetOffsetX - currentOffsetX
    const deltaY = targetOffsetY - currentOffsetY

    // 检查是否已经收敛到目标值
    const hasConverged =
      Math.abs(deltaX) < PARALLAX_CONFIG.IDLE_THRESHOLD &&
      Math.abs(deltaY) < PARALLAX_CONFIG.IDLE_THRESHOLD

    if (hasConverged) {
      // 动画已收敛到目标值，直接对齐并停止循环
      currentOffsetX = targetOffsetX
      currentOffsetY = targetOffsetY
      applyParallaxTransforms(currentOffsetX, currentOffsetY)
      isParallaxRunning = false
      parallaxRafId = null
      return
    }

    // 应用阻尼插值
    currentOffsetX += deltaX * PARALLAX_CONFIG.DAMPING
    currentOffsetY += deltaY * PARALLAX_CONFIG.DAMPING

    applyParallaxTransforms(currentOffsetX, currentOffsetY)

    parallaxRafId = requestAnimationFrame(parallaxLoop)
  }

  /**
   * 启动视差动画（如果尚未运行）
   */
  function startParallaxIfNeeded() {
    if (!isParallaxRunning && isPageVisible) {
      isParallaxRunning = true
      parallaxLoop()
    }
  }

  /**
   * 处理页面可见性变化
   */
  function handleVisibilityChange() {
    isPageVisible = !document.hidden
    if (!isPageVisible && parallaxRafId) {
      cancelAnimationFrame(parallaxRafId)
      parallaxRafId = null
      isParallaxRunning = false
    }
  }

  /**
   * 节流的鼠标移动处理
   */
  function handleMouseMove(e: MouseEvent) {
    // 节流处理
    const now = performance.now()
    if (now - lastMouseMoveTime < PARALLAX_CONFIG.THROTTLE_DELAY) {
      return
    }
    lastMouseMoveTime = now

    const centerX = window.innerWidth / 2
    const centerY = window.innerHeight / 2
    targetOffsetX = (e.clientX - centerX) / centerX
    targetOffsetY = (e.clientY - centerY) / centerY
    startParallaxIfNeeded()
  }

  function handleMouseLeave() {
    targetOffsetX = 0
    targetOffsetY = 0
    startParallaxIfNeeded()
  }

  // 初始化性能优化
  setupPerformanceOptimizations()

  // 监听页面可见性变化
  document.addEventListener('visibilitychange', handleVisibilityChange)

  // 组件卸载时清理
  onUnmounted(() => {
    if (parallaxRafId) {
      cancelAnimationFrame(parallaxRafId)
      parallaxRafId = null
    }
    isParallaxRunning = false

    // 清理性能优化
    cleanupPerformanceOptimizations()

    // 移除事件监听
    document.removeEventListener('visibilitychange', handleVisibilityChange)
  })

  return {
    handleMouseMove,
    handleMouseLeave,
  }
}

export type ParallaxAnimationReturn = ReturnType<typeof useParallaxAnimation>
