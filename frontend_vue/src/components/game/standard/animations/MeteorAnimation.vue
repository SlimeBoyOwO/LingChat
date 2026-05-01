<template>
  <div v-if="meteorsEnabled" class="meteor-wrapper">
    <canvas ref="canvasRef" id="meteor-canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch, computed } from 'vue'

interface MeteorTemplate {
  startX: number
  startY: number
  controlX: number
  controlY: number
  endX: number
  endY: number
  duration: number
}

interface ActiveMeteor {
  id: number
  template: MeteorTemplate
  startTime: number
  duration: number
}

const props = defineProps<{
  meteorsEnabled: boolean
  meteorFps?: number
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null
let animationFrameId: number | null = null

const activeMeteors = ref<ActiveMeteor[]>([])
let meteorIdCounter = 0
let meteorIntervalId: ReturnType<typeof setInterval> | null = null

// 预缓存的流星模板
const METEOR_CACHE_SIZE = 10
let meteorTemplates: MeteorTemplate[] = []
let lastUsedTemplateIndex = -1

const METEOR_CONFIG = {
  MAX_COUNT: 3,
  GENERATE_INTERVAL: 1000,
  DURATION_MIN: 9,
  DURATION_MAX: 14,
  START_X_MIN: 800,
  START_X_MAX: 1200,
  START_Y_RANGE: 0.25,
  END_X_MIN: -200,
  END_X_MAX: -400,
  END_Y_OFFSET: 1,
  CONTROL_X_RATIO_MIN: 0.5,
  CONTROL_X_RATIO_MAX: 0.9,
  CONTROL_Y_OFFSET: -0.1,
  // 流星尾巴配置
  TAIL_LENGTH: 350, // 尾巴像素长度
  HEAD_SIZE: 4,
}

// 帧率限制
const TARGET_FPS = ref(30)
const FRAME_INTERVAL = computed(() => 1000 / TARGET_FPS.value)

// 上一帧时间戳 - 用于帧率控制
let lastFrameTime = 0

/**
 * 计算二次贝塞尔曲线上的点
 * B(t) = (1-t)²·P0 + 2(1-t)·t·P1 + t²·P2
 */
function getBezierPoint(
  t: number,
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
): { x: number; y: number } {
  const mt = 1 - t
  const mt2 = mt * mt
  const t2 = t * t
  return {
    x: mt2 * p0.x + 2 * mt * t * p1.x + t2 * p2.x,
    y: mt2 * p0.y + 2 * mt * t * p1.y + t2 * p2.y,
  }
}

/**
 * 预生成流星模板缓存
 */
function initMeteorTemplateCache() {
  meteorTemplates = []

  const w = window.innerWidth
  const h = window.innerHeight

  for (let i = 0; i < METEOR_CACHE_SIZE; i++) {
    const startY = -Math.random() * (h * METEOR_CONFIG.START_Y_RANGE)
    const startX =
      w +
      METEOR_CONFIG.START_X_MIN +
      Math.random() * (METEOR_CONFIG.START_X_MAX - METEOR_CONFIG.START_X_MIN)
    const endX =
      METEOR_CONFIG.END_X_MIN + Math.random() * (METEOR_CONFIG.END_X_MAX - METEOR_CONFIG.END_X_MIN)
    const endY = startY + h * METEOR_CONFIG.END_Y_OFFSET
    const controlX =
      w *
      (METEOR_CONFIG.CONTROL_X_RATIO_MIN +
        Math.random() * (METEOR_CONFIG.CONTROL_X_RATIO_MAX - METEOR_CONFIG.CONTROL_X_RATIO_MIN))
    const controlY = startY + h * METEOR_CONFIG.CONTROL_Y_OFFSET
    const duration =
      METEOR_CONFIG.DURATION_MIN +
      Math.random() * (METEOR_CONFIG.DURATION_MAX - METEOR_CONFIG.DURATION_MIN)

    meteorTemplates.push({
      startX,
      startY,
      controlX,
      controlY,
      endX,
      endY,
      duration,
    })
  }
}

function getMeteorTemplate(): MeteorTemplate {
  let index = Math.floor(Math.random() * METEOR_CACHE_SIZE)
  while (index === lastUsedTemplateIndex && METEOR_CACHE_SIZE > 1) {
    index = Math.floor(Math.random() * METEOR_CACHE_SIZE)
  }
  lastUsedTemplateIndex = index
  return meteorTemplates[index]!
}

function createMeteor() {
  const template = getMeteorTemplate()
  createMeteorFromTemplate(template)
}

function createMeteorFromTemplate(template: MeteorTemplate) {
  const id = meteorIdCounter++
  const startTime = performance.now()

  activeMeteors.value.push({
    id,
    template,
    startTime,
    duration: template.duration * 1000, // 转换为毫秒
  })
}

function updateMeteorShower() {
  if (activeMeteors.value.length < METEOR_CONFIG.MAX_COUNT) createMeteor()
}

function getOpacity(progress: number): number {
  return Math.max(0, Math.min(1, 1 - progress))
}

/**
 * 绘制单个流星
 */
function drawMeteor(meteor: ActiveMeteor, currentTime: number) {
  if (!ctx || !canvasRef.value) return

  const { template, startTime, duration } = meteor

  // 计算当前动画进度 (0 到 1)
  const elapsed = currentTime - startTime
  const progress = elapsed / duration

  // 超过持续时间则移除
  if (progress >= 1) {
    activeMeteors.value = activeMeteors.value.filter((m) => m.id !== meteor.id)
    return
  }

  const p0 = { x: template.startX, y: template.startY }
  const p1 = { x: template.controlX, y: template.controlY }
  const p2 = { x: template.endX, y: template.endY }

  // 计算流星总长度（近似）
  const approxLength = Math.sqrt(
    Math.pow(template.endX - template.startX, 2) + Math.pow(template.endY - template.startY, 2),
  )

  // 尾巴长度占曲线的比例
  const tailRatio = METEOR_CONFIG.TAIL_LENGTH / approxLength

  // 当前头部的位置参数
  const headT = progress

  // 尾巴起点的位置参数（向后偏移tailRatio）
  // 由于我们希望尾巴长度固定，需要根据弧长计算
  const tailT = Math.max(0, headT - tailRatio * 1.5) // 简化计算，经验系数

  // 获取头部和尾巴位置
  const headPos = getBezierPoint(headT, p0, p1, p2)
  const tailStartPos = getBezierPoint(tailT, p0, p1, p2)

  // 绘制尾巴（渐变线条）
  const gradient = ctx.createLinearGradient(tailStartPos.x, tailStartPos.y, headPos.x, headPos.y)
  gradient.addColorStop(0, 'rgba(180, 220, 255, 0)')
  gradient.addColorStop(0.5, 'rgba(220, 240, 255, 0.6)')
  gradient.addColorStop(1, 'rgba(255, 255, 255, 1)')

  ctx.beginPath()
  ctx.strokeStyle = gradient
  ctx.lineWidth = METEOR_CONFIG.HEAD_SIZE
  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'

  // 绘制尾巴路径：沿贝塞尔曲线从tailT到headT
  // 为了绘制曲线尾巴，需要在曲线上采样多个点
  const steps = 30
  for (let i = 0; i <= steps; i++) {
    const t = tailT + (headT - tailT) * (i / steps)
    const point = getBezierPoint(t, p0, p1, p2)
    if (i === 0) {
      ctx.moveTo(point.x, point.y)
    } else {
      ctx.lineTo(point.x, point.y)
    }
  }
  ctx.stroke()

  // 绘制拖尾光晕
  ctx.save()
  ctx.filter = 'blur(4px)'
  ctx.beginPath()
  ctx.strokeStyle = `rgba(255, 255, 255, ${getOpacity(progress)})`
  ctx.lineWidth = METEOR_CONFIG.HEAD_SIZE * 1
  ctx.lineCap = 'round'

  for (let i = 0; i <= steps; i++) {
    const t = tailT + (headT - tailT) * (i / steps)
    const point = getBezierPoint(t, p0, p1, p2)
    if (i === 0) {
      ctx.moveTo(point.x, point.y)
    } else {
      ctx.lineTo(point.x, point.y)
    }
  }
  ctx.stroke()
  ctx.restore()
}

/**
 * 动画循环
 */
function animate(currentTime: number) {
  if (!ctx || !canvasRef.value) {
    animationFrameId = requestAnimationFrame(animate)
    return
  }

  // 帧率限制检查
  if (currentTime - lastFrameTime < FRAME_INTERVAL.value) {
    animationFrameId = requestAnimationFrame(animate)
    return
  }
  lastFrameTime = currentTime

  const canvas = canvasRef.value

  // 清空画布
  ctx.clearRect(0, 0, canvas.width, canvas.height)

  // 绘制所有活跃的流星
  for (const meteor of activeMeteors.value) {
    drawMeteor(meteor, currentTime)
  }

  animationFrameId = requestAnimationFrame(animate)
}

function resizeCanvas() {
  if (!canvasRef.value) return

  const canvas = canvasRef.value
  const dpr = window.devicePixelRatio || 1

  canvas.width = window.innerWidth * dpr
  canvas.height = window.innerHeight * dpr

  canvas.style.width = window.innerWidth + 'px'
  canvas.style.height = window.innerHeight + 'px'

  if (ctx) {
    ctx.scale(dpr, dpr)
  }

  // 重新初始化模板缓存以适应新尺寸
  initMeteorTemplateCache()
}

function startMeteorShower() {
  if (meteorIntervalId) clearInterval(meteorIntervalId)

  // 初始化模板缓存
  if (meteorTemplates.length === 0) {
    initMeteorTemplateCache()
  }

  // 立即创建初始流星
  for (let i = 0; i < METEOR_CONFIG.MAX_COUNT; i++) {
    createMeteor()
  }

  // 启动动画循环
  if (!animationFrameId) {
    animationFrameId = requestAnimationFrame(animate)
  }

  meteorIntervalId = setInterval(updateMeteorShower, METEOR_CONFIG.GENERATE_INTERVAL)
}

function stopMeteorShower() {
  if (meteorIntervalId) {
    clearInterval(meteorIntervalId)
    meteorIntervalId = null
  }
  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId)
    animationFrameId = null
  }
  activeMeteors.value = []
}

function handleVisibilityChange() {
  if (document.hidden) {
    if (meteorIntervalId) {
      clearInterval(meteorIntervalId)
      meteorIntervalId = null
    }
    if (animationFrameId) {
      cancelAnimationFrame(animationFrameId)
      animationFrameId = null
    }
  } else if (props.meteorsEnabled) {
    if (!animationFrameId) {
      animationFrameId = requestAnimationFrame(animate)
    }
    if (!meteorIntervalId) {
      meteorIntervalId = setInterval(updateMeteorShower, METEOR_CONFIG.GENERATE_INTERVAL)
    }
  }
}

onMounted(() => {
  if (canvasRef.value) {
    const context = canvasRef.value.getContext('2d')
    if (!context) {
      console.error('Failed to get 2D context from canvas')
      return
    }
    ctx = context
    resizeCanvas()
  }

  document.addEventListener('visibilitychange', handleVisibilityChange)
  window.addEventListener('resize', resizeCanvas)

  // 监听meteorFps prop变化，动态更新帧率
  watch(
    () => props.meteorFps,
    (newFps) => {
      if (newFps && newFps >= 10 && newFps <= 60) {
        TARGET_FPS.value = newFps
      }
    },
    { immediate: true },
  )

  watch(
    () => props.meteorsEnabled,
    (enabled) => {
      if (enabled) {
        startMeteorShower()
      } else {
        stopMeteorShower()
      }
    },
    { immediate: true },
  )
})

onUnmounted(() => {
  document.removeEventListener('visibilitychange', handleVisibilityChange)
  window.removeEventListener('resize', resizeCanvas)
  stopMeteorShower()
})
</script>

<style scoped>
.meteor-wrapper {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 2;
  pointer-events: none;
  overflow: hidden;
}

#meteor-canvas {
  width: 100%;
  height: 100%;
  /* GPU 加速 */
  transform: translateZ(0);
  will-change: transform;
}
</style>
