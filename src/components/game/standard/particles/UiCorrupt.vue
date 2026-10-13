<template>
  <!-- UI 崩坏：给 body 挂全局类，让对话框/按钮等 UI 文字出现 RGB 错位与抖动 -->
  <div
    v-show="false"
    class="ui-corrupt-anchor"
  ></div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from 'vue'

const props = defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
})

const CLASS = 'ling-ui-corrupt'

function apply() {
  if (props.enabled) document.body.classList.add(CLASS)
  else document.body.classList.remove(CLASS)
}

onMounted(apply)
watch(() => props.enabled, apply)
onBeforeUnmount(() => document.body.classList.remove(CLASS))
</script>

<style>
/* 非 scoped：作用于全局 UI。仅在 body.ling-ui-corrupt 下生效 */
body.ling-ui-corrupt #app {
  animation: ui-corrupt-jitter 0.9s steps(2) infinite;
}

/* 文字 RGB 错位（色散） */
body.ling-ui-corrupt #app * {
  text-shadow:
    1.5px 0 rgba(255, 0, 60, 0.7),
    -1.5px 0 rgba(0, 255, 220, 0.7) !important;
}

@keyframes ui-corrupt-jitter {
  0%,
  88%,
  100% {
    transform: translate(0, 0);
    filter: none;
  }
  90% {
    transform: translate(-3px, 1px);
    filter: hue-rotate(40deg);
  }
  94% {
    transform: translate(3px, -2px);
    filter: hue-rotate(-30deg) saturate(1.6);
  }
  97% {
    transform: translate(-1px, 2px);
  }
}
</style>
