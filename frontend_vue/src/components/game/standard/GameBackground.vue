<template>
  <div class="game-background"></div>
  <canvas ref="canvasRef" id="canvas"></canvas>
</template>

<script setup lang="ts">
// TODO 根据API获取的内容，动态设置背景css元素，background-image
import { onMounted, onUnmounted, ref } from "vue";
import { StarField } from "./particles/StarField";

const canvasRef = ref<HTMLCanvasElement | null>(null);
let starField: StarField | null = null;

onMounted(() => {
  if (canvasRef.value) {
    starField = new StarField(canvasRef.value);
  }
});

onUnmounted(() => {
  if (starField) {
    starField.destroy();
  }
});
</script>

<style>
/*TODO 背景是根据前端设置的*/
.game-background {
  position: absolute;
  width: 100%;
  height: 100%;
  background-image: url(@/assets/images/default_bg.jpg);
  background-size: cover;
  background-position: center center;
  background-attachment: fixed;
  background-repeat: no-repeat;
  z-index: -2;
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
