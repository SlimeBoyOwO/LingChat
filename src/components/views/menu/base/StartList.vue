<template>
  <nav
    class="flex
      flex-col
      items-stretch"
    :class="responsive && isUltraWide ? 'grid grid-cols-2' : ''"
    v-bind="$attrs"
  >
    <slot />
  </nav>
</template>

<script setup lang="ts">
import { inject, onUnmounted, provide, ref } from 'vue'

interface Props {
  /** 是否启用超宽屏 2 列切换（仅主菜单开启，二级菜单保持单列） */
  responsive?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  responsive: false,
})

const isUltraWide = ref(false)

let mq: MediaQueryList | null = null
let mqListener: ((e: MediaQueryListEvent) => void) | null = null

function update() {
  const matched = mq ? mq.matches : false
  // matchMedia 的 min-aspect-ratio: 2/1 在恰好 2.0 时返回 true，需求是严格 > 2，用 JS 计算修正
  isUltraWide.value = matched && window.innerWidth / window.innerHeight > 2
}

if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
  mq = window.matchMedia('(min-aspect-ratio: 2/1)')
  update()
  mqListener = () => update()
  mq.addEventListener('change', mqListener)
}

provide('isUltraWide', isUltraWide)

onUnmounted(() => {
  if (mq && mqListener) {
    mq.removeEventListener('change', mqListener)
  }
})
</script>
