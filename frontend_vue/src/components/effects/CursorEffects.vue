<template>
  <div class="cursor-effects-container"></div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount } from "vue";

interface Particle {
  element: HTMLElement;
  life: number;
}

interface Point {
  x: number;
  y: number;
}

// --- 拖尾效果状态 ---
const trailParticles: Particle[] = [];
const MAX_TRAIL_PARTICLES = 250;

let mouseHistory: Point[] = []; // 存储最近的鼠标位置
const MOUSE_HISTORY_SIZE = 5; // 取5个点进行平均

let lastAveragePosition: Point | null = null;
let animationFrameId: number;

// --- 拖尾效果逻辑 ---
const createTrailParticle = (x: number, y: number) => {
  if (trailParticles.length >= MAX_TRAIL_PARTICLES) {
    const oldestParticle = trailParticles.shift();
    if (oldestParticle) {
      oldestParticle.element.remove();
    }
  }

  const element = document.createElement("div");
  element.className = "cursor-line-particle";
  element.style.left = `${x}px`;
  element.style.top = `${y}px`;
  document.body.appendChild(element);

  trailParticles.push({
    element,
    life: 1.0,
  });
};

const updateTrailParticles = () => {
  for (let i = trailParticles.length - 1; i >= 0; i--) {
    const p = trailParticles[i];
    p.life -= 0.01;
    p.life /= 1.08;

    if (p.life <= 0) {
      p.element.remove();
      trailParticles.splice(i, 1);
    } else {
      p.element.style.opacity = p.life.toString();
      p.element.style.transform = `translate(-50%, -50%) scale(${p.life})`;
    }
  }
  animationFrameId = requestAnimationFrame(updateTrailParticles);
};

const getAveragePosition = (positions: Point[]): Point => {
  if (positions.length === 0) return { x: 0, y: 0 };

  const sum = positions.reduce(
    (acc, pos) => {
      return { x: acc.x + pos.x, y: acc.y + pos.y };
    },
    { x: 0, y: 0 }
  );

  return {
    x: sum.x / positions.length,
    y: sum.y / positions.length,
  };
};

const handleMouseMove = (e: MouseEvent) => {
  const currentMousePosition: Point = { x: e.clientX, y: e.clientY };

  mouseHistory.push(currentMousePosition);
  if (mouseHistory.length > MOUSE_HISTORY_SIZE) {
    mouseHistory.shift();
  }

  const currentAverage = getAveragePosition(mouseHistory);

  if (lastAveragePosition) {
    const dx = currentAverage.x - lastAveragePosition.x;
    const dy = currentAverage.y - lastAveragePosition.y;
    const distance = Math.sqrt(dx * dx + dy * dy);

    // 只在距离足够大时生成粒子，避免密集生成
    if (distance > 2) {
      const particleCount = Math.floor(distance / 1);

      for (let i = 0; i < particleCount; i++) {
        const ratio = i / particleCount;
        const x = lastAveragePosition.x + dx * ratio;
        const y = lastAveragePosition.y + dy * ratio;
        createTrailParticle(x, y);
      }
    }

    createTrailParticle(currentAverage.x, currentAverage.y);
  } else {
    createTrailParticle(currentAverage.x, currentAverage.y);
  }

  lastAveragePosition = currentAverage;
};

// --- 点击效果逻辑 ---
const handleClick = (e: MouseEvent) => {
  const x = e.clientX;
  const y = e.clientY;
  const particleCount = 12;
  const colors = ["#FFC0CB", "#87CEFA"]; // 颜色

  for (let i = 0; i < particleCount; i++) {
    const particle = document.createElement("div");
    particle.className = "click-triangle-particle";

    const size = Math.random() * 15 + 5; // 大小
    const color = colors[Math.floor(Math.random() * colors.length)];

    // 将不透明度与大小关联
    const opacity = ((size - 5) / 15) * 0.6 + 0.4; // 不透明度

    // 使用CSS变量将随机值传递给动画
    particle.style.setProperty("--triangle-size", `${size}px`);
    particle.style.setProperty("--triangle-color", color);

    const angle = Math.random() * Math.PI * 2;
    const distance = Math.random() * 60 + 30; // 移动距离
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

    // 移除粒子
    setTimeout(() => {
      particle.remove();
    }, 1000);
  }
};

// --- 生命周期钩子 ---
onMounted(() => {
  window.addEventListener("mousemove", handleMouseMove);
  window.addEventListener("click", handleClick);
  animationFrameId = requestAnimationFrame(updateTrailParticles);
});

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", handleMouseMove);
  window.removeEventListener("click", handleClick);
  cancelAnimationFrame(animationFrameId);
  trailParticles.forEach((p) => p.element.remove());
  trailParticles.length = 0;
  mouseHistory.length = 0;
});
</script>

<style>
/* 拖尾样式 */
.cursor-line-particle {
  position: fixed;
  left: 0;
  top: 0;
  width: 4px;
  height: 4px;
  border-radius: 2px;
  background-color: #87cefa;
  box-shadow: 0 0 4px #87cefa, 0 0 8px #87cefa;
  pointer-events: none;
  z-index: 9999;
  transform-origin: center center;
  transform: translate(-50%, -50%);
}

/* 点击样式 */
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
