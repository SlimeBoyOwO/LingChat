<template>
  <div
    class="flex flex-1 px-3 py-6 text-white text-shadow-2xs mx-auto items-start min-h-0 max-h-full overflow-y-auto flex-wrap gap-5"
    :style="{ width: containerWidth + '%' }"
  >
    <slot></slot>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'

const uiStore = useUIStore()

// 窄屏适配：ratio 1.0→0.5，宽度 70%→100%（rate=60）
const containerWidth = computed(() => {
  const ratio = uiStore.aspectRatio
  if (ratio >= 1.0) return 70
  const percent = Math.min(100, 70 + (1.0 - ratio) * 60)
  return Math.round(percent)
})
</script>

<style scoped></style>
