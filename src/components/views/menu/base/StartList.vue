<template>
  <nav
    class="start-list flex max-h-full min-h-0 flex-col items-stretch overflow-x-hidden overflow-y-auto"
    v-bind="$attrs"
  >
    <slot />
  </nav>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });
</script>

<style scoped>
/* 滚动只是兜底，隐藏滚动条避免临界高度时闪动 */
.start-list {
  scrollbar-width: none;
}

.start-list::-webkit-scrollbar {
  display: none;
}

/* 主菜单按布局变量切换列数；二级菜单没有 main-menu-list 类，保持单列 */
.main-menu-list {
  display: grid;
  grid-template-columns: repeat(var(--menu-cols, 1), 1fr);
  column-gap: var(--menu-col-gap, 0px);
  position: relative;
  top: var(--menu-list-offset, 0px);
}

/* 竖屏下二级菜单逐项居中，避免长短不一的条目左对齐成锯齿 */
@media (max-aspect-ratio: 6/5) {
  nav:not(.main-menu-list) {
    align-items: center;
  }
}
</style>
