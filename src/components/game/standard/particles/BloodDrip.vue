<template>
  <!-- 血滴粒子：红色液滴受重力下落（canvas 实现） -->
  <canvas
    v-if="enabled"
    ref="canvasRef"
    class="blood-canvas"
  />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
  intensity: {
    type: Number,
    default: 1,
    validator: (value: number) => value >= 0 && value <= 2,
  },
})

interface Drop {
  x: number
  y: number
  vy: number
  vx: number
  size: number
  alpha: number
}

const canvasRef = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null
let animId = 0
let drops: Drop[] = []
let lastTime = 0

const GRAVITY = 900 // px/s²

function spawnDrop(W: number, fromTop: boolean): Drop {
  return {
    x: Math.random() * W,
    y: fromTop ? -10 : Math.random() * window.innerHeight * 0.5,
    vy: 20 + Math.random() * 60,
    vx: (Math.random() - 0.5) * 15,
    size: 2 + Math.random() * 5,
    alpha: 0.55 + Math.random() * 0.35,
  }
}

function loop(time: number) {
  if (!ctx || !canvasRef.value) return
  const W = canvasRef.value.width
  const H = canvasRef.value.height
  const dt = Math.min((time - lastTime) / 1000 || 0.016, 0.05)
  lastTime = time

  // 持续补充血滴，密度随强度
  const target = Math.floor(18 * props.intensity)
  while (drops.length < target) drops.push(spawnDrop(W, true))

  ctx.clearRect(0, 0, W, H)
  for (const d of drops) {
    d.vy += GRAVITY * dt
    d.x += d.vx * dt
    d.y += d.vy * dt

    // 液滴：圆头 + 上拖尾
    ctx.fillStyle = `rgba(140, 0, 8, ${d.alpha})`
    ctx.beginPath()
    ctx.ellipse(d.x, d.y, d.size * 0.6, d.size * 1.6, 0, 0, Math.PI * 2)
    ctx.fill()
    ctx.beginPath()
    ctx.arc(d.x, d.y + d.size * 1.2, d.size * 0.65, 0, Math.PI * 2)
    ctx.fill()
  }
  drops = drops.filter((d) => d.y < H + 20)

  animId = requestAnimationFrame(loop)
}

function resize() {
  if (!canvasRef.value) return
  canvasRef.value.width = window.innerWidth
  canvasRef.value.height = window.innerHeight
}

onMounted(() => {
  const c = canvasRef.value
  if (!c) return
  resize()
  ctx = c.getContext('2d')
  window.addEventListener('resize', resize)
  animId = requestAnimationFrame(loop)
})

onBeforeUnmount(() => {
  cancelAnimationFrame(animId)
  window.removeEventListener('resize', resize)
})
</script>

<style scoped>
.blood-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
</style>
