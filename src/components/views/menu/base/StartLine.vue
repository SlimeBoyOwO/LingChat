<template>
  <div
    :class="{
      'mobile-hide': mobile === false,
    }"
    v-bind="$attrs"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
interface Props {
  mobile?: boolean
}

withDefaults(defineProps<Props>(), {
  mobile: true,
})
</script>

<style scoped>
/*
 * 仅在「触屏 + 窄屏」时隐藏（手机上的桌面专属功能）。
 * 原实现用 hidden sm:block 纯宽度断点，桌面端把窗口拉窄也会误隐藏
 * （如剧本编辑器入口在窄窗口下消失，见 issue #707 图 11）。
 * pointer: coarse + max-width 组合可区分「手机」与「窄桌面窗口」。
 */
.mobile-hide {
  display: block;
}

@media (pointer: coarse) and (max-width: 640px) {
  .mobile-hide {
    display: none;
  }
}
</style>
