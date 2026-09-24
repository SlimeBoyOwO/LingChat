<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    class="heart-liquid"
    :style="glowStyle"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
  >
    <defs>
      <clipPath :id="clipId">
        <path :d="HEART_PATH" />
      </clipPath>
      <linearGradient :id="gradId" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" :stop-color="frontTopColor" />
        <stop offset="100%" :stop-color="frontBottomColor" />
      </linearGradient>
    </defs>

    <!-- 心形容器暗色底 -->
    <path :d="HEART_PATH" fill="rgba(255, 255, 255, 0.05)" />

    <!-- 液体：后波（浅色）+ 前波（渐变）双层，心形裁剪；随窗口移动倾斜晃动。
         负面情绪峰值分档变色（红→蓝→灰→黑），颜色在 frame 循环里平滑过渡 -->
    <g :clip-path="`url(#${clipId})`" :transform="`rotate(${tiltDeg} 12 12)`">
      <path :d="backWaveD" :fill="backWaveColor" />
      <path :d="frontWaveD" :fill="`url(#${gradId})`" />
      <!-- 环境雾霾染色：整颗心随雾霾强度变浑浊（深罩在液体之上） -->
      <path :d="HEART_PATH" :fill="hazeTintColor" />
    </g>

    <!-- 描边盖在液体之上：白边，满溢（>100）发光（颜色随情绪档位） -->
    <path
      :d="HEART_PATH"
      fill="none"
      stroke="rgba(255, 255, 255, 0.9)"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
    />

    <!-- 黑色雾霾粒子：从爱心外缘冒出，向外向上飘散（不裁剪，可飞出图标边界） -->
    <circle
      v-for="(p, i) in hazeParticles"
      :key="i"
      :cx="p.x"
      :cy="p.y"
      :r="p.r"
      fill="#101016"
      :opacity="p.alpha"
    />
  </svg>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useId, watch } from "vue";
import { startFrameLoop, type FrameLoopHandle } from "@/core/animation/frame-scheduler";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";

const props = withDefaults(
  defineProps<{
    /** 好感平均值：null=无数据（空杯）；<0 冷色描边；>100 满溢发光（钳到满杯） */
    value: number | null;
    /** 负面情绪峰值（0~100+）：≥30 左右变蓝、≥60 全灰、≥100 纯黑并升起黑烟 */
    negative?: number | null;
    size?: number;
    /** 动态波浪开关（高级设置）；关闭后液面静止为平面，液位弹簧保留 */
    wave?: boolean;
  }>(),
  { size: 18, wave: true, negative: null },
);

// Lucide heart 轮廓（24x24）
const HEART_PATH =
  "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z";

// 同页多颗心共存，clip/渐变 id 必须各自唯一
const uid = useId();
const clipId = `heart-clip-${uid}`;
const gradId = `heart-grad-${uid}`;

// ── 情绪分档配色（负面峰值驱动）：[顶部色, 底部色] ──
type Rgb = [number, number, number];
const RED: Rgb[] = [
  [255, 125, 156],
  [224, 17, 60],
];
const BLUE: Rgb[] = [
  [126, 200, 255],
  [29, 78, 216],
];
const GRAY: Rgb[] = [
  [201, 201, 209],
  [85, 85, 94],
];
const BLACK: Rgb[] = [
  [74, 74, 82],
  [12, 12, 16],
];
/** [阈值, 配色]：0~20 纯红、30 起蓝、60 全灰、100 纯黑；档间线性插值 */
const STOPS: [number, Rgb[]][] = [
  [0, RED],
  [20, RED],
  [30, BLUE],
  [60, GRAY],
  [100, BLACK],
];

const mix = (a: Rgb, b: Rgb, t: number): Rgb => [
  Math.round(a[0] + (b[0] - a[0]) * t),
  Math.round(a[1] + (b[1] - a[1]) * t),
  Math.round(a[2] + (b[2] - a[2]) * t),
];
const rgbStr = (c: Rgb) => `rgb(${c[0]}, ${c[1]}, ${c[2]})`;

function paletteAt(p: number): { top: Rgb; bottom: Rgb } {
  const x = Math.max(0, p);
  for (let i = 1; i < STOPS.length; i++) {
    const [at, colors] = STOPS[i];
    if (x <= at) {
      const [prevAt, prevColors] = STOPS[i - 1];
      const t = (x - prevAt) / (at - prevAt);
      return { top: mix(prevColors[0], colors[0], t), bottom: mix(prevColors[1], colors[1], t) };
    }
  }
  return { top: BLACK[0], bottom: BLACK[1] };
}

/** 平滑后的负面峰值（frame 循环里向 props.negative 指数趋近，避免跳变） */
const moodPeak = ref(props.negative ?? 0);
const palette = computed(() => paletteAt(moodPeak.value));
const frontTopColor = computed(() => rgbStr(palette.value.top));
const frontBottomColor = computed(() => rgbStr(palette.value.bottom));
const backWaveColor = computed(() => {
  const [r, g, b] = palette.value.top;
  return `rgba(${r}, ${g}, ${b}, 0.35)`;
});

const overflow = computed(() => (props.value ?? 0) > 100);
/** 雾霾强度 0..1（ref 供雾晕样式使用）：负面峰值 ≥95 起、≥135 拉满 */
const hazeLevel = ref(0);
/** 环境雾霾染色：整颗心罩一层暗色（强度随雾霾），让黑化状态在深色背景下也可读 */
const hazeTintColor = computed(() => `rgba(10, 10, 16, ${(hazeLevel.value * 0.42).toFixed(3)})`);
const glowStyle = computed(() => {
  const filters: string[] = [];
  if (overflow.value) {
    const [r, g, b] = palette.value.bottom;
    filters.push(`drop-shadow(0 0 4px rgba(${r}, ${g}, ${b}, 0.9))`);
  }
  // 黑化雾晕：整颗心笼一层黑色阴影，配合黑烟粒子读出「雾霾」感
  if (hazeLevel.value > 0.03) {
    filters.push(`drop-shadow(0 0 6px rgba(5, 5, 8, ${(hazeLevel.value * 0.9).toFixed(2)}))`);
  }
  return filters.length
    ? { filter: filters.join(" "), overflow: "visible" }
    : { overflow: "visible" };
});

/** 目标液位 0..1 */
const targetLevel = computed(() => {
  const v = props.value;
  if (v === null) return 0;
  return Math.min(1, Math.max(0, v / 100));
});

// ── 液体物理：液位弹簧阻尼（欠阻尼 → 过冲）+ 晃动能量注入/衰减 + 双层行波 ──
const frontWaveD = ref("");
const backWaveD = ref("");
/** 液面倾斜角（度）：窗口拖动时注入角速度，弹簧回正 */
const tiltDeg = ref(0);

interface HazeParticle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  life: number;
  maxLife: number;
  alpha: number;
}
/** 雾霾粒子（普通数组即可：waveD 每帧更新本就驱动重渲染） */
const hazeParticles: HazeParticle[] = [];
let hazeTimer = 0;

let level = targetLevel.value;
let velocity = 0;
let slosh = 0; // 晃动能量 0..1：液位突变/窗口拖动注入，随时间指数衰减
let phase = 0;
let tilt = 0;
let tiltVel = 0;
let loop: FrameLoopHandle | null = null;
let unlistenMove: UnlistenFn | null = null;

const W = 24;

/** 正弦液面 + 向下闭合到底部外的填充路径（左右各外延 4px 防止晃动露边） */
function wavePath(surfaceY: number, amp: number, ph: number, waves: number): string {
  const points: string[] = [];
  for (let x = -4; x <= W + 4; x += 2) {
    const y = surfaceY + Math.sin((x / W) * Math.PI * 2 * waves + ph) * amp;
    points.push(`L ${x} ${y.toFixed(2)}`);
  }
  return `M -4 28 ${points.join(" ")} L ${W + 4} 28 Z`;
}

function frame(_now: number, delta: number) {
  // 秒为单位的帧间隔；首帧 delta 为 0，退化为 60fps 步长（与迁移前一致）
  const dt = Math.min(0.05, delta > 0 ? delta / 1000 : 0.016);

  // 弹簧阻尼积分：刚度/阻尼刻意欠阻尼，液面冲向目标位会轻微过冲再回稳
  const stiffness = 130;
  const damping = 9;
  const accel = stiffness * (targetLevel.value - level) - damping * velocity;
  velocity += accel * dt;
  level += velocity * dt;

  // 弹簧速度本身也带起晃动；相位速度随晃动能量加快（激烈时浪更急）
  slosh = Math.min(1, slosh + Math.abs(velocity) * dt * 1.5);
  slosh *= Math.exp(-2.4 * dt);
  if (props.wave) phase += dt * (2.4 + slosh * 5);

  // 倾斜角弹簧回正（欠阻尼 → 拖窗停下后液面左右摇两下再平）
  tiltVel += (-70 * tilt - 7 * tiltVel) * dt;
  tilt += tiltVel * dt;
  tiltDeg.value = props.wave ? Math.max(-14, Math.min(14, tilt)) : 0;

  // 负面峰值平滑趋近（情绪变色约 1/3s 跟上，不生硬跳变）
  moodPeak.value += ((props.negative ?? 0) - moodPeak.value) * (1 - Math.exp(-3 * dt));

  // 液面映射到心形内部（心形内容区约 y=2~21.5）：满杯盖过顶部，空杯沉到心尖以下
  const surfaceY = 22.5 - level * 24.5;
  // 波浪关闭时液面为静止平面（振幅 0），液位弹簧照常工作
  const ampFront = props.wave ? 0.9 + slosh * 1.8 : 0;
  frontWaveD.value = wavePath(surfaceY, ampFront, phase, 1.5);
  backWaveD.value = wavePath(surfaceY + 0.6, ampFront * 0.75, phase * 0.8 + 1.9, 1.2);

  // ── 黑色雾霾：负面峰值 ≥95 起，
  //    粒子从爱心顶部两瓣冒出，向上飘散（上浮加速 + 左右摆动 + 扩散）──
  const hazeTarget = Math.min(1, Math.max(0, ((props.negative ?? 0) - 95) / 40));
  hazeLevel.value += (hazeTarget - hazeLevel.value) * (1 - Math.exp(-2.5 * dt));
  const haze = hazeLevel.value;
  if (haze > 0.03) {
    hazeTimer -= dt;
    if (hazeTimer <= 0 && hazeParticles.length < 64) {
      // 心形顶部上缘随机取一点：角度取上半圆 [π, 2π]（y 轴向下，sin 为负即上方）
      const ang = Math.PI * (1 + Math.random());
      const edgeR = 9.6 + Math.random() * 1.4;
      hazeParticles.push({
        x: 12 + Math.cos(ang) * edgeR,
        y: 10 + Math.sin(ang) * edgeR * 0.85,
        vx: (Math.random() - 0.5) * 1.0,
        vy: -(1.5 + Math.random() * 1.6),
        r: 0.9 + Math.random() * 1.1,
        life: 0,
        maxLife: 2.8 + Math.random() * 1.8,
        alpha: 0,
      });
      hazeTimer = (0.018 + Math.random() * 0.03) / haze;
    }
  }
  for (let i = hazeParticles.length - 1; i >= 0; i--) {
    const p = hazeParticles[i];
    p.life += dt;
    p.x += p.vx * dt + Math.sin(p.life * 4 + i) * 0.6 * dt; // 烟气左右摇曳
    p.y += p.vy * dt;
    p.vx *= Math.exp(-0.6 * dt); // 水平漂移衰减，烟柱逐渐垂直上升
    p.vy -= 0.35 * dt; // 持续上浮加速（缓和，烟雾飘得更久）
    p.r *= 1 + 0.9 * dt; // 上升扩散
    const fadeIn = Math.min(1, p.life / 0.3);
    const fadeOut = Math.pow(Math.max(0, 1 - p.life / p.maxLife), 1.3);
    p.alpha = Math.max(0, haze * 0.55 * Math.min(fadeIn, fadeOut));
    if (p.life >= p.maxLife) hazeParticles.splice(i, 1);
  }
}

// 好感突变 → 注入晃动能量（升/降好感时液体「晃一下」）
watch(targetLevel, (next, prev) => {
  slosh = Math.min(1, slosh + Math.abs(next - prev) * 4 + 0.15);
});

onMounted(async () => {
  // 液体物理循环交给共享调度器：页面隐藏/窗口失焦时自动暂停，恢复后虚拟时钟续上，
  // 液面不会因为暂停时长而瞬移
  loop ??= startFrameLoop(frame);

  // 拖动窗口 → 液体物理：位移距离注入晃动能量，水平速度注入倾斜角速度
  try {
    let lastPos: { x: number; y: number } | null = null;
    unlistenMove = await getCurrentWindow().onMoved(({ payload: pos }) => {
      if (lastPos) {
        const dx = pos.x - lastPos.x;
        const dy = pos.y - lastPos.y;
        slosh = Math.min(1, slosh + Math.hypot(dx, dy) / 260);
        tiltVel = Math.max(-40, Math.min(40, tiltVel + dx * 0.12));
      }
      lastPos = { x: pos.x, y: pos.y };
    });
  } catch {
    /* 非 Tauri 环境（纯 web 预览）无窗口事件，忽略 */
  }
});
onUnmounted(() => {
  loop?.stop();
  loop = null;
  unlistenMove?.();
});
</script>
