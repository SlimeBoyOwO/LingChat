<template>
  <button
    v-bind="$attrs"
    class="start-item cursor-pointer border-none bg-transparent p-[5px] text-justify font-['Maoken_Assorted_Sans',-apple-system,BlinkMacSystemFont,'Segoe_UI',Roboto,'Helvetica_Neue',Arial,sans-serif] whitespace-nowrap text-white transition-all duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.28)] text-shadow-[0_2px_5px_rgba(0,0,0,0.45)] hover:not-disabled:-translate-y-2 hover:not-disabled:scale-105 hover:not-disabled:text-[#f0f0f0] hover:not-disabled:text-shadow-[0_2px_5px_rgba(0,0,0,0.45),0_0_8px_rgba(255,255,255,0.45)] disabled:cursor-not-allowed disabled:opacity-50 hover:disabled:text-white hover:disabled:text-shadow-[0_2px_4px_rgba(0,0,0,0.5)] max-[767px]:text-left max-[767px]:whitespace-normal"
  >
    <slot />
  </button>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });
</script>

<style>
/* 白字常压在浅色云背景上，加深色描边保证可读性 */
.start-item {
  -webkit-text-stroke: 0.012em rgba(30, 60, 100, 0.35);
}

/* 一级菜单：行距和字号都由 StartPage 的布局变量控制，
   每行高度约为 1.5 倍字号 + 上下内边距 10px + 行距 */
.start-item:not(.menu-subitem) {
  margin-top: var(--menu-gap, 15px);
  font-size: clamp(
    24px,
    min(
      var(--menu-font-cap, 4vw),
      (
          100svh - 2 * var(--menu-border-y, 6vw) - var(--menu-pad-top, 0px) - var(--menu-rows, 6) *
            (10px + var(--menu-gap, 15px)) - 20px
        ) /
        (1.5 * var(--menu-rows, 6))
    ),
    var(--menu-font-max, 72px)
  );
}

.start-item.menu-subitem {
  margin-top: var(--menu-subitem-gap, 15px);
  font-size: var(--menu-subitem-font, clamp(40px, 4vw, 72px));
}

/* 竖屏二级菜单允许换行，长标题不会撑破列宽 */
@media (max-aspect-ratio: 6/5) {
  .start-item.menu-subitem {
    white-space: normal;
    text-align: center;
  }
}
</style>
