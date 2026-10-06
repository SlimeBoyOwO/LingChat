<template>
  <canvas ref="canvasRef" class="pointer-events-none h-full w-full"></canvas>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, type PropType } from "vue";

/**
 * 星辉：蓝色细点自上而下飘落，上下两端淡入淡出（原桌宠 BAParticles）。
 *
 * 画布尺寸取自**父元素**而非窗口，所以在主界面能铺满整屏，
 * 塞进桌宠的小圆头像里也照常工作 —— 这是它能两边通用的关键。
 *
 * 本轮的改动只针对每帧的绘制开销：不再逐粒子 save/restore、不再逐帧取上下文、
 * 只设置该粒子真正用到的样式，隐藏时停掉 rAF。粒子数量、速度、颜色、
 * 淡入淡出曲线与形状比例一概未动，所以观感与原先完全一致。
 */
const props = defineProps({
  enabled: { type: Boolean, default: true },
  particleCount: { type: Number, default: 35 },
  speed: { type: Number, default: 0.6 },
  colors: {
    type: Array as PropType<string[]>,
    default: () => ["#00BFFF", "#87CEFA", "#FFFFFF", "#E0F7FA"],
  },
});

const canvasRef = ref<HTMLCanvasElement | null>(null);
/** 上下文只取一次。原先在 rAF 回调里每帧 getContext */
let ctx: CanvasRenderingContext2D | null = null;
let animationFrameId: number;
let particles: Particle[] = [];
let width = 0;
let height = 0;
let running = false;
let observer: ResizeObserver | null = null;

// 圆形与十字两种形态，圆形权重更高
const types = ["circle", "circle", "cross"];

class Particle {
  x: number = 0;
  y: number = 0;
  size: number = 0;
  speedX: number = 0;
  speedY: number = 0;
  color: string = "";
  type: string = "";
  opacity: number = 0;
  maxOpacity: number = 0;

  constructor() {
    this.reset(true);
  }

  reset(isInit = false) {
    this.x = Math.random() * width;
    this.y = isInit ? Math.random() * height : -(Math.random() * 20 + 10);
    this.size = Math.random() * 1.5 + 0.8;
    this.speedY = (Math.random() * 0.4 + 0.3) * props.speed;
    this.speedX = (Math.random() - 0.5) * 0.15 * props.speed;
    this.color = props.colors[Math.floor(Math.random() * props.colors.length)]!;
    this.type = types[Math.floor(Math.random() * types.length)]!;
    this.opacity = isInit ? Math.random() * 0.6 : 0;
    this.maxOpacity = Math.random() * 0.5 + 0.3;
  }

  update() {
    this.x += this.speedX;
    this.y += this.speedY;

    const progress = this.y / height;

    if (progress < 0.15) {
      this.opacity = (progress / 0.15) * this.maxOpacity;
    } else if (progress > 0.8) {
      this.opacity = ((1 - progress) / 0.2) * this.maxOpacity;
    } else {
      this.opacity = this.maxOpacity;
    }

    this.opacity = Math.max(0, Math.min(this.maxOpacity, this.opacity));

    if (this.y > height + 10) {
      this.reset();
    }
  }

  /**
   * 不用 save/restore：转义只需在帧末复原，而这里改动的状态只有 globalAlpha
   * 与一个样式，逐粒子压栈弹栈纯属白费。同理只设置该形态真正用到的样式
   * （圆形不描边、十字不填充），省掉一半样式赋值。
   */
  draw(ctx: CanvasRenderingContext2D) {
    ctx.globalAlpha = this.opacity;

    if (this.type === "circle") {
      ctx.fillStyle = this.color;
      ctx.beginPath();
      ctx.arc(this.x, this.y, this.size, 0, Math.PI * 2);
      ctx.fill();
    } else {
      ctx.strokeStyle = this.color;
      const s = this.size * 1.2;
      ctx.beginPath();
      ctx.moveTo(this.x - s, this.y);
      ctx.lineTo(this.x + s, this.y);
      ctx.moveTo(this.x, this.y - s);
      ctx.lineTo(this.x, this.y + s);
      ctx.stroke();
    }
  }
}

const resizeCanvas = () => {
  if (!canvasRef.value) return;
  const parent = canvasRef.value.parentElement;
  if (parent) {
    width = parent.clientWidth;
    height = parent.clientHeight;
    canvasRef.value.width = width;
    canvasRef.value.height = height;
  }
};

const loop = () => {
  if (!running || !ctx) return;

  ctx.clearRect(0, 0, width, height);
  // 十字的线宽是常量，原来逐粒子赋一次，这里每帧赋一次即可
  ctx.lineWidth = 1;

  for (let i = 0; i < particles.length; i++) {
    const p = particles[i]!;
    p.update();
    p.draw(ctx);
  }

  ctx.globalAlpha = 1;
  animationFrameId = requestAnimationFrame(loop);
};

const startLoop = () => {
  if (running) return;
  running = true;
  loop();
};

const stopLoop = () => {
  running = false;
  cancelAnimationFrame(animationFrameId);
};

const onVisibilityChange = () => {
  if (document.hidden) stopLoop();
  else if (props.enabled) startLoop();
};

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;
  ctx = canvas.getContext("2d");

  resizeCanvas();
  // 观察父元素而不是窗口：主界面里对话框展开、窗口分屏都会改变可用区域
  const host = canvas.parentElement;
  if (host && typeof ResizeObserver !== "undefined") {
    observer = new ResizeObserver(resizeCanvas);
    observer.observe(host);
  } else {
    window.addEventListener("resize", resizeCanvas);
  }
  document.addEventListener("visibilitychange", onVisibilityChange);

  for (let i = 0; i < props.particleCount; i++) {
    particles.push(new Particle());
  }

  if (props.enabled) startLoop();
});

// 关闭时同时停掉 rAF：设置页切换特效只是换组件，但桌宠窗口的粒子是常驻的
watch(
  () => props.enabled,
  (on) => {
    if (on && !document.hidden) {
      resizeCanvas();
      startLoop();
    } else {
      stopLoop();
    }
  },
);

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
  window.removeEventListener("resize", resizeCanvas);
  document.removeEventListener("visibilitychange", onVisibilityChange);
  stopLoop();
});
</script>
