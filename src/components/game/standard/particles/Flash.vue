<template>
  <div
    v-if="enabled"
    class="flash-container"
    :class="modeClass"
    :style="{ '--flash-opacity': Math.min(0.45 + intensity * 0.3, 0.9) }"
  ></div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

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
  // red = 血红闪烁（惊悚），black =  blackout 断电式黑闪
  mode: {
    type: String,
    default: 'red',
    validator: (value: string) => ['red', 'black'].includes(value),
  },
})

const modeClass = computed(() => (props.mode === 'black' ? 'flash-black' : 'flash-red'))
</script>

<style scoped>
.flash-container {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

/* 心跳式两段闪：亮-暗-亮-长暗，比匀速闪烁更有压迫感 */
.flash-red {
  background: rgb(120, 0, 10);
  animation: flash-pulse 1.6s ease-in-out infinite;
}

.flash-black {
  background: #000;
  animation: flash-pulse 2.2s ease-in-out infinite;
}

@keyframes flash-pulse {
  0%,
  100% {
    opacity: 0;
  }
  8% {
    opacity: var(--flash-opacity, 0.75);
  }
  16% {
    opacity: 0;
  }
  30% {
    opacity: calc(var(--flash-opacity, 0.75) * 0.6);
  }
  45% {
    opacity: 0;
  }
}
</style>
