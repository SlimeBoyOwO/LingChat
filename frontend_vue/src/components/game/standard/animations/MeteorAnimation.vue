<template>
  <div v-if="meteorsEnabled" class="meteor-wrapper">
    <svg id="meteor-svg" ref="svgRef">
      <defs>
        <linearGradient
          id="meteor-grad-fixed"
          gradientUnits="objectBoundingBox"
          x1="1"
          y1="0"
          x2="0"
          y2="0"
        >
          <stop offset="0%" stop-color="rgba(255,255,255,1)" />
          <stop offset="40%" stop-color="rgba(255,255,255,1)" />
          <stop offset="70%" stop-color="rgba(220,240,255,0.8)" />
          <stop offset="100%" stop-color="rgba(180,220,255,0)" />
        </linearGradient>
      </defs>
      <!-- Vue 响应式渲染流星路径 -->
      <path
        v-for="meteor in activeMeteors"
        :key="meteor.id"
        :d="meteor.d"
        class="meteor-path"
        :style="{ animationDuration: `${meteor.duration}s` }"
      />
    </svg>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'

interface MeteorTemplate {
  d: string
  duration: number
  startYRatio: number // 相对于屏幕高度的比例
}

interface MeteorData {
  id: number
  d: string
  duration: number
  startY: number
}

const props = defineProps<{
  meteorsEnabled: boolean
}>()

const activeMeteors = ref<MeteorData[]>([])
let meteorIdCounter = 0
let meteorIntervalId: ReturnType<typeof setInterval> | null = null

// 预缓存的流星模板（10个）
const METEOR_CACHE_SIZE = 10
let meteorTemplates: MeteorTemplate[] = []
let lastUsedTemplateIndex = -1

const METEOR_CONFIG = {
  MAX_COUNT: 3,
  GENERATE_INTERVAL: 1000,
  MIN_DISTANCE: 800,
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
}

/**
 * 预生成流星模板缓存
 * 使用当前窗口尺寸生成固定的流星路径模板
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

    const d = `M ${startX} ${startY} Q ${controlX} ${controlY} ${endX} ${endY}`

    meteorTemplates.push({
      d,
      duration,
      startYRatio: -startY / h, // 存储为正数比例
    })
  }
}

/**
 * 从缓存中获取一个流星模板
 * 使用简单的轮询方式避免连续使用同一个模板
 */
function getMeteorTemplate(): MeteorTemplate {
  let index = Math.floor(Math.random() * METEOR_CACHE_SIZE)
  // 避免连续使用同一个模板
  while (index === lastUsedTemplateIndex && METEOR_CACHE_SIZE > 1) {
    index = Math.floor(Math.random() * METEOR_CACHE_SIZE)
  }
  lastUsedTemplateIndex = index
  return meteorTemplates[index]!
}

/**
 * 获取多个不重复的流星模板
 * 确保返回的模板数量不超过缓存大小
 */
function getMultipleTemplates(count: number): MeteorTemplate[] {
  const result: MeteorTemplate[] = []
  const usedIndices = new Set<number>()

  // 按startYRatio排序，选择分散的模板
  const sortedTemplates = meteorTemplates
    .map((t, i) => ({ template: t, index: i }))
    .sort((a, b) => a.template.startYRatio - b.template.startYRatio)

  // 从排序后的模板中等间隔选择
  const step = Math.floor(sortedTemplates.length / count)
  for (let i = 0; i < count && i < sortedTemplates.length; i++) {
    const idx = (i * step) % sortedTemplates.length
    const sortedItem = sortedTemplates[idx]
    if (sortedItem) {
      result.push(sortedItem.template)
      usedIndices.add(sortedItem.index)
    }
  }

  return result
}

function createMeteor() {
  const h = window.innerHeight

  // 从缓存获取模板
  const template = getMeteorTemplate()
  createMeteorFromTemplate(template)
}

/**
 * 批量创建流星，确保它们分散显示
 */
function createMeteorsBatch(count: number) {
  const templates = getMultipleTemplates(count)
  for (const template of templates) {
    createMeteorFromTemplate(template)
  }
}

function createMeteorFromTemplate(template: MeteorTemplate) {
  const h = window.innerHeight
  const startY = -template.startYRatio * h
  const id = meteorIdCounter++

  activeMeteors.value.push({
    id,
    d: template.d,
    duration: template.duration,
    startY,
  })

  setTimeout(() => {
    activeMeteors.value = activeMeteors.value.filter((m) => m.id !== id)
  }, template.duration * 1000)
}

function updateMeteorShower() {
  if (activeMeteors.value.length < METEOR_CONFIG.MAX_COUNT) createMeteor()
}

function startMeteorShower() {
  if (meteorIntervalId) clearInterval(meteorIntervalId)

  // 初始化模板缓存
  if (meteorTemplates.length === 0) {
    initMeteorTemplateCache()
  }

  // 立即加载 MAX_COUNT 颗流星（使用分散的模板确保多颗流星同时显示）
  createMeteorsBatch(METEOR_CONFIG.MAX_COUNT)
  meteorIntervalId = setInterval(updateMeteorShower, METEOR_CONFIG.GENERATE_INTERVAL)
}

function handleResize() {
  // 窗口大小变化时重新生成模板缓存
  initMeteorTemplateCache()
}

function stopMeteorShower() {
  if (meteorIntervalId) {
    clearInterval(meteorIntervalId)
    meteorIntervalId = null
  }
  activeMeteors.value = []
}

// 页面可见性优化：当页面不可见时暂停动画
function handleVisibilityChange() {
  if (document.hidden) {
    if (meteorIntervalId) {
      clearInterval(meteorIntervalId)
      meteorIntervalId = null
    }
  } else if (props.meteorsEnabled) {
    if (!meteorIntervalId) {
      meteorIntervalId = setInterval(updateMeteorShower, METEOR_CONFIG.GENERATE_INTERVAL)
    }
  }
}

onMounted(() => {
  document.addEventListener('visibilitychange', handleVisibilityChange)
  window.addEventListener('resize', handleResize)

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
  window.removeEventListener('resize', handleResize)
  stopMeteorShower()
})
</script>

<style scoped>
/* ========== 流星层 ========== */
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

#meteor-svg {
  width: 100%;
  height: 100%;
}

.meteor-path {
  fill: none;
  stroke: url(#meteor-grad-fixed);
  stroke-width: 5;
  stroke-linecap: round;
  filter: drop-shadow(0 0 20px rgba(255, 255, 255, 0.95))
    drop-shadow(0 0 12px rgba(180, 220, 255, 0.9));
  stroke-dasharray: 600, 3000;
  stroke-dashoffset: 3000;
  animation-name: meteor-move;
  animation-timing-function: linear;
  animation-fill-mode: forwards;
}

@keyframes meteor-move {
  0% {
    stroke-dashoffset: 3000;
  }
  100% {
    stroke-dashoffset: -3000;
  }
}
</style>
