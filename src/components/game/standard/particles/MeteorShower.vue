<template>
  <div class="meteor-shower">
    <!-- 底下垫一层慢慢飘的星辉。只有流星的话画面上是几条线加一大片黑，太空 -->
    <BAParticles class="meteor-shower__sparks" :particle-count="SPARK_COUNT" :speed="SPARK_SPEED" />
    <canvas ref="canvasRef" class="meteor-shower__meteors"></canvas>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import BAParticles from "./BAParticles.vue";
import { useMeteors } from "./hooks/useMeteors";
import { SPARK_COUNT, SPARK_SPEED } from "./config/meteors";

/**
 * 流星雨。
 *
 * 全场流星共用一个飞行方向，只带一点点各自的抖动；入场点沿迎风的边随机，
 * 到达间隔服从指数分布，所以不会踩着拍子来。拖尾长度由速度乘快门时间推出，
 * 火流星另留一道慢慢暗下去的余迹。绘制细节见 hooks/useMeteors.ts。
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

useMeteors(canvasRef, props);
</script>

<style scoped>
.meteor-shower {
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
}

.meteor-shower__sparks,
.meteor-shower__meteors {
  position: absolute;
  inset: 0;
  display: block;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
</style>
