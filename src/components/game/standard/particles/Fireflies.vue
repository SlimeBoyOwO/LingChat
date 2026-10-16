<template>
  <canvas ref="canvasRef" class="fireflies-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useFireflies } from "./hooks/useFireflies";

/**
 * 萤火虫。
 *
 * 每只虫子朝自己的落点缓入、停一会儿、再窜向下一处，落点到期就换。
 * 光晕是一张预渲染的精灵，走加色混合叠在一起，靠近的虫子会互相照亮；
 * 闪光用快起慢落的包络，而不是正弦。绘制细节见 hooks/useFireflies.ts。
 */
const props = withDefaults(
  defineProps<{
    enabled?: boolean;
    /** 密度倍率，0 到 2 */
    intensity?: number;
  }>(),
  {
    enabled: true,
    intensity: 1,
  },
);

const canvasRef = ref<HTMLCanvasElement | null>(null);

useFireflies(canvasRef, props);
</script>

<style scoped>
.fireflies-layer {
  position: absolute;
  top: 0;
  left: 0;
  display: block;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: -1;
}
</style>
