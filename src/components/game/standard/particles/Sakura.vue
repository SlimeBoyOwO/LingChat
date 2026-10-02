<template>
  <canvas ref="canvasRef" class="sakura-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useFallingParticle } from "./hooks/useFallingParticle";
import { sakura } from "./config/sakura";

/**
 * 樱花。
 *
 * 花瓣一边下落一边自转，并用横向宽度的正弦振荡伪造三维翻面。
 * 与雪共用同一套渲染场，绘制细节见 hooks/useFallingField.ts。
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

useFallingParticle(canvasRef, props, sakura);
</script>

<style scoped>
.sakura-layer {
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
