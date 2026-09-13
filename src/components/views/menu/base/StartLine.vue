<template>
  <div
    :class="{
      'mobile-hide': mobile === false && isMobile(),
    }"
    v-bind="$attrs"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
  import { isMobile } from '@/utils/platform';

  defineOptions({ inheritAttrs: false });

  interface Props {
    mobile?: boolean;
  }

  withDefaults(defineProps<Props>(), {
    mobile: true,
  });
</script>

<style scoped>
  /*
   * 仅在移动端（Android/iOS）隐藏桌面专属入口，用项目 isMobile() 运行时判定。
   * 原实现用 hidden sm:block 纯宽度断点，桌面端把窗口拉窄也会误隐藏
   * （如剧本编辑器入口在窄窗口下消失，见 issue #707 图 11）；
   * 纯 CSS 的 (pointer: coarse) 媒体查询在 Surface 等触屏笔记本上同样会误判，
   * 因此改为运行时环境判定，桌面端（含触屏笔记本）任何窗口宽度都不隐藏。
   */
  .mobile-hide {
    display: none;
  }
</style>
