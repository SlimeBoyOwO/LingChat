<template>
  <div ref="canvasContainer" class="fireworks-container">
    <canvas ref="fireworksCanvas" class="fireworks-canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'

const props = defineProps<{
  enabled: boolean
  intensity?: number
}>()

const canvasContainer = ref<HTMLElement>()
const fireworksCanvas = ref<HTMLCanvasElement>()
let animationFrameId: number
let particles: Particle[] = []
let lastTime = 0
let resizeObserver: ResizeObserver

interface Firework {
  x: number
  y: number
  vx: number
  vy: number
  life: number
  maxLife: number
  color: string
  size: number
  gravity: number
  rotation: number
  rotationSpeed: number
  exploded: boolean
}

interface Particle {
  x: number
  y: number
  vx: number
  vy: number
  life: number
  maxLife: number
  color: string
  size: number
  gravity: number
  rotation: number
  rotationSpeed: number
}

const colors = ['#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff', '#00ffff', '#ffffff']
const intensity = ref(props.intensity || 1)
let fireworks: Firework[] = []

// 创建烟花
function createFirework(): Firework {
  if (!canvasContainer.value) throw new Error('Canvas container not available')
  
  const x = Math.random() * canvasContainer.value.clientWidth

  return {
    x,
    y: canvasContainer.value.clientHeight,
    vx: (Math.random() - 0.5) * 2,
    vy: -Math.random() * 8 - 8, // 向上飞行
    life: 1.0,
    maxLife: Math.random() * 40 + 60,
    color: colors[Math.floor(Math.random() * colors.length)]!,
    size: 3,
    gravity: 0.1,
    rotation: 0,
    rotationSpeed: 0,
    exploded: false,
  }
}

// 创建爆炸粒子
function createExplosion(x: number, y: number, color: string, count: number = 50) {
  for (let i = 0; i < count; i++) {
    const angle = Math.random() * Math.PI * 2
    const speed = Math.random() * 5 + 2
    
    particles.push({
      x,
      y,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      life: 1.0,
      maxLife: Math.random() * 60 + 40,
      color,
      size: Math.random() * 3 + 1,
      gravity: Math.random() * 0.2 + 0.05,
      rotation: Math.random() * Math.PI * 2,
      rotationSpeed: (Math.random() - 0.5) * 0.2
    })
  }
}

// 更新粒子
function updateParticles() {
  for (let i = particles.length - 1; i >= 0; i--) {
    const p = particles[i]
    if (!p) continue
    
    // 更新位置
    p.x += p.vx
    p.y += p.vy
    p.vy += p.gravity
    p.rotation += p.rotationSpeed
    
    // 更新生命值
    p.life -= 1 / p.maxLife
    
    // 移除死亡粒子
    if (p.life <= 0) {
      particles.splice(i, 1)
    }
  }
}

// 更新烟花
function updateFireworks() {
  for (let i = fireworks.length - 1; i >= 0; i--) {
    const f = fireworks[i]
    if (!f) continue
    
    // 更新位置
    f.x += f.vx
    f.y += f.vy
    f.vy += f.gravity
    
    // 检查是否到达爆炸高度
    const shouldExplode = f.y <= (canvasContainer.value?.clientHeight || 0) * (Math.random() /2) || f.vy >= 0
    
    if (shouldExplode && !f.exploded) {
      f.exploded = true
      createExplosion(f.x, f.y, f.color, 60)
    }
    
    // 移除已爆炸的烟花
    if (f.exploded) {
      fireworks.splice(i, 1)
    }
  }
}

// 绘制粒子
function drawParticles(ctx: CanvasRenderingContext2D, width: number, height: number) {
  ctx.clearRect(0, 0, width, height)
  
  // 绘制烟花
  for (const f of fireworks) {
    ctx.save()
    ctx.globalAlpha = 0.8
    ctx.fillStyle = f.color
    ctx.shadowBlur = 10
    ctx.shadowColor = f.color
    ctx.beginPath()
    ctx.arc(f.x, f.y, f.size, 0, Math.PI * 2)
    ctx.fill()
    ctx.restore()
  }
  
  // 绘制爆炸粒子
  for (const p of particles) {
    ctx.save()
    ctx.globalAlpha = p.life
    ctx.translate(p.x, p.y)
    ctx.rotate(p.rotation)
    
    // 绘制星形粒子
    ctx.beginPath()
    ctx.fillStyle = p.color
    ctx.shadowBlur = 15
    ctx.shadowColor = p.color
    
    // 绘制星形
    const spikes = 5
    const outerRadius = p.size * 2
    const innerRadius = p.size
    
    for (let i = 0; i < spikes * 2; i++) {
      const radius = i % 2 === 0 ? outerRadius : innerRadius
      const angle = (Math.PI * i) / spikes
      ctx.lineTo(Math.cos(angle) * radius, Math.sin(angle) * radius)
    }
    
    ctx.closePath()
    ctx.fill()
    
    // 绘制圆形粒子
    ctx.beginPath()
    ctx.arc(0, 0, p.size, 0, Math.PI * 2)
    ctx.fill()
    
    ctx.restore()
  }
}

// 主动画循环
function animate(currentTime: number) {
  if (!fireworksCanvas.value || !canvasContainer.value) return
  
  const ctx = fireworksCanvas.value.getContext('2d')
  if (!ctx) return
  
  const width = canvasContainer.value.clientWidth
  const height = canvasContainer.value.clientHeight
  
  // 设置canvas尺寸
  if (fireworksCanvas.value.width !== width || fireworksCanvas.value.height !== height) {
    fireworksCanvas.value.width = width
    fireworksCanvas.value.height = height
  }
  
  // 随机创建新烟花
  if (props.enabled && Math.random() < 0.05 * intensity.value) {
    fireworks.push(createFirework())
  }
  
  updateFireworks()
  updateParticles()
  drawParticles(ctx, width, height)
  
  animationFrameId = requestAnimationFrame(animate)
}




// 调整canvas尺寸
function resizeCanvas() {
  if (!fireworksCanvas.value || !canvasContainer.value) return
  
  const width = canvasContainer.value.clientWidth
  const height = canvasContainer.value.clientHeight
  
  fireworksCanvas.value.width = width
  fireworksCanvas.value.height = height
}

// 监听props变化
watch(() => props.enabled, (newEnabled) => {
  if (newEnabled) {
    nextTick(() => {
      resizeCanvas()
      if (!animationFrameId) {
        animationFrameId = requestAnimationFrame(animate)
      }
    })
  } else {
    if (animationFrameId) {
      cancelAnimationFrame(animationFrameId)
      animationFrameId = 0
    }
  }
})

watch(() => props.intensity, (newIntensity) => {
  intensity.value = newIntensity || 1
})

onMounted(() => {
  // 初始化canvas尺寸
  nextTick(() => {
    resizeCanvas()
    
    // 设置resize观察器
    if (canvasContainer.value) {
      resizeObserver = new ResizeObserver(resizeCanvas)
      resizeObserver.observe(canvasContainer.value)
    }
    
    // 开始动画
    if (props.enabled) {
      animationFrameId = requestAnimationFrame(animate)
    }
  })
})

onUnmounted(() => {
  // 清理资源
  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId)
  }
  
  if (resizeObserver && canvasContainer.value) {
    resizeObserver.unobserve(canvasContainer.value)
  }
  
  particles = []
})
</script>

<style scoped>
@reference "tailwindcss";

.fireworks-container {
  @apply absolute w-full h-full pointer-events-none z-[-1] overflow-hidden left-0 top-0;
}
.fireworks-canvas {
  @apply absolute w-full h-full pointer-events-none z-[-2] bg-transparent left-0 top-0;
}
</style>
