<template>
  <canvas ref="canvasRef" class="thunderstorm-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useRain } from "./hooks/useRain";
import { THUNDERSTORM_PROFILE } from "./config/thunderstorm";

/**
 * 雷阵雨。跑的是和雨同一台引擎，换一套更密的参数并打开闪电。
 *
 * 它属于「天气」层，与设置页的「氛围」特效相互独立，可以同时开着。
 * 全部绘制细节见 hooks/useRain.ts，这里只有参数表。
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

useRain(canvasRef, props, THUNDERSTORM_PROFILE);
</script>

<style scoped>
.thunderstorm-layer {
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
