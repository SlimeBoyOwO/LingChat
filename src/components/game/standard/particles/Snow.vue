<template>
  <canvas ref="canvasRef" class="snow-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useFallingParticle } from "./hooks/useFallingParticle";
import { snow } from "./config/snow";

/**
 * 雪。
 *
 * 三层景深叠出视差，冷白的雪花字形缓慢飘落，横向风与逐粒正弦横摆叠加。
 * 与樱花共用同一套渲染场，绘制细节见 hooks/useFallingField.ts。
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

useFallingParticle(canvasRef, props, snow);
</script>

<style scoped>
.snow-layer {
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
