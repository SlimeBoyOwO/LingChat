<template>
  <!-- 全局路由过渡遮罩：常驻 DOM，切 class 触发透明度过渡。
       可见期间吞掉点击，防止过渡中连点触发第二次跳转 -->
  <div
    class="route-fade-mask"
    :class="{ 'is-visible': maskVisible }"
    :style="maskStyle"
    aria-hidden="true"
  ></div>
</template>

<script setup lang="ts">
import { maskStyle, maskVisible } from "@/composables/app/useRouteFade";
</script>

<style scoped>
.route-fade-mask {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  /* 高于通知 10000 与对话框 10001，过渡期间整窗纯黑 */
  z-index: 10050;
  background: #000;
  opacity: 0;
  pointer-events: none;
  transition-property: opacity;
}

.route-fade-mask.is-visible {
  opacity: 1;
  pointer-events: auto;
}
</style>
