<template>
  <canvas ref="canvasRef" class="w-full h-full pointer-events-none"></canvas>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";

const props = defineProps({
  particleCount: { type: Number, default: 35 }, // 数量适当，不显得凌乱
  speed: { type: Number, default: 0.6 },
});

const canvasRef = ref<HTMLCanvasElement | null>(null);
let animationFrameId: number;
let particles: Particle[] = [];
let width = 0;
let height = 0;

// BA 风格配色：青蓝、亮蓝、纯白
const colors = ["#00BFFF", "#87CEFA", "#FFFFFF", "#E0F7FA"];
const types = ["circle", "circle", "cross"]; // 保留特色十字星

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
    // 如果是初始化，让粒子随机分布在全屏；如果是重生，则固定在顶部上方
    this.y = isInit ? Math.random() * height : -(Math.random() * 20 + 10);
    this.size = Math.random() * 1.5 + 0.8;
    // Y轴速度向下(正数)，X轴极轻微漂移
    this.speedY = (Math.random() * 0.4 + 0.3) * props.speed;
    this.speedX = (Math.random() - 0.5) * 0.15 * props.speed;
    this.color = colors[Math.floor(Math.random() * colors.length)];
    this.type = types[Math.floor(Math.random() * types.length)];
    this.opacity = isInit ? Math.random() * 0.6 : 0;
    this.maxOpacity = Math.random() * 0.5 + 0.3; // 限制最大透明度，避免刺眼
  }

  update() {
    this.x += this.speedX;
    this.y += this.speedY;

    // 根据粒子所在的 Y 轴位置计算透明度，彻底拒绝闪烁
    // 0~15% 高度：平滑淡入
    // 15%~80% 高度：保持常亮
    // 80%~100% 高度：平滑淡出
    const progress = this.y / height;

    if (progress < 0.15) {
      this.opacity = (progress / 0.15) * this.maxOpacity;
    } else if (progress > 0.8) {
      this.opacity = ((1 - progress) / 0.2) * this.maxOpacity;
    } else {
      this.opacity = this.maxOpacity;
    }

    // 确保透明度在合法范围内
    this.opacity = Math.max(0, Math.min(this.maxOpacity, this.opacity));

    // 如果粒子完全掉出底部边界，重置到顶部
    if (this.y > height + 10) {
      this.reset();
    }
  }

  draw(ctx: CanvasRenderingContext2D) {
    ctx.save();
    ctx.globalAlpha = this.opacity;
    ctx.fillStyle = this.color;
    ctx.strokeStyle = this.color;

    if (this.type === "circle") {
      ctx.beginPath();
      ctx.arc(this.x, this.y, this.size, 0, Math.PI * 2);
      ctx.fill();
    } else if (this.type === "cross") {
      // 绘制精致的科技十字星
      const s = this.size * 1.2;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(this.x - s, this.y);
      ctx.lineTo(this.x + s, this.y);
      ctx.moveTo(this.x, this.y - s);
      ctx.lineTo(this.x, this.y + s);
      ctx.stroke();
    }

    ctx.restore();
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
  if (!canvasRef.value) return;
  const ctx = canvasRef.value.getContext("2d");
  if (!ctx) return;

  // 清空画布 (透明背景)
  ctx.clearRect(0, 0, width, height);

  particles.forEach((p) => {
    p.update();
    p.draw(ctx);
  });

  animationFrameId = requestAnimationFrame(loop);
};

onMounted(() => {
  resizeCanvas();
  window.addEventListener("resize", resizeCanvas);

  for (let i = 0; i < props.particleCount; i++) {
    particles.push(new Particle());
  }

  loop();
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", resizeCanvas);
  cancelAnimationFrame(animationFrameId);
});
</script>
