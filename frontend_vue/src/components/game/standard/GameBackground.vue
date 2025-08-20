<template>
  <div class="game-background" :style="backgroundStyle"></div>
  <canvas ref="canvasRef" id="canvas"></canvas>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, watch } from "vue";
import { StarField } from "./particles/StarField";
import { useUIStore } from "../../../stores/modules/ui/ui";

const canvasRef = ref<HTMLCanvasElement | null>(null);
const uiStore = useUIStore();

let starField: StarField | null = null;

// 计算背景样式
const backgroundStyle = computed(() => {
  return {
    backgroundImage: uiStore.currentBackground
      ? `url(${uiStore.currentBackground})`
      : "url(@/assets/images/default_bg.jpg)",
  };
});

onMounted(() => {
  if (canvasRef.value) {
    starField = new StarField(canvasRef.value);
  }

  // 监听背景变化
  watch(
    () => uiStore.currentBackground,
    (newBackground) => {
      console.log("背景已更新:", newBackground);
      // 由于使用了 computed，背景会自动更新
    }
  );
});

onUnmounted(() => {
  if (starField) {
    starField.destroy();
  }
});
</script>

<style scoped>
.game-background {
  position: absolute;
  width: 100%;
  height: 100%;
  background-size: cover;
  background-position: center center;
  background-attachment: fixed;
  background-repeat: no-repeat;
  z-index: -2;
  transition: background-image 0.5s ease-in-out; /* 添加过渡效果 */
}

canvas {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: -1;
}

body {
  display: block;
}
</style>
