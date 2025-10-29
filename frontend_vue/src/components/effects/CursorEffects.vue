<template>
  <div class="cursor-effects-container">
    <svg class="cursor-effects-svg" ref="svgContainer"></svg>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";

interface Line {
  element: SVGLineElement;
  life: number;
}

interface Point {
  x: number;
  y: number;
}

// --- SVG 容器引用 ---
const svgContainer = ref<SVGSVGElement | null>(null);

// --- 拖尾效果状态 ---
const trailLines: Line[] = [];
let lastAveragePosition: Point | null = null;
let animationFrameId: number;

// --- 拖尾效果逻辑 ---
const createTrailLine = (p1: Point, p2: Point) => {
  if (!svgContainer.value) return;

  const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
  line.setAttribute("x1", p1.x.toString());
  line.setAttribute("y1", p1.y.toString());
  line.setAttribute("x2", p2.x.toString());
  line.setAttribute("y2", p2.y.toString());
  line.setAttribute("stroke", "#87CEFA");
  line.setAttribute("stroke-width", "3");
  line.setAttribute("stroke-linecap", "round");

  svgContainer.value.appendChild(line);

  trailLines.push({
    element: line,
    life: 1.0,
  });
};

const updateTrail = () => {
  for (let i = trailLines.length - 1; i >= 0; i--) {
    const line = trailLines[i];
    line.life -= 0.02; // 调整衰减速度

    if (line.life <= 0) {
      line.element.remove();
      trailLines.splice(i, 1);
    } else {
      line.element.setAttribute("stroke-opacity", line.life.toString());
    }
  }
  animationFrameId = requestAnimationFrame(updateTrail);
};

const handleMouseMove = (e: MouseEvent) => {
  const currentPosition: Point = { x: e.clientX, y: e.clientY };

  if (lastAveragePosition) {
    createTrailLine(lastAveragePosition, currentPosition);
  }

  lastAveragePosition = currentPosition;
};

// --- 点击效果逻辑 (保持不变) ---
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
    const opacity = ((size - 5) / 15) * 0.6 + 0.4; // 不透明度

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

    setTimeout(() => {
      particle.remove();
    }, 1000);
  }
};

// --- 生命周期钩子 ---
onMounted(() => {
  window.addEventListener("mousemove", handleMouseMove);
  window.addEventListener("click", handleClick);
  animationFrameId = requestAnimationFrame(updateTrail);
});

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", handleMouseMove);
  window.removeEventListener("click", handleClick);
  cancelAnimationFrame(animationFrameId);
  trailLines.forEach((line) => line.element.remove());
  trailLines.length = 0;
});
</script>

<style>
/* SVG 拖尾样式 */
.cursor-effects-svg {
  filter: drop-shadow(0 0 2px #87cefa) drop-shadow(0 0 5px #87cefa);
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 9999;
}

/* 点击样式 (保持不变) */
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
