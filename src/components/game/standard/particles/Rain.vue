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

// 帧率限制相关变量
const TARGET_FPS = 60
const FRAME_INTERVAL = 1000 / TARGET_FPS // 约 16.67ms
let lastFrameTime = 0

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

  // 重置上一帧时间戳
  lastFrameTime = 0
  loop(0)
}

/**
 * 更新雨滴位置（基于固定时间步长）
 * @param elapsedMs 距离上一帧经过的时间（毫秒）
 */
function updateDrops(elapsedMs: number) {
  // 将时间转换为速度因子，保持雨滴移动速度与帧率解耦
  // 原始速度基于每帧移动 speed 像素（假设 60fps）
  // 现根据实际经过时间调整移动距离
  const speedFactor = elapsedMs / FRAME_INTERVAL

  for (const drop of drops) {
    // 根据时间差移动雨滴
    drop.y += drop.speed * speedFactor

    // 雨滴超出屏幕时，重置位置并重新随机化 x 坐标
    if (drop.y > H) {
      drop.y = -drop.length
      drop.x = Math.random() * W
    }
  }
}

function render() {
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
  }
}

function loop(currentTime: number) {
  if (!ctx) return

  // 初始化上一帧时间
  if (lastFrameTime === 0) {
    lastFrameTime = currentTime
    animId = requestAnimationFrame(loop)
    return
  }

  // 计算距离上一帧的时间差（毫秒）
  let elapsed = currentTime - lastFrameTime

  // 限制最大时间差，避免跳跃过大（例如切换标签页后恢复）
  const MAX_DELTA = 100 // 最大100ms
  if (elapsed > MAX_DELTA) {
    elapsed = MAX_DELTA
  }

  // 只有达到帧间隔时间才更新逻辑和渲染
  if (elapsed >= FRAME_INTERVAL) {
    // 更新时间戳，但保留超出部分用于下一帧（可选，保持平滑）
    lastFrameTime = currentTime - (elapsed % FRAME_INTERVAL)

    // 使用固定时间步长更新雨滴位置（保持速度稳定）
    // 这里使用 FRAME_INTERVAL 作为标准步长，因为 elapsed 可能大于 FRAME_INTERVAL
    // 为了更精确，可以使用 elapsed 但会受帧率波动影响，这里采用标准步长保证每帧移动距离一致
    // 但如果帧率掉帧严重，使用固定步长会导致移动变慢，因此采用实际经过时间
    // 改进：直接使用 elapsed 作为时间差，但速度因子基于实际时间与基准帧间隔的比例
    updateDrops(elapsed)
    render()
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
