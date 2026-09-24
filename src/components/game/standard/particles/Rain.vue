<template>
  <canvas id="glcanvas" class="rain-container" ref="canvasRef" />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { startFrameLoop, type FrameLoopHandle } from "@/core/animation/frame-scheduler";
import type { Drop } from "./config/rain";
import { useRain } from "./hooks/useRain";

const props = defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
  intensity: {
    type: Number,
    default: 1,
    validator: (value: number) => value >= 0 && value <= 2,
  },
});

const canvasRef = ref<HTMLCanvasElement | null>(null);

// 响应式雨滴数量
const dropCount = ref(Math.floor(50 * props.intensity));

let W = 0,
  H = 0;

let drops: Drop[] = [];

let ctx: CanvasRenderingContext2D | null = null;
/** 共享帧循环句柄（帧率上限与页面隐藏/失焦暂停由调度器统一处理） */
let loop: FrameLoopHandle | null = null;

// 帧率限制相关变量
const TARGET_FPS = 60;
const FRAME_INTERVAL = 1000 / TARGET_FPS; // 约 16.67ms

const { createDrop } = useRain();

/**
 * 处理窗口 resize，更新 Canvas 尺寸并重新初始化雨滴
 */
function handleResize() {
  if (!canvasRef.value) return;

  canvasRef.value.width = window.innerWidth;
  canvasRef.value.height = window.innerHeight;
  W = canvasRef.value.width;
  H = canvasRef.value.height;

  // 重新初始化雨滴以适应新尺寸
  drops = [];
  for (let i = 0; i < dropCount.value; i++) {
    drops.push(createDrop(W, H, props.intensity));
  }
}

function init() {
  if (!props.enabled) return;

  const canvas = canvasRef.value;
  if (!canvas) return;

  canvas.width = window.innerWidth;
  canvas.height = window.innerHeight;
  W = canvas.width;
  H = canvas.height;
  ctx = canvas.getContext("2d");

  drops = [];
  for (let i = 0; i < dropCount.value; i++) {
    drops.push(createDrop(W, H, props.intensity));
  }

  startLoop();
}

/** 启动共享帧循环（重复调用不会重复订阅） */
function startLoop() {
  // 帧率上限交给调度器；页面隐藏/窗口失焦时调度器会暂停并冻结虚拟时钟
  loop ??= startFrameLoop(frame, { fps: TARGET_FPS });
}

/** 停止共享帧循环（幂等）并清空雨滴 */
function stopLoop() {
  loop?.stop();
  loop = null;
  drops = [];
}

/**
 * 更新雨滴位置（基于固定时间步长）
 * @param elapsedMs 距离上一帧经过的时间（毫秒）
 */
function updateDrops(elapsedMs: number) {
  // 将时间转换为速度因子，保持雨滴移动速度与帧率解耦
  // 原始速度基于每帧移动 speed 像素（假设 60fps）
  // 现根据实际经过时间调整移动距离
  const speedFactor = elapsedMs / FRAME_INTERVAL;

  for (const drop of drops) {
    // 根据时间差移动雨滴
    drop.y += drop.speed * speedFactor;

    // 雨滴超出屏幕时，重置位置并重新随机化 x 坐标
    if (drop.y > H) {
      drop.y = -drop.length;
      drop.x = Math.random() * W;
    }
  }
}

function render() {
  if (!ctx) return;

  ctx.clearRect(0, 0, W, H);

  for (const drop of drops) {
    ctx.beginPath();
    ctx.moveTo(drop.x, drop.y);
    ctx.lineTo(drop.x, drop.y + drop.length);

    const gradient = ctx.createLinearGradient(drop.x, drop.y, drop.x, drop.y + drop.length);
    gradient.addColorStop(0, "rgba(255, 255, 255, 0.3)");
    gradient.addColorStop(1, "rgba(255, 255, 255, 0.7)");

    ctx.strokeStyle = gradient;
    ctx.lineWidth = 1.25;
    ctx.stroke();
  }
}

/**
 * 每帧回调：只做更新与渲染，不再自己调度下一帧
 * @param _now 虚拟时间（毫秒，页面隐藏/失焦期间不推进）；雨滴物理只依赖 delta
 * @param delta 距上一次回调经过的时间（毫秒）
 */
function frame(_now: number, delta: number) {
  if (!ctx) return;

  // 首帧（delta 为 0）与长时间暂停后的异常值都跳过/收敛，避免雨滴跳跃
  const MAX_DELTA = 100; // 最大100ms
  const elapsed = Math.min(delta, MAX_DELTA);
  if (elapsed <= 0) return;

  // 使用实际经过时间更新雨滴位置（速度因子基于基准帧间隔，保持速度与帧率解耦）
  updateDrops(elapsed);
  render();
}

// 监听 intensity 变化，动态调整雨滴数量
watch(
  () => props.intensity,
  (newIntensity) => {
    dropCount.value = Math.floor(50 * newIntensity);
    handleResize();
  },
);

// 监听 enabled 状态变化
watch(
  () => props.enabled,
  (newVal) => {
    if (newVal) {
      init();
    } else {
      stopLoop();
    }
  },
);

onMounted(() => {
  init();
  window.addEventListener("resize", handleResize);
});

onBeforeUnmount(() => {
  stopLoop();
  window.removeEventListener("resize", handleResize);
});
</script>

<style scoped>
.rain-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: -1;
  overflow: hidden;
}

.rain-item {
  position: absolute;
  display: inline-block;
  width: 2px;
  background: linear-gradient(rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.6));
}
</style>
