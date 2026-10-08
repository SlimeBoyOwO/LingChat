<template>
  <!-- 电视雪花/矩形噪点：随机色块在网格上跳动（DDLC RectStatic 风格） -->
  <canvas
    v-if="enabled"
    ref="canvasRef"
    class="static-canvas"
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

const canvasRef = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null
let animId = 0
let lastFrame = 0

const CELL = 32
const PALETTE = ['#0a0a0a', '#1a1a1a', '#2d0a10', '#0a1a1a', '#3d0007', '#062a26', '#111']

function draw(time: number) {
  if (!ctx || !canvasRef.value) return
  // 每 ~70ms 重排一次，太快会晃眼
  if (time - lastFrame < 70) {
    animId = requestAnimationFrame(draw)
    return
  }
  lastFrame = time

  const W = canvasRef.value.width
  const H = canvasRef.value.height
  ctx.clearRect(0, 0, W, H)

  const cols = Math.ceil(W / CELL)
  const rows = Math.ceil(H / CELL)
  // 覆盖率随强度提升，基础约 6%
  const chance = 0.06 * props.intensity
  for (let x = 0; x < cols; x++) {
    for (let y = 0; y < rows; y++) {
      if (Math.random() < chance) {
        ctx.fillStyle = PALETTE[(Math.random() * PALETTE.length) | 0]
        ctx.fillRect(x * CELL, y * CELL, CELL, CELL)
      }
    }
  }
  animId = requestAnimationFrame(draw)
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
  animId = requestAnimationFrame(draw)
})

onBeforeUnmount(() => {
  cancelAnimationFrame(animId)
  window.removeEventListener('resize', resize)
})
</script>

<style scoped>
.static-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
</style>
