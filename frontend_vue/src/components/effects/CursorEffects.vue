<template>
  <div class="cursor-effects-container">
    <!-- Canvas 拖尾轨迹 -->
    <canvas ref="canvasRef" class="cursor-trail-canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";

interface TrailPoint {
  x: number;
  y: number;
  alpha: number;
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  maxLife: number;
  size: number;
  color: string;
  rotation: number;
  rotationSpeed: number;
}

// --- Canvas 引用 ---
const canvasRef = ref<HTMLCanvasElement | null>(null);
let ctx: CanvasRenderingContext2D | null = null;

// --- 拖尾效果状态 ---
const points: TrailPoint[] = [];
let maxPoints = 10; // 动态调整
const fadeSpeed = 0.05;
let animationId: number | null = null;
let isAnimating = false;
let lastMouseTime = 0;
const MOUSE_THROTTLE = 16; // ~60fps

// --- 粒子系统 ---
const particles: Particle[] = [];

// --- 性能监控 ---
let frameCount = 0;
let lastFpsTime = 0;
let currentFps = 60;

// --- 初始化 Canvas ---
const initCanvas = () => {
  const canvas = canvasRef.value;
  if (!canvas) return;

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

// --- 性能监控 ---
const monitorPerformance = () => {
  frameCount++;
  const now = performance.now();
  if (now - lastFpsTime >= 1000) {
    currentFps = frameCount;
    frameCount = 0;
    lastFpsTime = now;

    // 自动降级效果
    if (currentFps < 30) {
      maxPoints = Math.max(5, maxPoints - 1);
    } else if (currentFps > 50 && maxPoints < 15) {
      maxPoints = Math.min(15, maxPoints + 1);
    }
  }
};

// --- 绘制拖尾轨迹 ---
const drawTrail = () => {
  if (!ctx || points.length < 2) return;

  ctx.beginPath();
  ctx.moveTo(points[0].x, points[0].y);

  // 使用二次贝塞尔曲线连接点，创建平滑轨迹
  for (let i = 1; i < points.length - 1; i++) {
    const xc = (points[i].x + points[i + 1].x) * 0.5;
    const yc = (points[i].y + points[i + 1].y) * 0.5;
    ctx.quadraticCurveTo(points[i].x, points[i].y, xc, yc);
  }

  // 连接到最后一个点
  if (points.length > 1) {
    const last = points[points.length - 1];
    ctx.lineTo(last.x, last.y);
  }

  // 设置线条样式
  const avgAlpha = Math.min(1, points[points.length - 1].alpha * 1.5);
  ctx.strokeStyle = `rgba(135, 206, 250, ${avgAlpha})`;
  ctx.lineWidth = 3;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.shadowBlur = 10;
  ctx.shadowColor = "#87CEFA";

  ctx.stroke();
};

// --- 绘制粒子 ---
const drawParticles = () => {
  if (!ctx) return;

  for (let i = particles.length - 1; i >= 0; i--) {
    const p = particles[i];

    // 更新粒子
    p.x += p.vx;
    p.y += p.vy;
    p.vy += 0.05; // 轻微重力
    p.life =
      p.life - 1 / (60 * p.maxLife) < 0 ? 0 : p.life - 1 / (60 * p.maxLife); // 60fps衰减
    p.rotation += p.rotationSpeed;

    // 绘制粒子
    const alpha = p.life;
    ctx.save();
    ctx.translate(p.x, p.y);
    ctx.rotate((p.rotation * Math.PI) / 180);
    ctx.globalAlpha = alpha;

    // 绘制三角形
    ctx.beginPath();
    ctx.moveTo(0, -p.size);
    ctx.lineTo(-p.size * 0.7, p.size * 0.7);
    ctx.lineTo(p.size * 0.7, p.size * 0.7);
    ctx.closePath();
    ctx.fillStyle = p.color;
    ctx.fill();

    ctx.restore();

    // 移除死亡粒子
    if (p.life <= 0) {
      particles.splice(i, 1);
    }
  }
};

// --- 更新点透明度 ---
const updatePoints = () => {
  // 批量更新透明度
  for (let i = points.length - 1; i >= 0; i--) {
    points[i].alpha -= fadeSpeed;
    if (points[i].alpha <= 0) {
      points.splice(i, 1);
    }
  }

  // 限制最大点数
  if (points.length > maxPoints) {
    points.splice(0, points.length - maxPoints);
  }
};

// --- 主绘制循环 ---
const draw = () => {
  monitorPerformance();

  if (!ctx || !canvasRef.value) {
    stopAnimation();
    return;
  }

  const canvas = canvasRef.value;
  const dpr = window.devicePixelRatio || 1;

  // 清除画布
  ctx.clearRect(0, 0, canvas.width / dpr, canvas.height / dpr);

  let shouldContinue = false;

  // 绘制拖尾
  if (points.length >= 2) {
    drawTrail();
    shouldContinue = shouldContinue || points.length > 0;
  }

  // 绘制粒子
  if (particles.length > 0) {
    drawParticles();
    shouldContinue = shouldContinue || particles.length > 0;
  }

  // 更新点状态
  updatePoints();

  // 条件性继续动画
  if (shouldContinue) {
    animationId = requestAnimationFrame(draw);
  } else {
    stopAnimation();
  }
};

// --- 停止动画 ---
const stopAnimation = () => {
  if (animationId) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }
  isAnimating = false;
};

// --- 节流的鼠标移动处理 ---
const handleMouseMove = (e: MouseEvent) => {
  const now = Date.now();
  if (now - lastMouseTime < MOUSE_THROTTLE) {
    return;
  }
  lastMouseTime = now;

  points.push({
    x: e.clientX,
    y: e.clientY,
    alpha: 1.0,
  });

  // 启动动画循环（如果未运行）
  if (!isAnimating && animationId === null) {
    isAnimating = true;
    animationId = requestAnimationFrame(draw);
  }

  // 限制点数
  if (points.length > maxPoints * 1.5) {
    points.splice(0, points.length - maxPoints);
  }
};

// --- 优化的点击效果（使用Canvas替代DOM）---
const handleClick = (e: MouseEvent) => {
  const particleCount = 12;
  const colors = ["#FFC0CB", "#87CEFA"];

  for (let i = 0; i < particleCount; i++) {
    const angle = Math.random() * Math.PI * 2;
    const speed = Math.random() * 2 + 1;
    const life = Math.random() * 0.8 + 0.7;

    particles.push({
      x: e.clientX,
      y: e.clientY,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      life: 1.0,
      maxLife: life,
      size: Math.random() * 8 + 4,
      color: colors[Math.floor(Math.random() * colors.length)],
      rotation: Math.random() * 360,
      rotationSpeed: (Math.random() - 0.5) * 8,
    });
  }

  // 确保动画运行
  if (!isAnimating && animationId === null) {
    isAnimating = true;
    animationId = requestAnimationFrame(draw);
  }
};

// --- 窗口大小变化处理 ---
const handleResize = () => {
  initCanvas();
};

// --- 生命周期钩子 ---
onMounted(() => {
  initCanvas();
  window.addEventListener("mousemove", handleMouseMove);
  window.addEventListener("click", handleClick);
  window.addEventListener("resize", handleResize);
});

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", handleMouseMove);
  window.removeEventListener("click", handleClick);
  window.removeEventListener("resize", handleResize);
  stopAnimation();
  points.length = 0;
  particles.length = 0;
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
