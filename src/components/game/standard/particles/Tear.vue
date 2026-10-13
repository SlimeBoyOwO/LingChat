<template>
  <!-- 画面撕裂：把当前背景图切成若干水平长条，随机横向错位 -->
  <div
    v-if="enabled && bgUrl"
    class="tear-container"
  >
    <div
      v-for="(s, i) in slices"
      :key="i"
      class="tear-slice"
      :style="s"
    ></div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUIStore } from '../../../../stores/modules/ui/ui'

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

const uiStore = useUIStore()

const bgUrl = computed(() => {
  const bg = uiStore.currentBackground
  if (!bg) return ''
  if (bg.startsWith('http') || bg.startsWith('data:') || bg.startsWith('@/')) return bg
  return convertFileSrc(bg)
})

interface Slice {
  top: string
  height: string
  backgroundImage: string
  backgroundPosition: string
  backgroundSize: string
  transform: string
  opacity: number
}

const SLICE_COUNT = 14
const slices = ref<Slice[]>([])
let timer = 0

function buildSlices() {
  const next: Slice[] = []
  for (let i = 0; i < SLICE_COUNT; i++) {
    const top = (i / SLICE_COUNT) * 100
    const height = 100 / SLICE_COUNT
    next.push({
      top: `${top}%`,
      height: `${height}%`,
      backgroundImage: `url(${bgUrl.value})`,
      backgroundPosition: `center ${(top / (100 - height)) * 100}%`,
      backgroundSize: 'cover',
      transform: 'translateX(0)',
      opacity: 0.85,
    })
  }
  slices.value = next
}

function jitter() {
  // 每条切片按"开/关"节奏随机错位，还原 DDLC Tear 的抽动观感
  for (const s of slices.value) {
    if (Math.random() < 0.45) {
      const offset = (Math.random() - 0.5) * 90 * props.intensity
      s.transform = `translateX(${offset}px)`
    } else {
      s.transform = 'translateX(0)'
    }
  }
  timer = window.setTimeout(jitter, 90 + Math.random() * 260)
}

onMounted(() => {
  if (!props.enabled) return
  buildSlices()
  jitter()
})

onBeforeUnmount(() => clearTimeout(timer))
</script>

<style scoped>
.tear-container {
  position: absolute;
  inset: 0;
  pointer-events: none;
  overflow: hidden;
}

.tear-slice {
  position: absolute;
  left: -5%;
  width: 110%;
  background-repeat: no-repeat;
  will-change: transform;
}
</style>
