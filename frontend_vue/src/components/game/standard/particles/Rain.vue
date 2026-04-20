<template>
  <canvas id="glcanvas" class="rain-container" ref="canvasRef" />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { Drop } from './config/rain'
import { useRain } from './hooks/useRain'

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

const DROP_COUNT = 50 * props.intensity
let W = 0,
  H = 0

let drops: Drop[] = []

let ctx: CanvasRenderingContext2D | null = null
let animId = 0

const { createDrop } = useRain()

function init() {
  if (!props.enabled) return

  const canvas = canvasRef.value
  if (canvas) {
    canvas.width = window.innerWidth
    canvas.height = window.innerHeight
    W = canvasRef.value?.width as number
    H = canvasRef.value?.height as number
    ctx = canvas.getContext('2d')

    for (let i = 0; i < DROP_COUNT; i++) {
      drops.push(createDrop(W, H, props.intensity))
    }
    loop()
  }
}

function loop() {
  if (!ctx) return

  ctx.clearRect(0, 0, W, H)

  for (const drop of drops) {
    ctx.beginPath()
    ctx.moveTo(drop.x, drop.y)
    ctx.lineTo(drop.x, drop.y + drop.length)

    const gradient = ctx.createLinearGradient(drop.x, drop.y, drop.x, drop.y + drop.length)
    gradient.addColorStop(0, 'rgba(255, 255, 255, 0.3)')
    gradient.addColorStop(1, 'rgba(255, 255, 255, 0.7)')

    ctx.strokeStyle = gradient
    ctx.lineWidth = 1.25
    ctx.stroke()
    drop.y += drop.speed

    if (drop.y > H) {
      drop.y = -drop.length
    }
  }

  animId = requestAnimationFrame(loop)
}

onMounted(() => {
  init()
})

onBeforeUnmount(() => {
  cancelAnimationFrame(animId)
})
</script>

<style scoped>
.rain-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: -1;
  overflow: hidden;
}

.rain-item {
  position: absolute;
  display: inline-block;
  width: 2px;
  background: linear-gradient(rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.6));
}
</style>
