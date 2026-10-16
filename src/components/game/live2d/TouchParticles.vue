<template>
  <canvas ref="canvasRef" class="pointer-events-none absolute inset-0 z-1 h-full w-full"></canvas>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";

/**
 * 抚摸时从指尖附近飘起的淡粉小点与爱心。
 *
 * 与 standard/particles 下那些不同，它没有常驻粒子群：没有存活粒子就不跑 rAF，
 * 所以只在抚摸期间有开销，平时完全空转。
 */

interface Mote {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  life: number;
  total: number;
  heart: boolean;
  color: string;
  phase: number;
  phaseSpeed: number;
}

/** 粉得发白的一小撮，靠低透明度做「淡淡的」观感，不要做成礼花 */
const COLORS = ["#ffb3c8", "#ff9dbb", "#ffc8da", "#ff8fb0"];
/** 单颗最亮时的透明度 */
const MAX_ALPHA = 0.45;
/** 同时存活上限，长时间抚摸也不至于堆爆画布 */
const MAX_MOTES = 60;
/** 从触摸点散开的半径，像素 */
const SPREAD_RADIUS = 9;
/** 存活时长区间，毫秒 */
const MIN_LIFE = 700;
const MAX_LIFE = 1500;
/** 上飘速度区间，像素每毫秒 */
const MIN_RISE = 0.018;
const MAX_RISE = 0.055;

const canvasRef = ref<HTMLCanvasElement | null>(null);
const motes: Mote[] = [];
let frameId = 0;
let lastFrameAt = 0;
let running = false;

/** 画布尺寸取自自身而不是窗口，所以在主界面与任意容器里都成立。
    放在这里而不是 onMounted，是因为挂载瞬间父容器未必已经有尺寸。 */
function ensureCanvasSize(canvas: HTMLCanvasElement) {
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  if (width > 0 && height > 0 && (canvas.width !== width || canvas.height !== height)) {
    canvas.width = width;
    canvas.height = height;
  }
}

/** 在给定的视口坐标附近撒一颗。坐标换算在这里做，调用方不必关心布局。 */
function spawn(clientX: number, clientY: number) {
  const canvas = canvasRef.value;
  if (!canvas || motes.length >= MAX_MOTES) return;
  ensureCanvasSize(canvas);
  const rect = canvas.getBoundingClientRect();
  const angle = Math.random() * Math.PI * 2;
  const radius = Math.random() * SPREAD_RADIUS;
  const total = MIN_LIFE + Math.random() * (MAX_LIFE - MIN_LIFE);
  motes.push({
    x: clientX - rect.left + Math.cos(angle) * radius,
    y: clientY - rect.top + Math.sin(angle) * radius,
    vx: (Math.random() - 0.5) * 0.02,
    vy: -(MIN_RISE + Math.random() * (MAX_RISE - MIN_RISE)),
    size: 3 + Math.random() * 3.5,
    life: total,
    total,
    // 爱心占比不高，偶尔冒一颗才像点缀而不是喷泉
    heart: Math.random() < 0.3,
    color: COLORS[Math.floor(Math.random() * COLORS.length)]!,
    phase: Math.random() * Math.PI * 2,
    phaseSpeed: 0.0015 + Math.random() * 0.002,
  });
  startLoop();
}

/** 心形：底部收成尖，上面两个圆瓣，中间留一个不大的凹口。
    尺寸都压在几像素内，所以形状只需要「看着像」，不必精确。 */
function drawHeart(ctx: CanvasRenderingContext2D, x: number, y: number, size: number) {
  ctx.beginPath();
  ctx.moveTo(x, y + size);
  ctx.bezierCurveTo(x - size * 1.4, y + size * 0.2, x - size, y - size * 0.9, x, y - size * 0.45);
  ctx.bezierCurveTo(x + size, y - size * 0.9, x + size * 1.4, y + size * 0.2, x, y + size);
  ctx.closePath();
  ctx.fill();
}

function step(now: number) {
  const canvas = canvasRef.value;
  const ctx = canvas?.getContext("2d");
  if (!canvas || !ctx) {
    stopLoop();
    return;
  }
  // 夹住 dt：标签页切回来时一次积分过头会让粒子瞬移出画布
  const dt = lastFrameAt ? Math.min(48, now - lastFrameAt) : 16;
  lastFrameAt = now;
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  for (let index = motes.length - 1; index >= 0; index -= 1) {
    const mote = motes[index]!;
    mote.life -= dt;
    if (mote.life <= 0) {
      motes.splice(index, 1);
      continue;
    }
    mote.phase += mote.phaseSpeed * dt;
    // 横向叠一点正弦，飘上去的轨迹才不是直线
    mote.x += (mote.vx + Math.sin(mote.phase) * 0.012) * dt;
    mote.y += mote.vy * dt;

    const progress = 1 - mote.life / mote.total;
    const fadeIn = Math.min(1, progress / 0.15);
    const fadeOut = Math.min(1, (1 - progress) / 0.45);
    ctx.globalAlpha = MAX_ALPHA * fadeIn * fadeOut;
    ctx.fillStyle = mote.color;
    const size = mote.size * (1 - 0.3 * progress);
    if (mote.heart) {
      drawHeart(ctx, mote.x, mote.y, size);
    } else {
      ctx.beginPath();
      ctx.arc(mote.x, mote.y, size * 0.55, 0, Math.PI * 2);
      ctx.fill();
    }
  }
  ctx.globalAlpha = 1;

  if (motes.length) frameId = requestAnimationFrame(step);
  else stopLoop();
}

function startLoop() {
  if (running) return;
  running = true;
  lastFrameAt = 0;
  frameId = requestAnimationFrame(step);
}

function stopLoop() {
  running = false;
  if (frameId) cancelAnimationFrame(frameId);
  frameId = 0;
}

onBeforeUnmount(stopLoop);

defineExpose({ spawn });
</script>
