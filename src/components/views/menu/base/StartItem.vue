<template>
  <button
    v-bind="$attrs"
    class="start-item mt-[15px] cursor-pointer border-none bg-transparent p-[5px] text-justify font-['Maoken_Assorted_Sans',-apple-system,BlinkMacSystemFont,'Segoe_UI',Roboto,'Helvetica_Neue',Arial,sans-serif] whitespace-nowrap text-white transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.28)] text-shadow-[0_2px_5px_rgba(0,0,0,0.45)] hover:not-disabled:-translate-y-2 hover:not-disabled:scale-105 hover:not-disabled:text-[#f0f0f0] hover:not-disabled:text-shadow-[0_2px_5px_rgba(0,0,0,0.45),0_0_8px_rgba(255,255,255,0.45)] disabled:cursor-not-allowed disabled:opacity-50 hover:disabled:text-white hover:disabled:text-shadow-[0_2px_4px_rgba(0,0,0,0.5)] max-[767px]:text-left max-[767px]:whitespace-normal"
    :class="isUltraWide ? 'text-[clamp(28px,3vw,52px)]' : 'text-[clamp(40px,4vw,72px)]'"
  >
    <slot />
  </button>
</template>

<script setup lang="ts">
import { inject } from "vue";

defineOptions({ inheritAttrs: false });

const isUltraWide = inject<{ value: boolean }>("isUltraWide", { value: false });
</script>

<style>
/* 白字常压在浅色云背景上，单靠投影对比度不足，加深色描边保证可读性 */
.start-item {
  -webkit-text-stroke: 0.012em rgba(30, 60, 100, 0.35);
}

/* 二级菜单（menu-subitem）：仅当窄屏内容超宽（如"剧情模式（在自由模式进入自由模式）"）
   时缩小字号；一级菜单保持原字号（40px），不缩小、不拥挤。 */
@media (max-width: 767px) {
  /* 一级菜单：竖屏手机视口仅 400px 左右时沿用 40px；平板级宽视口（如 600px）
     靠 vw 项放大，避免字号在宽屏上显得过小 */
  .start-item:not(.menu-subitem) {
    font-size: clamp(40px, 13vw, 84px);
  }
  .start-item.menu-subitem {
    font-size: 32px;
  }
}
</style>
