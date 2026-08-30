<template>
  <div
    class="start-page absolute top-0 right-0 bottom-0 left-0 z-5 flex items-center justify-center
      border-[6vw] border-transparent will-change-transform [transform-style:preserve-3d]
      md:justify-start"
    v-bind="$attrs"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
  defineOptions({ inheritAttrs: false });
</script>

<style>
  /* 小高度视口（手机横屏）：保留原 6vw 边框间距，仅叠加横向安全区，
   让菜单列避开横屏时位于侧边的灵动岛（桌面/竖屏 safe=0，无影响）。 */
  @media (max-height: 520px) {
    .start-page {
      padding-left: var(--safe-area-inset-left, 0px);
      padding-right: var(--safe-area-inset-right, 0px);
    }
  }
</style>
