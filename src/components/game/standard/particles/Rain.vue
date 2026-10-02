<template>
  <canvas ref="canvasRef" class="rain-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useRain } from "./hooks/useRain";

/**
 * 雨。
 *
 * 三层景深叠出视差，风向由两条正弦阵风缓慢摆动，雨滴触地溅起涟漪，
 * 镜头玻璃上另有一层虚焦的光斑。绘制细节见 hooks/useRain.ts。
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

useRain(canvasRef, props);
</script>

<style scoped>
.rain-layer {
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
