<template>
  <div
    class="flex flex-1 px-3 py-6 text-white text-shadow-2xs mx-auto items-start min-h-0 max-h-full overflow-y-auto flex-wrap gap-5"
    :style="{ width: containerWidth + '%' }"
  >
    <slot></slot>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'

const uiStore = useUIStore()

// 监听窗口绝对宽度 (如果你的 uiStore 中已经有类似 screenWidth 的状态，可以直接替换这段逻辑)
const windowWidth = ref(window.innerWidth)
const updateWidth = () => {
  windowWidth.value = window.innerWidth
}

onMounted(() => window.addEventListener('resize', updateWidth))
onUnmounted(() => window.removeEventListener('resize', updateWidth))

const containerWidth = computed(() => {
  const ratio = uiStore.aspectRatio
  const currentWidth = windowWidth.value

  // 1. 维度一：基于屏幕比例 (原逻辑)
  // 窄屏适配：ratio 1.0→0.5，宽度 70%→100%
  let ratioWidth = 70
  if (ratio < 1.0) {
    ratioWidth = 70 + (1.0 - ratio) * 60
  }

  // 2. 维度二：基于绝对分辨率 (新逻辑)
  // 假设 1024px(如iPad横屏) 是基准，低于此分辨率开始逐渐增加百分比
  // 假设 375px(常见小手机) 时宽度撑满到 100%
  let resolutionWidth = 70
  if (currentWidth < 1024) {
    // 线性插值计算：屏幕从 1024 缩小到 375，百分比从 70 增加到 100
    // 公式: 70 + (1024 - 当前宽度) / (1024 - 375) * 30
    const scale = (1024 - currentWidth) / (1024 - 675)
    resolutionWidth = 70 + scale * 30
  }

  // 3. 综合判断：取两者中需要更宽的那一个，并限制最高不超过 100%
  const finalPercent = Math.max(ratioWidth, resolutionWidth)

  return Math.round(Math.min(100, finalPercent))
})
</script>
