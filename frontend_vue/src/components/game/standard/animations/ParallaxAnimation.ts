import { onUnmounted, ref, type Ref } from 'vue'

/* ================== 视差倾斜 & 平移效果 (优化版) ================== */
export interface ParallaxConfig {
  MAX_ANGLE: number
  VERTICAL_SCALE: number
  CHAR_MAX_SHIFT: number
  BG_MAX_SHIFT: number
  STARS_MAX_SHIFT: number
  DAMPING: number
  IDLE_THRESHOLD: number
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
}

/**
 * 视差动画 Hook
 *
 * 用于创建鼠标移动时的视差效果，包括人物、背景、星星层的平移动画
 *
 * @param elements - 需要应用视差效果的元素引用
 * @param config - 可选的自定义配置
 * @returns 返回鼠标移动和鼠标离开的事件处理函数
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

  /**
   * 应用视差变换到各层元素
   */
  function applyParallaxTransforms(offsetX: number, offsetY: number) {
    const charShift = -offsetX * PARALLAX_CONFIG.CHAR_MAX_SHIFT
    const bgShift = -offsetX * PARALLAX_CONFIG.BG_MAX_SHIFT
    const starsShift = -offsetX * PARALLAX_CONFIG.STARS_MAX_SHIFT

    if (elements.charRef.value) {
      elements.charRef.value.style.transform = `translate(-50%, -50%) translateX(${charShift}px)`
    }
    if (elements.bgRef.value) {
      elements.bgRef.value.style.transform = `translateX(${bgShift}px)`
    }
    if (elements.starsLayerRef.value) {
      elements.starsLayerRef.value.style.transform = `translateX(${starsShift}px)`
    }
  }

  /**
   * 优化的视差动画循环
   * - 只在有实际移动时启动
   * - 当值收敛到目标时自动停止
   * - 避免空闲时的不必要计算
   */
  function parallaxLoop() {
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
    if (!isParallaxRunning) {
      isParallaxRunning = true
      parallaxLoop()
    }
  }

  function handleMouseMove(e: MouseEvent) {
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

  // 组件卸载时清理
  onUnmounted(() => {
    if (parallaxRafId) {
      cancelAnimationFrame(parallaxRafId)
      parallaxRafId = null
    }
    isParallaxRunning = false
  })

  return {
    handleMouseMove,
    handleMouseLeave,
  }
}

export type ParallaxAnimationReturn = ReturnType<typeof useParallaxAnimation>
