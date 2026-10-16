<template>
  <canvas ref="canvasRef" class="blizzard-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useFallingParticle } from "./hooks/useFallingParticle";
import { blizzard } from "./config/blizzard";

/**
 * 雪暴。与雪共用同一套渲染场，只是雪片更小更密、被风横着抽过去。
 *
 * 全部绘制细节见 hooks/useFallingField.ts，这里只有参数表。
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

useFallingParticle(canvasRef, props, blizzard);
</script>

<style scoped>
.blizzard-layer {
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
