<template>
  <img
    src="../../../../assets/images/LingChatLogo.webp"
    alt="LingChatLogo"
    draggable="false"
    class="start-logo absolute z-5 w-auto cursor-pointer drop-shadow-[0_4px_8px_rgba(0,0,0,0.3)] transition-transform duration-300 hover:scale-105"
    @contextmenu.prevent
    v-bind="$attrs"
  />
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });
</script>

<style scoped>
/* 尺寸与位置由 StartPage 的布局变量决定。
   左右同时定位 + 自动外边距用于竖屏两列布局时的居中 */
.start-logo {
  height: var(--menu-logo-h, 16vw);
  max-width: var(--menu-logo-maxw, 40vw);
  top: var(--safe-area-inset-top, 0px);
  left: var(--menu-logo-left, auto);
  right: var(--menu-logo-right, var(--safe-area-inset-right, 0px));
  margin-left: var(--menu-logo-margin-l, 0px);
  margin-right: var(--menu-logo-margin-r, 0px);
}
</style>
