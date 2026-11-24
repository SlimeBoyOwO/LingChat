<template>
  <div
    class="start-page absolute top-0 right-0 bottom-0 left-0 z-5 flex items-center justify-center border-[6vw] border-transparent will-change-transform [transform-style:preserve-3d]"
    v-bind="$attrs"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });
</script>

<style>
/* 菜单响应式参数只在 StartPage 定义，按宽高比切换状态；
   LOGO、列表、按钮只消费变量，不再各自写断点和数值。 */
.start-page {
  --menu-cols: 1;
  --menu-rows: 6;
  --menu-gap: 15px;
  --menu-font-cap: 4vw;
  --menu-font-max: 72px;
  --menu-subitem-font: clamp(40px, 4vw, 72px);
  --menu-subitem-gap: 15px;
  --menu-col-gap: 0px;
  --menu-list-offset: 0px;
  --menu-border-y: min(6vw, 6vh);
  --menu-pad-top: 0px;
  --menu-logo-h: 16vw;
  --menu-logo-maxw: 40vw;
  --menu-logo-left: auto;
  --menu-logo-right: var(--safe-area-inset-right, 0px);
  --menu-logo-margin-l: 0px;
  --menu-logo-margin-r: 0px;

  justify-content: flex-start;
  padding-top: var(--menu-pad-top);
  padding-left: 2.5vw;
  border-top-width: var(--menu-border-y);
  border-bottom-width: var(--menu-border-y);
}

/* 小高度视口（手机横屏）：叠加横向安全区，避开侧边灵动岛 */
@media (max-height: 520px) {
  .start-page {
    padding-left: var(--safe-area-inset-left, 0px);
    padding-right: var(--safe-area-inset-right, 0px);
  }
}

/* 竖屏/近方形：菜单居中，顶部给 LOGO 留出空间 */
@media (max-aspect-ratio: 6/5) {
  .start-page {
    justify-content: center;
    padding-left: 0;
    --menu-border-y: 6vw;
    --menu-font-cap: 19vw;
    --menu-font-max: 84px;
    --menu-subitem-font: clamp(38px, 8.2vw, 72px);
    --menu-subitem-gap: 4px;
    --menu-logo-h: min(24vw, 10vh);
    --menu-logo-maxw: 55vw;
    --menu-pad-top: calc(var(--menu-logo-h) + 8px + var(--safe-area-inset-top, 0px));
  }
}

/* 窄且矮的单列：压缩行距，把空间让给字号 */
@media (max-aspect-ratio: 6/5) and (max-width: 499px) and (max-height: 700px) {
  .start-page {
    --menu-gap: 4px;
  }
}

/* 较宽但偏矮的竖屏：两列三行，LOGO 居中并放大 */
@media (min-width: 500px) and (max-width: 767px) and (max-height: 800px) and (max-aspect-ratio: 6/5) {
  .start-page {
    --menu-cols: 2;
    --menu-rows: 3;
    --menu-font-cap: 9.8vw;
    --menu-font-max: 72px;
    --menu-col-gap: 4vw;
    --menu-list-offset: -4vh;
    --menu-logo-h: min(24vw, 18vh);
    --menu-logo-left: 0px;
    --menu-logo-right: 0px;
    --menu-logo-margin-l: auto;
    --menu-logo-margin-r: auto;
  }
}

/* 超宽：两列三行，宽度充足，字号只受高度约束 */
@media (min-aspect-ratio: 2/1) {
  .start-page {
    --menu-cols: 2;
    --menu-rows: 3;
    --menu-font-cap: 100vw;
    --menu-font-max: 52px;
  }
}
</style>
