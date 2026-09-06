<template>
  <div
    v-if="enabled"
    class="glitch-container"
    :class="{ 'glitch-surge': surging }"
  >
    <!-- RGB 通道错位层 -->
    <div class="glitch-layer glitch-red"></div>
    <div class="glitch-layer glitch-cyan"></div>
    <!-- 扫描线 -->
    <div class="glitch-scanlines"></div>
    <!-- 随机切片抖动条 -->
    <div
      v-for="(s, i) in slices"
      :key="i"
      class="glitch-slice"
      :style="s"
    ></div>
    <!-- 噪点闪屏 -->
    <div
      v-if="noiseOn"
      class="glitch-noise"
    ></div>
  </div>
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

interface SliceStyle {
  top: string
  height: string
  transform: string
  opacity: number
  background: string
}

const slices = ref<SliceStyle[]>([])
const noiseOn = ref(false)
// 周期性"大故障"：整帧扭曲 + 色相偏移
const surging = ref(false)

let sliceTimer = 0
let noiseTimer = 0
let surgeTimer = 0

function reshuffleSlices() {
  const count = Math.floor(4 + props.intensity * 4)
  const next: SliceStyle[] = []
  for (let i = 0; i < count; i++) {
    const offset = (Math.random() - 0.5) * 120 * props.intensity
    next.push({
      top: `${Math.random() * 100}%`,
      height: `${6 + Math.random() * 42}px`,
      transform: `translateX(${offset}px)`,
      opacity: 0.3 + Math.random() * 0.5,
      background:
        Math.random() > 0.5
          ? 'rgba(255, 0, 60, 0.5)'
          : 'rgba(0, 255, 220, 0.45)',
    })
  }
  slices.value = next
  sliceTimer = window.setTimeout(() => {
    slices.value = []
    sliceTimer = window.setTimeout(reshuffleSlices, 100 + Math.random() * 420)
  }, 70 + Math.random() * 130)
}

function flashNoise() {
  noiseOn.value = true
  noiseTimer = window.setTimeout(() => {
    noiseOn.value = false
    noiseTimer = window.setTimeout(flashNoise, 180 + Math.random() * 800)
  }, 60 + Math.random() * 90)
}

function surge() {
  surging.value = true
  surgeTimer = window.setTimeout(() => {
    surging.value = false
    surgeTimer = window.setTimeout(surge, 1500 + Math.random() * 4000)
  }, 120 + Math.random() * 220)
}

onMounted(() => {
  if (!props.enabled) return
  reshuffleSlices()
  flashNoise()
  surge()
})

onBeforeUnmount(() => {
  clearTimeout(sliceTimer)
  clearTimeout(noiseTimer)
  clearTimeout(surgeTimer)
})
</script>

<style scoped>
.glitch-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  overflow: hidden;
}

/* 大故障期间整帧扭曲 + 色相偏移 */
.glitch-surge {
  animation: glitch-surge-anim 0.12s steps(2) infinite;
  filter: hue-rotate(90deg) saturate(2);
}

@keyframes glitch-surge-anim {
  0% {
    transform: translateX(-8px) skewX(2deg);
  }
  50% {
    transform: translateX(7px) skewX(-1.5deg);
  }
  100% {
    transform: translateX(-3px);
  }
}

.glitch-layer {
  position: absolute;
  inset: 0;
  mix-blend-mode: screen;
}

.glitch-red {
  background: rgba(255, 0, 60, 0.13);
  animation: glitch-shift-a 0.9s steps(2) infinite;
}

.glitch-cyan {
  background: rgba(0, 255, 220, 0.12);
  animation: glitch-shift-b 1.1s steps(2) infinite;
}

@keyframes glitch-shift-a {
  0% {
    transform: translateX(0);
  }
  20% {
    transform: translateX(-12px);
  }
  40% {
    transform: translateX(6px);
  }
  60% {
    transform: translateX(0);
  }
  85% {
    transform: translateX(-6px);
  }
  100% {
    transform: translateX(0);
  }
}

@keyframes glitch-shift-b {
  0% {
    transform: translateX(0);
  }
  25% {
    transform: translateX(10px);
  }
  50% {
    transform: translateX(-8px);
  }
  75% {
    transform: translateX(4px);
  }
  100% {
    transform: translateX(0);
  }
}

.glitch-scanlines {
  position: absolute;
  inset: 0;
  background: repeating-linear-gradient(
    to bottom,
    transparent 0,
    transparent 2px,
    rgba(0, 0, 0, 0.18) 3px,
    transparent 4px
  );
  animation: scanline-move 8s linear infinite;
}

@keyframes scanline-move {
  from {
    background-position: 0 0;
  }
  to {
    background-position: 0 100px;
  }
}

.glitch-slice {
  position: absolute;
  left: 0;
  width: 100%;
  mix-blend-mode: screen;
}

.glitch-noise {
  position: absolute;
  inset: 0;
  background-image:
    radial-gradient(rgba(255, 255, 255, 0.4) 1px, transparent 1px),
    radial-gradient(rgba(0, 0, 0, 0.5) 1px, transparent 1px);
  background-size:
    3px 3px,
    4px 4px;
  background-position:
    0 0,
    1px 2px;
  mix-blend-mode: overlay;
  opacity: 0.65;
}
</style>
