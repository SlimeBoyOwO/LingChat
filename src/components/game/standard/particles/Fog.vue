<template>
  <canvas ref="canvasRef" class="fog-layer"></canvas>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useFog } from "./hooks/useFog";

/**
 * 雾。
 *
 * 三层纹理带横向漂移出视差，纹理自带上下渐隐，另加一层很薄的整屏纱。
 * 具体参数见 config/fog.ts，绘制细节见 hooks/useFog.ts。
 */
const props = withDefaults(
  defineProps<{
    enabled?: boolean;
    /** 浓度倍率，0 到 2 */
    intensity?: number;
  }>(),
  {
    enabled: true,
    intensity: 1,
  },
);

const canvasRef = ref<HTMLCanvasElement | null>(null);

useFog(canvasRef, props);
</script>

<style scoped>
.fog-layer {
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
