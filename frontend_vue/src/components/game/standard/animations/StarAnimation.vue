<template>
  <!-- 星星粒子层（位于背景和人物之间） -->
  <div v-if="starsEnabled" class="stars-layer" ref="starsLayerRef">
    <canvas id="stars-canvas" ref="canvasRef"></canvas>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'

const props = defineProps<{
  starsEnabled: boolean
  starsLayerRef: HTMLElement | null
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

interface Star {
  x: number
  y: number
  size: number
  baseOpacity: number
  rotation: number
  type: 'star' | 'circle'
}

const STARS_COUNT = 80
const FLICKER_SPEED = 0.003
const starsPositions = shallowRef<Star[]>([])
let starsFrameId: number | null = null
let starsCtx: CanvasRenderingContext2D | null = null

// 缓存预渲染的星星图像
let starImageCache: Map<number, HTMLCanvasElement> | null = null
let circleImageCache: Map<number, HTMLCanvasElement> | null = null

/**
 * 创建带发光效果的星星形状到离屏 canvas
 * 预渲染一次，后续使用 drawImage 快速绘制
 */
function createStarImage(size: number): HTMLCanvasElement {
  const padding = size * 2 // 为发光效果预留空间
  const canvas = document.createElement('canvas')
  const actualSize = size * 1.5 + padding * 2
  canvas.width = actualSize
  canvas.height = actualSize
  const ctx = canvas.getContext('2d')!
  const centerX = actualSize / 2
  const centerY = actualSize / 2

  // 发光效果
  ctx.shadowColor = 'rgba(255, 255, 200, 0.8)'
  ctx.shadowBlur = 20

  // 渐变填充
  const gradient = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, size * 1.5)
  gradient.addColorStop(0, '#ffffff')
  gradient.addColorStop(0.6, 'rgba(255, 240, 200, 0.9)')
  gradient.addColorStop(1, 'rgba(255, 200, 100, 0)')
  ctx.fillStyle = gradient

  // 绘制星形路径
  ctx.beginPath()
  for (let i = 0; i < 8; i++) {
    const angle = (i * Math.PI) / 4
    const radius = i % 2 === 0 ? size : size * 0.4
    const dx = centerX + Math.cos(angle) * radius
    const dy = centerY + Math.sin(angle) * radius
    if (i === 0) ctx.moveTo(dx, dy)
    else ctx.lineTo(dx, dy)
  }
  ctx.closePath()
  ctx.fill()

  return canvas
}

/**
 * 创建带发光效果的圆形到离屏 canvas
 */
function createCircleImage(size: number): HTMLCanvasElement {
  const padding = size * 2
  const canvas = document.createElement('canvas')
  const actualSize = size * 1.5 + padding * 2
  canvas.width = actualSize
  canvas.height = actualSize
  const ctx = canvas.getContext('2d')!
  const centerX = actualSize / 2
  const centerY = actualSize / 2

  // 发光效果
  ctx.shadowColor = 'rgba(255, 255, 200, 0.8)'
  ctx.shadowBlur = 20

  // 渐变填充
  const gradient = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, size * 1.5)
  gradient.addColorStop(0, '#ffffff')
  gradient.addColorStop(0.6, 'rgba(255, 240, 200, 0.9)')
  gradient.addColorStop(1, 'rgba(255, 200, 100, 0)')
  ctx.fillStyle = gradient

  ctx.beginPath()
  ctx.arc(centerX, centerY, size, 0, Math.PI * 2)
  ctx.fill()

  return canvas
}

/**
 * 获取或创建缓存的星星图像
 * 使用 Map 缓存不同尺寸的预渲染图像
 */
function getStarImage(size: number): HTMLCanvasElement {
  if (!starImageCache) {
    starImageCache = new Map()
  }
  const roundedSize = Math.round(size)
  let image = starImageCache.get(roundedSize)
  if (!image) {
    image = createStarImage(roundedSize)
    starImageCache.set(roundedSize, image)
  }
  return image
}

function getCircleImage(size: number): HTMLCanvasElement {
  if (!circleImageCache) {
    circleImageCache = new Map()
  }
  const roundedSize = Math.round(size)
  let image = circleImageCache.get(roundedSize)
  if (!image) {
    image = createCircleImage(roundedSize)
    circleImageCache.set(roundedSize, image)
  }
  return image
}

/**
 * 清理图像缓存
 */
function clearImageCaches() {
  starImageCache?.clear()
  circleImageCache?.clear()
  starImageCache = null
  circleImageCache = null
}

function generateStars() {
  if (!canvasRef.value) return
  const w = window.innerWidth
  const h = window.innerHeight
  canvasRef.value.width = w
  canvasRef.value.height = h
  starsCtx = canvasRef.value.getContext('2d')
  if (!starsCtx) return

  // 预生成所有可能用到的尺寸的图像缓存
  // 星星尺寸范围是 2-7 (Math.random() * 5 + 2)
  // 预生成常用尺寸以减少运行时开销
  for (let size = 2; size <= 7; size++) {
    getStarImage(size)
    getCircleImage(size)
  }

  const tempStars: Star[] = []
  for (let i = 0; i < STARS_COUNT; i++) {
    tempStars.push({
      x: Math.random() * w,
      y: Math.random() * (h * 0.33),
      size: Math.random() * 5 + 2,
      baseOpacity: Math.random() * 0.5 + 0.5,
      rotation: Math.random() * Math.PI * 2,
      type: Math.random() > 0.2 ? 'star' : 'circle',
    })
  }
  starsPositions.value = tempStars
}

/**
 * 优化版渲染函数
 * - 使用预缓存的离屏 canvas 图像
 * - 避免每帧创建渐变和设置阴影
 * - 缓存 stars 数组避免响应式开销
 */
function renderStars() {
  if (!starsCtx || !canvasRef.value) return
  const w = canvasRef.value.width
  const h = canvasRef.value.height

  // 关键修复：重置全局透明度再 clearRect，避免清除不干净导致的问题
  starsCtx.globalAlpha = 1.0
  starsCtx.clearRect(0, 0, w, h)

  const now = Date.now()

  const stars = starsPositions.value

  for (let i = 0; i < stars.length; i++) {
    const pos = stars[i]
    if (!pos) continue

    const flicker = 0.6 + 0.4 * Math.sin(now * FLICKER_SPEED + pos.x)
    const opacity = Math.min(pos.baseOpacity * flicker, 1.0)
    starsCtx!.globalAlpha = opacity

    // 获取预缓存的图像
    const image = pos.type === 'star' ? getStarImage(pos.size) : getCircleImage(pos.size)
    const imgSize = image.width
    const halfSize = imgSize / 2

    // 使用 drawImage 快速绘制，避免每帧创建渐变和路径
    if (pos.type === 'star') {
      // 星星需要旋转
      starsCtx.save()
      starsCtx.translate(pos.x, pos.y)
      starsCtx.rotate(pos.rotation)
      starsCtx.drawImage(image, -halfSize, -halfSize)
      starsCtx.restore()
    } else {
      // 圆形无需旋转，直接绘制
      starsCtx.drawImage(image, pos.x - halfSize, pos.y - halfSize)
    }
  }
}

function flickerAnimation() {
  renderStars()
  starsFrameId = requestAnimationFrame(flickerAnimation)
}

function handleResize() {
  stopStars()
  startStars()
}

function startStars() {
  if (!canvasRef.value) return
  generateStars()
  flickerAnimation()
  window.removeEventListener('resize', handleResize)
  window.addEventListener('resize', handleResize)
}

function stopStars() {
  window.removeEventListener('resize', handleResize)
  if (starsFrameId) {
    cancelAnimationFrame(starsFrameId)
    starsFrameId = null
  }
  if (starsCtx && canvasRef.value) {
    starsCtx.clearRect(0, 0, canvasRef.value.width, canvasRef.value.height)
  }
  // 清理图像缓存
  clearImageCaches()
}

onMounted(() => {
  watch(
    () => props.starsEnabled,
    async (enabled) => {
      if (enabled) {
        await nextTick()
        startStars()
      } else {
        stopStars()
      }
    },
    { immediate: true },
  )
})

onUnmounted(() => {
  stopStars()
})
</script>

<style scoped>
.stars-layer {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 1;
  pointer-events: none;
  overflow: hidden;
  /* 移除 transition */
  will-change: transform;
}

#stars-canvas {
  width: 100%;
  height: 100%;
  display: block;
}
</style>
