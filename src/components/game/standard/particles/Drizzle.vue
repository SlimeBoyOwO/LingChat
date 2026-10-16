<template>
  <canvas ref="canvasRef" class="drizzle-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useRain } from "./hooks/useRain";
import { DRIZZLE_PROFILE } from "./config/drizzle";

/**
 * 小雨。跑的是和雨同一台引擎，换一套更稀更慢的参数。
 *
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

useRain(canvasRef, props, DRIZZLE_PROFILE);
</script>

<style scoped>
.drizzle-layer {
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
