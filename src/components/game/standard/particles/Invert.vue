<template>
  <!-- 反色闪屏：白色叠层 + difference 混合 = 画面反色，随机短促爆发 -->
  <div
    v-if="enabled && active"
    class="invert-layer"
  ></div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
  intensity: {
    type: Number,
    default: 1,
    validator: (value: number) => value >= 0 && value <= 2,
  },
})

const active = ref(false)
let timer = 0

function burst() {
  if (!props.enabled) return
  active.value = true
  // 反色持续 80~220ms
  timer = window.setTimeout(() => {
    active.value = false
    // 间隔随强度缩短：0.4s ~ 3s
    const gap = (400 + Math.random() * 2600) / props.intensity
    timer = window.setTimeout(burst, gap)
  }, 80 + Math.random() * 140)
}

onMounted(burst)
watch(
  () => props.enabled,
  (v) => {
    if (v) burst()
    else {
      active.value = false
      clearTimeout(timer)
    }
  },
)
onBeforeUnmount(() => clearTimeout(timer))
</script>

<style scoped>
.invert-layer {
  position: absolute;
  inset: 0;
  background: #fff;
  mix-blend-mode: difference;
  pointer-events: none;
}
</style>
