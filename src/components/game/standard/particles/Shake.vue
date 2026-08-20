<template>
  <!--
    震屏特效：本身只是全屏指针事件透明的占位层，
    通过 watch 把震动动画类挂到游戏画面容器上，
    卸载/关闭时负责还原，避免残留抖动。
  -->
  <div
    v-show="false"
    class="shake-anchor"
    ref="anchorRef"
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

const anchorRef = ref<HTMLElement | null>(null)
const shakeTarget = ref<HTMLElement | null>(null)

const SHAKE_CLASS = 'ling-shake-active'

function findShakeTarget(anchor: HTMLElement | null): HTMLElement | null {
  // 锚点的父级是粒子层，粒子层的父级即游戏画面容器；找不到就退回 body
  const layer = anchor?.parentElement
  const target = (layer?.parentElement as HTMLElement | null) ?? document.body
  if (!layer?.parentElement) {
    console.warn('[Shake] 未找到画面容器，退回 document.body')
  }
  return target
}

function applyShake() {
  if (!shakeTarget.value) return
  shakeTarget.value.style.setProperty('--shake-amplitude', `${8 * props.intensity}px`)
  shakeTarget.value.classList.add(SHAKE_CLASS)
}

function removeShake() {
  shakeTarget.value?.classList.remove(SHAKE_CLASS)
  shakeTarget.value?.style.removeProperty('--shake-amplitude')
}

onMounted(() => {
  shakeTarget.value = findShakeTarget(anchorRef.value)
  if (props.enabled) applyShake()
})

watch(
  () => props.enabled,
  (val) => {
    if (val) applyShake()
    else removeShake()
  },
)

watch(
  () => props.intensity,
  () => {
    if (props.enabled) applyShake()
  },
)

onBeforeUnmount(removeShake)
</script>

<style>
/* 非 scoped：类挂在组件外的容器上 */
.ling-shake-active {
  animation: ling-shake 0.16s linear infinite;
  will-change: transform;
}

@keyframes ling-shake {
  0% {
    transform: translate(0, 0);
  }
  20% {
    transform: translate(var(--shake-amplitude, 8px), calc(var(--shake-amplitude, 8px) * -0.6));
  }
  40% {
    transform: translate(calc(var(--shake-amplitude, 8px) * -0.8), var(--shake-amplitude, 8px));
  }
  60% {
    transform: translate(var(--shake-amplitude, 8px), calc(var(--shake-amplitude, 8px) * 0.5));
  }
  80% {
    transform: translate(calc(var(--shake-amplitude, 8px) * -0.5), calc(var(--shake-amplitude, 8px) * -1));
  }
  100% {
    transform: translate(0, 0);
  }
}
</style>
