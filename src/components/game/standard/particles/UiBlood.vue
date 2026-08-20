<template>
  <!-- 血红 UI：给 body 挂全局类，让界面文字变为血红色（剧本演出用，可随时关闭） -->
  <div
    v-show="false"
    class="ui-blood-anchor"
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

const CLASS = 'ling-ui-blood'

function apply() {
  if (props.enabled) document.body.classList.add(CLASS)
  else document.body.classList.remove(CLASS)
}

onMounted(apply)
watch(() => props.enabled, apply)
onBeforeUnmount(() => document.body.classList.remove(CLASS))
</script>

<style>
/* 非 scoped：作用于全局 UI。仅在 body.ling-ui-blood 下生效 */
body.ling-ui-blood #app * {
  color: #b8091a !important;
  text-shadow:
    0 0 8px rgba(184, 9, 26, 0.75),
    1px 0 rgba(60, 0, 5, 0.6) !important;
}

/* 整体轻微搏动，像文字在渗血 */
body.ling-ui-blood #app {
  animation: ui-blood-pulse 2.4s ease-in-out infinite;
}

@keyframes ui-blood-pulse {
  0%,
  100% {
    filter: brightness(1);
  }
  50% {
    filter: brightness(0.82) saturate(1.4);
  }
}
</style>
