<template>
  <div class="cursor-effects-container">
    <!-- Canvas 拖尾轨迹 -->
    <canvas 
      ref="canvasRef" 
      class="cursor-trail-canvas"
    ></canvas>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";

interface TrailPoint {
  x: number;
  y: number;
  alpha: number;
}

// --- Canvas 引用 ---
const canvasRef = ref<HTMLCanvasElement | null>(null);
let ctx: CanvasRenderingContext2D | null = null;

// --- 拖尾效果状态 ---
const points: TrailPoint[] = [];
const maxPoints = 10; // 保留最近10个点
const fadeSpeed = 0.05; // 透明度衰减速度
let animationId: number;

// --- 初始化 Canvas ---
const initCanvas = () => {
  const canvas = canvasRef.value;
  if (!canvas) return;

  // 设置 Canvas 尺寸为窗口大小（考虑设备像素比）
  const dpr = window.devicePixelRatio || 1;
  canvas.width = window.innerWidth * dpr;
  canvas.height = window.innerHeight * dpr;
  canvas.style.width = `${window.innerWidth}px`;
  canvas.style.height = `${window.innerHeight}px`;

  ctx = canvas.getContext("2d");
  if (ctx) {
    ctx.scale(dpr, dpr);
  }
};

// --- 绘制平滑曲线拖尾 ---
const draw = () => {
  if (!ctx || !canvasRef.value) return;

  const canvas = canvasRef.value;
  const dpr = window.devicePixelRatio || 1;

  // 清除画布
  ctx.clearRect(0, 0, canvas.width / dpr, canvas.height / dpr);

  if (points.length < 2) {
    animationId = requestAnimationFrame(draw);
    return;
  }

  // 开始绘制路径
  ctx.beginPath();
  ctx.moveTo(points[0].x, points[0].y);

  // 使用二次贝塞尔曲线连接点，创建平滑轨迹
  for (let i = 1; i < points.length - 1; i++) {
    // 计算控制点（当前点和下一个点的中点）
    const xc = (points[i].x + points[i + 1].x) / 2;
    const yc = (points[i].y + points[i + 1].y) / 2;

    // 绘制二次贝塞尔曲线
    ctx.quadraticCurveTo(points[i].x, points[i].y, xc, yc);
  }

  // 连接到最后一个点
  if (points.length > 1) {
    const last = points[points.length - 1];
    ctx.lineTo(last.x, last.y);
  }

  // 设置线条样式
  const avgAlpha = points.reduce((sum, p) => sum + p.alpha, 0) / points.length;
  ctx.strokeStyle = `rgba(135, 206, 250, ${avgAlpha})`;
  ctx.lineWidth = 3;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  // 添加发光效果
  ctx.shadowBlur = 10;
  ctx.shadowColor = "#87CEFA";

  ctx.stroke();

  // 更新所有点的透明度
  for (let i = 0; i < points.length; i++) {
    points[i].alpha -= fadeSpeed;
  }

  // 移除完全透明的点
  while (points.length > 0 && points[0].alpha <= 0) {
    points.shift();
  }

  animationId = requestAnimationFrame(draw);
};

// --- 鼠标移动事件处理 ---
const handleMouseMove = (e: MouseEvent) => {
  // 添加新的轨迹点
  points.push({
    x: e.clientX,
    y: e.clientY,
    alpha: 1.0,
  });

  // 限制点数，防止无限增长
  if (points.length > maxPoints) {
    points.shift();
  }
};

// --- 窗口大小变化处理 ---
const handleResize = () => {
  initCanvas();
};

// --- 点击效果逻辑 ---
const handleClick = (e: MouseEvent) => {
  const x = e.clientX;
  const y = e.clientY;
  const particleCount = 12;
  const colors = ["#FFC0CB", "#87CEFA"];

  for (let i = 0; i < particleCount; i++) {
    const particle = document.createElement("div");
    particle.className = "click-triangle-particle";

    const size = Math.random() * 15 + 5;
    const color = colors[Math.floor(Math.random() * colors.length)];
    const opacity = ((size - 5) / 15) * 0.6 + 0.4;

    particle.style.setProperty("--triangle-size", `${size}px`);
    particle.style.setProperty("--triangle-color", color);

    const angle = Math.random() * Math.PI * 2;
    const distance = Math.random() * 60 + 30;
    particle.style.setProperty(
      "--translate-x",
      `${Math.cos(angle) * distance}px`
    );
    particle.style.setProperty(
      "--translate-y",
      `${Math.sin(angle) * distance}px`
    );
    particle.style.setProperty(
      "--initial-rotation",
      `${Math.random() * 360}deg`
    );
    particle.style.setProperty(
      "--final-rotation",
      `${Math.random() * 360 + 180}deg`
    );

    particle.style.left = `${x}px`;
    particle.style.top = `${y}px`;
    particle.style.opacity = opacity.toString();

    document.body.appendChild(particle);

    setTimeout(() => {
      particle.remove();
    }, 1000);
  }
};

// --- 生命周期钩子 ---
onMounted(() => {
  initCanvas();
  window.addEventListener("mousemove", handleMouseMove);
  window.addEventListener("click", handleClick);
  window.addEventListener("resize", handleResize);
  animationId = requestAnimationFrame(draw);
});

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", handleMouseMove);
  window.removeEventListener("click", handleClick);
  window.removeEventListener("resize", handleResize);
  cancelAnimationFrame(animationId);
  points.length = 0;
});
</script>

<style scoped>
/* Canvas 拖尾样式 */
.cursor-trail-canvas {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 9999;
}
</style>

<style>
/* 点击三角粒子样式（全局样式，因为是动态创建的元素） */
.click-triangle-particle {
  position: fixed;
  pointer-events: none;
  z-index: 10000;
  width: 0;
  height: 0;
  border-left: var(--triangle-size) solid transparent;
  border-right: var(--triangle-size) solid transparent;
  border-bottom: calc(var(--triangle-size) * 1.5) solid var(--triangle-color);
  animation: click-burst-animation 1s forwards;
}

@keyframes click-burst-animation {
  from {
    transform: translate(-50%, -50%) rotate(var(--initial-rotation)) scale(1);
    opacity: inherit;
  }
  to {
    transform: translate(
        calc(-50% + var(--translate-x)),
        calc(-50% + var(--translate-y))
      )
      rotate(var(--final-rotation)) scale(0);
    opacity: 0;
  }
}
</style>
