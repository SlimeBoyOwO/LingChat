<template>
  <!-- 黑暗侵蚀：四角暗角像血管一样搏动收拢，画面边缘轻微抖动 -->
  <div
    v-if="enabled"
    class="veins-layer"
    :style="{ '--vein-strength': 0.55 + intensity * 0.25 }"
  ></div>
</template>

<script setup lang="ts">
defineProps({
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
</script>

<style scoped>
.veins-layer {
  position: absolute;
  inset: -3%;
  pointer-events: none;
  background: radial-gradient(
    ellipse at center,
    transparent 28%,
    rgba(10, 0, 2, calc(var(--vein-strength, 0.8) * 0.55)) 62%,
    rgba(5, 0, 1, var(--vein-strength, 0.8)) 100%
  );
  animation:
    veins-pulse 2.6s ease-in-out infinite,
    veins-drift 7s ease-in-out infinite;
}

/* 搏动：暗角周期性收紧，模拟心跳 */
@keyframes veins-pulse {
  0%,
  100% {
    transform: scale(1);
    opacity: 0.85;
  }
  12% {
    transform: scale(1.04);
    opacity: 1;
  }
  24% {
    transform: scale(0.99);
    opacity: 0.8;
  }
  36% {
    transform: scale(1.02);
    opacity: 0.95;
  }
  55% {
    transform: scale(1);
    opacity: 0.85;
  }
}

/* 漂移：整体轻微游走，避免死板 */
@keyframes veins-drift {
  0%,
  100% {
    translate: 0 0;
  }
  25% {
    translate: 1.2% 0.8%;
  }
  50% {
    translate: -0.8% 1.2%;
  }
  75% {
    translate: 0.6% -1%;
  }
}
</style>
