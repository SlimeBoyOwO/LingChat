<template>
  <button
    v-bind="$attrs"
    class="start-item bg-transparent
      text-white
      border-none
      mt-[15px]
      p-[5px]
      font-['Maoken_Assorted_Sans',-apple-system,BlinkMacSystemFont,'Segoe_UI',Roboto,'Helvetica_Neue',Arial,sans-serif]
      cursor-pointer
      text-justify
      whitespace-nowrap
      max-[767px]:whitespace-normal
      max-[767px]:text-left
      text-shadow-[0_2px_4px_rgba(0,0,0,0.3)]
      transition-all
      duration-300
      ease-[cubic-bezier(0.18,0.89,0.32,1.28)]
      hover:not-disabled:text-[#f0f0f0]
      hover:not-disabled:text-shadow-[0_0_6px_rgba(255,255,255,0.4)]
      hover:not-disabled:-translate-y-2
      hover:not-disabled:scale-105
      disabled:opacity-50
      disabled:cursor-not-allowed
      hover:disabled:text-white
      hover:disabled:text-shadow-[0_2px_4px_rgba(0,0,0,0.5)]"
    :class="isUltraWide ? 'text-[clamp(28px,3vw,52px)]' : 'text-[clamp(40px,4vw,72px)]'"
  >
    <slot />
  </button>
</template>

<script setup lang="ts">
import { inject } from 'vue'

defineOptions({ inheritAttrs: false })

const isUltraWide = inject<{ value: boolean }>('isUltraWide', { value: false })
</script>

<style>
/* 二级菜单（menu-subitem）：仅当窄屏内容超宽（如"剧情模式（在自由模式进入自由模式）"）
   时缩小字号；一级菜单保持原字号（40px），不缩小、不拥挤。 */
@media (max-width: 767px) {
  .start-item.menu-subitem {
    font-size: 32px;
  }
}
</style>
