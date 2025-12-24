<template>
  <div class="cursor-effects-container">
    <!-- Canvas 拖尾轨迹 -->
    <canvas ref="canvasRef" class="cursor-trail-canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue'

interface TrailPoint {
  x: number
  y: number
  alpha: number
}

interface Particle {
  x: number
  y: number
  vx: number
  vy: number
  life: number
  maxLife: number
  size: number
  color: string
  rotation: number
  rotationSpeed: number
}

// --- Canvas 引用 ---
const canvasRef = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null

// --- 拖尾效果状态 ---
const points: TrailPoint[] = []
let maxPoints = 36 // 动态调整
const fadeSpeed = 0.025
let animationId: number | null = null
let isAnimating = false
let lastMouseTime = 0
const MOUSE_THROTTLE = 16 // ~60fps

// --- 粒子系统 ---
const particles: Particle[] = []

// --- 性能监控 ---
let frameCount = 0
let lastFpsTime = 0
let currentFps = 60

// --- 帧率限制 ---
const TARGET_FPS = 60 // 目标帧率
const FRAME_INTERVAL = 1000 / TARGET_FPS // 帧间隔时间（毫秒）
let lastFrameTime = 0 // 上一帧的时间

// --- 初始化 Canvas ---
const initCanvas = () => {
  const canvas = canvasRef.value
  if (!canvas) return

  const dpr = window.devicePixelRatio || 1
  canvas.width = window.innerWidth * dpr
  canvas.height = window.innerHeight * dpr
  canvas.style.width = `${window.innerWidth}px`
  canvas.style.height = `${window.innerHeight}px`

  ctx = canvas.getContext('2d')
  if (ctx) {
    ctx.scale(dpr, dpr)
  }
}

// --- 性能监控 ---
const monitorPerformance = () => {
  frameCount++
  const now = performance.now()
  if (now - lastFpsTime >= 1000) {
    currentFps = frameCount
    frameCount = 0
    lastFpsTime = now

    // 自动降级效果
    if (currentFps < 30) {
      maxPoints = Math.max(5, maxPoints - 1)
    } else if (currentFps > 50 && maxPoints < 50) {
      maxPoints = Math.min(50, maxPoints + 1)
    }
  }
}

// --- 绘制拖尾轨迹 ---
const drawTrail = () => {
  if (!ctx || points.length < 2) return

  ctx.lineWidth = 3
  ctx.lineCap = 'butt' // 使用 butt 消除连接处的重叠变黑问题
  ctx.lineJoin = 'round'
  ctx.shadowBlur = 10
  ctx.shadowColor = '#87CEFA'

  let startX = points[0]?.x || 0
  let startY = points[0]?.y || 0
  let startAlpha = points[0]?.alpha || 0

  // 分段绘制：使用线性渐变填充每一段，确保颜色过渡平滑
  for (let i = 1; i < points.length - 1; i++) {
    const p = points[i]
    const nextP = points[i + 1]
    if (!p || !nextP) continue
    const xc = (p.x + nextP.x) * 0.5
    const yc = (p.y + nextP.y) * 0.5
    const endAlpha = (p.alpha + nextP.alpha) * 0.5

    ctx.beginPath()
    ctx.moveTo(startX, startY)
    ctx.quadraticCurveTo(p.x, p.y, xc, yc)

    // 创建每一段的渐变
    const gradient = ctx.createLinearGradient(startX, startY, xc, yc)
    gradient.addColorStop(0, `rgba(135, 206, 250, ${startAlpha})`)
    gradient.addColorStop(1, `rgba(135, 206, 250, ${endAlpha})`)
    ctx.strokeStyle = gradient
    ctx.stroke()

    startX = xc
    startY = yc
    startAlpha = endAlpha
  }

  // 连接到最后一个点
  if (points.length > 1) {
    const last = points[points.length - 1]
    if (!last) return
    ctx.beginPath()
    ctx.moveTo(startX, startY)
    ctx.lineTo(last.x, last.y)

    const gradient = ctx.createLinearGradient(startX, startY, last.x, last.y)
    gradient.addColorStop(0, `rgba(135, 206, 250, ${startAlpha})`)
    gradient.addColorStop(1, `rgba(135, 206, 250, ${last.alpha})`)
    ctx.strokeStyle = gradient
    ctx.stroke()

    // 绘制头部圆帽（因为使用了butt，需要手动补圆）
    ctx.beginPath()
    ctx.arc(last.x, last.y, 1.5, 0, Math.PI * 2)
    ctx.fillStyle = `rgba(135, 206, 250, ${last.alpha})`
    ctx.fill()
  }
}

// --- 绘制粒子 ---
const drawParticles = () => {
  if (!ctx) return

  for (let i = particles.length - 1; i >= 0; i--) {
    const p = particles[i]
    if (!p) continue

    // 更新粒子
    p.x += p.vx
    p.y += p.vy
    p.vy += 0.05 // 轻微重力
    p.life = p.life - 1 / (60 * p.maxLife) < 0 ? 0 : p.life - 1 / (60 * p.maxLife) // 60fps衰减
    p.rotation += p.rotationSpeed

    // 绘制粒子
    const alpha = p.life
    ctx.save()
    ctx.translate(p.x, p.y)
    ctx.rotate((p.rotation * Math.PI) / 180)
    ctx.globalAlpha = alpha

    // 绘制三角形
    ctx.beginPath()
    ctx.moveTo(0, -p.size)
    ctx.lineTo(-p.size * 0.7, p.size * 0.7)
    ctx.lineTo(p.size * 0.7, p.size * 0.7)
    ctx.closePath()
    ctx.fillStyle = p.color
    ctx.fill()

    ctx.restore()

    // 移除死亡粒子
    if (p.life <= 0) {
      particles.splice(i, 1)
    }
  }
}

// --- 更新点透明度 ---
const updatePoints = () => {
  // 批量更新透明度
  for (let i = points.length - 1; i >= 0; i--) {
    const p = points[i]
    if (!p) continue
    p.alpha -= fadeSpeed
    if (p.alpha <= 0) {
      points.splice(i, 1)
    }
  }

  // 限制最大点数
  if (points.length > maxPoints) {
    points.splice(0, points.length - maxPoints)
  }
}

// --- 主绘制循环 ---
const draw = (timestamp: number) => {
  // 帧率限制：只有当距离上一帧的时间超过设定的帧间隔时才执行绘制
  if (timestamp - lastFrameTime < FRAME_INTERVAL) {
    animationId = requestAnimationFrame(draw)
    return
  }

  lastFrameTime = timestamp
  monitorPerformance()

  if (!ctx || !canvasRef.value) {
    stopAnimation()
    return
  }

  const canvas = canvasRef.value
  const dpr = window.devicePixelRatio || 1

  // 清除画布
  ctx.clearRect(0, 0, canvas.width / dpr, canvas.height / dpr)

  let shouldContinue = false

  // 绘制拖尾
  if (points.length >= 2) {
    drawTrail()
    shouldContinue = shouldContinue || points.length > 0
  }

  // 绘制粒子
  if (particles.length > 0) {
    drawParticles()
    shouldContinue = shouldContinue || particles.length > 0
  }

  // 更新点状态
  updatePoints()

  // 条件性继续动画
  if (shouldContinue) {
    animationId = requestAnimationFrame(draw)
  } else {
    stopAnimation()
  }
}

// --- 停止动画 ---
const stopAnimation = () => {
  if (animationId) {
    cancelAnimationFrame(animationId)
    animationId = null
  }
  isAnimating = false
}

// --- 节流的鼠标移动处理 ---
const handleMouseMove = (e: MouseEvent) => {
  const now = Date.now()
  if (now - lastMouseTime < MOUSE_THROTTLE) {
    return
  }
  lastMouseTime = now

  points.push({
    x: e.clientX,
    y: e.clientY,
    alpha: 1.0,
  })

  // 启动动画循环（如果未运行）
  if (!isAnimating && animationId === null) {
    isAnimating = true
    lastFrameTime = performance.now() // 初始化帧时间
    animationId = requestAnimationFrame(draw)
  }

  // 限制点数
  if (points.length > maxPoints * 1.5) {
    points.splice(0, points.length - maxPoints)
  }
}

// --- 优化的点击效果（使用Canvas替代DOM）---
const handleClick = (e: MouseEvent) => {
  const particleCount = 12
  const colors = ['#FFC0CB', '#87CEFA']

  for (let i = 0; i < particleCount; i++) {
    const angle = Math.random() * Math.PI * 2
    const speed = Math.random() * 2 + 1
    const life = Math.random() * 0.8 + 0.7

    particles.push({
      x: e.clientX,
      y: e.clientY,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      life: 1.0,
      maxLife: life,
      size: Math.random() * 8 + 4,
      color: colors[Math.floor(Math.random() * colors.length)] || '#FFC0CB',
      rotation: Math.random() * 360,
      rotationSpeed: (Math.random() - 0.5) * 8,
    })
  }

  // 确保动画运行
  if (!isAnimating && animationId === null) {
    isAnimating = true
    lastFrameTime = performance.now() // 初始化帧时间
    animationId = requestAnimationFrame(draw)
  }
}

// --- 窗口大小变化处理 ---
const handleResize = () => {
  initCanvas()
}

// --- 生命周期钩子 ---
onMounted(() => {
  initCanvas()
  window.addEventListener('mousemove', handleMouseMove)
  window.addEventListener('click', handleClick)
  window.addEventListener('resize', handleResize)
})

onBeforeUnmount(() => {
  window.removeEventListener('mousemove', handleMouseMove)
  window.removeEventListener('click', handleClick)
  window.removeEventListener('resize', handleResize)
  stopAnimation()
  points.length = 0
  particles.length = 0
})
</script>

<style scoped>
/* Canvas 拖尾样式 */
.cursor-trail-canvas {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 9999;
}
</style>

<style>
/* 点击三角粒子样式（全局样式，因为是动态创建的元素） */
.click-triangle-particle {
  position: fixed;
  pointer-events: none;
  z-index: 10000;
  width: 0;
  height: 0;
  border-left: var(--triangle-size) solid transparent;
  border-right: var(--triangle-size) solid transparent;
  border-bottom: calc(var(--triangle-size) * 1.5) solid var(--triangle-color);
  animation: click-burst-animation 1s forwards;
}

@keyframes click-burst-animation {
  from {
    transform: translate(-50%, -50%) rotate(var(--initial-rotation)) scale(1);
    opacity: inherit;
  }
  to {
    transform: translate(calc(-50% + var(--translate-x)), calc(-50% + var(--translate-y)))
      rotate(var(--final-rotation)) scale(0);
    opacity: 0;
  }
}
</style>
