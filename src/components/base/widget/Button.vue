<template>
  <button :disabled="disabled" :class="[baseClasses, typeClasses]" @click="$emit('click', $event)">
    <Icon v-if="icon" :icon="icon" :size="icon_size"></Icon>
    <slot></slot>
  </button>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import Icon from './Icon.vue'
import type { IconType } from './Icon.vue'

interface ButtonProps {
  type?: 'big' | 'menu' | 'nav' | 'select' | 'delete' | 'add' | 'save' | 'start' | 'close'
  disabled?: boolean
  icon?: IconType
  icon_size?: number
  // 新增：用于替代原CSS中的 .nav.active 和 .select.selected
  active?: boolean
}
const props = defineProps<ButtonProps>()

const emit = defineEmits<{
  (e: 'click', event: MouseEvent): void
}>()

// 1. 所有按钮的基础通用样式
const baseClasses = `
  inline-flex items-center justify-center gap-2
  border-none outline-none cursor-pointer
  transition-all duration-200 ease-in-out
  disabled:opacity-60 disabled:cursor-not-allowed
`

// 2. 根据 type 和 active 状态计算特定的 Tailwind 类
const typeClasses = computed(() => {
  switch (props.type) {
    case 'menu':
      return `
        bg-transparent text-white p-[15px] my-[10px] rounded-xl
        text-[clamp(32px,4vw,72px)] font-normal font-['Maoken_Assorted_Sans',sans-serif]
        justify-start [text-shadow:0_2px_4px_rgba(0,0,0,0.5)]
        hover:text-[#f0f0f0] hover:[text-shadow:0_0_10px_rgba(255,255,255,0.8)]
      `

    case 'start':
      return 'px-4 py-2 font-medium rounded bg-[#4caf50] text-white hover:bg-[#45a049]'

    case 'close':
      return 'px-4 py-2 font-medium rounded bg-[#f44336] text-white hover:bg-[#d32f2f]'

    case 'nav':
      return `
        px-[15px] py-[10px] mx-[5px] rounded-lg text-base font-bold relative z-10 
        [text-shadow:0_2px_4px_rgba(0,0,0,0.2)] transition-colors duration-300
        [&>svg]:w-[1.125rem] [&>svg]:h-[1.125rem] [&>svg]:stroke-[2.5px] [&>svg]:shrink-0
        ${
          props.active
            ? 'text-[var(--accent-color)] bg-white/10 hover:bg-white/15'
            : 'text-white bg-transparent hover:text-[var(--accent-color)]'
        }
      `

    case 'big':
      return `
        w-full p-3 text-base font-bold rounded-lg
        bg-[#e9ecef] text-[#495057]
        hover:bg-[var(--accent-color)] hover:text-white 
        hover:-translate-y-0.5 hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)]
      `

    case 'select':
      return `
        self-end px-[15px] py-[8px] rounded-full text-[13px] font-medium
        ${
          props.active
            ? 'bg-[var(--accent-color)] text-white'
            : 'bg-[#ccc] text-[#666] hover:bg-[#555] hover:text-white'
        }
      `

    // 默认兜底样式 (适用于 delete, add, save 等没写特定样式的type)
    default:
      return 'px-4 py-2 font-medium rounded bg-gray-200 text-gray-800 hover:bg-gray-300'
  }
})
</script>

<!-- <style scoped> 已经被完全移除！ -->
