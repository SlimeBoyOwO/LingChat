<template>
  <canvas id="glcanvas" class="rain-container" ref="canvasRef" />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
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

// 响应式雨滴数量
const dropCount = ref(Math.floor(50 * props.intensity))

let W = 0,
  H = 0

let drops: Drop[] = []

let ctx: CanvasRenderingContext2D | null = null
let animId = 0

const { createDrop } = useRain()

/**
 * 处理窗口 resize，更新 Canvas 尺寸并重新初始化雨滴
 */
function handleResize() {
  if (!canvasRef.value) return

  canvasRef.value.width = window.innerWidth
  canvasRef.value.height = window.innerHeight
  W = canvasRef.value.width
  H = canvasRef.value.height

  // 重新初始化雨滴以适应新尺寸
  drops = []
  for (let i = 0; i < dropCount.value; i++) {
    drops.push(createDrop(W, H, props.intensity))
  }
}

function init() {
  if (!props.enabled) return

  const canvas = canvasRef.value
  if (!canvas) return

  canvas.width = window.innerWidth
  canvas.height = window.innerHeight
  W = canvas.width
  H = canvas.height
  ctx = canvas.getContext('2d')

  drops = []
  for (let i = 0; i < dropCount.value; i++) {
    drops.push(createDrop(W, H, props.intensity))
  }

  loop()
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

    // 雨滴超出屏幕时，重置位置并重新随机化 x 坐标
    if (drop.y > H) {
      drop.y = -drop.length
      drop.x = Math.random() * W
    }
  }

  animId = requestAnimationFrame(loop)
}

// 监听 intensity 变化，动态调整雨滴数量
watch(
  () => props.intensity,
  (newIntensity) => {
    dropCount.value = Math.floor(50 * newIntensity)
    handleResize()
  },
)

// 监听 enabled 状态变化
watch(
  () => props.enabled,
  (newVal) => {
    if (newVal) {
      init()
    } else {
      cancelAnimationFrame(animId)
      drops = []
    }
  },
)

onMounted(() => {
  init()
  window.addEventListener('resize', handleResize)
})

onBeforeUnmount(() => {
  cancelAnimationFrame(animId)
  window.removeEventListener('resize', handleResize)
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
