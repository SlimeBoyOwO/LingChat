<template>
  <!-- 流星层（SVG动画） -->
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

function getValidStartY() {
  const h = window.innerHeight
  const maxAttempts = 120
  let attempts = 0
  while (attempts < maxAttempts) {
    const startY = -Math.random() * (h * METEOR_CONFIG.START_Y_RANGE)
    const tooClose = activeMeteors.value.some(
      (m) => Math.abs(startY - m.startY) < METEOR_CONFIG.MIN_DISTANCE,
    )
    if (!tooClose) return startY
    attempts++
  }
  return -Math.random() * (h * METEOR_CONFIG.START_Y_RANGE)
}

function createMeteor() {
  const w = window.innerWidth
  const h = window.innerHeight
  const startY = getValidStartY()
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
  const id = meteorIdCounter++

  activeMeteors.value.push({ id, d, duration, startY })
  setTimeout(() => {
    activeMeteors.value = activeMeteors.value.filter((m) => m.id !== id)
  }, duration * 1000)
}

function updateMeteorShower() {
  if (activeMeteors.value.length < METEOR_CONFIG.MAX_COUNT) createMeteor()
}

function startMeteorShower() {
  if (meteorIntervalId) clearInterval(meteorIntervalId)
  for (let i = 0; i < 3; i++) {
    setTimeout(() => {
      if (activeMeteors.value.length < METEOR_CONFIG.MAX_COUNT) createMeteor()
    }, i * 300)
  }
  meteorIntervalId = setInterval(updateMeteorShower, METEOR_CONFIG.GENERATE_INTERVAL)
}

function stopMeteorShower() {
  if (meteorIntervalId) {
    clearInterval(meteorIntervalId)
    meteorIntervalId = null
  }
  activeMeteors.value = []
}

onMounted(() => {
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
