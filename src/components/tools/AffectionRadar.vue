<template>
  <div class="flex min-w-0 flex-1 flex-col">
    <div class="text-center text-xs font-semibold" :style="{ color: palette.stroke }">
      {{ title }}
    </div>
    <svg class="mt-1 block w-full" viewBox="0 0 400 360">
      <defs>
        <linearGradient :id="gradientId" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" :stop-color="palette.gradientFrom" />
          <stop offset="100%" :stop-color="palette.gradientTo" />
        </linearGradient>
      </defs>

      <!-- 参考环（25/50/75 虚线 + 100 满刻度淡色实线） -->
      <polygon
        v-for="ring in [0.25, 0.5, 0.75]"
        :key="ring"
        class="fill-none stroke-white/10"
        stroke-dasharray="4 4"
        :points="ringPoints(ring)"
      />
      <polygon class="fill-none stroke-white/15" :points="ringPoints(1)" />
      <!-- 轴线 -->
      <line
        v-for="i in 6"
        :key="`axis-${i}`"
        class="stroke-white/10"
        :x1="CX"
        :y1="CY"
        :x2="pointAt(i - 1, R).x"
        :y2="pointAt(i - 1, R).y"
      />

      <!-- 数据多边形 -->
      <polygon
        class="radar-data-polygon"
        :points="dataPolygonPoints"
        :fill="`url(#${gradientId})`"
        :stroke="palette.stroke"
      />

      <!-- 顶点圆点（>100 满溢：外层脉冲高亮光晕；<0 冷色） -->
      <g v-for="(p, i) in dataPoints" :key="`vertex-${i}`">
        <circle
          v-if="displayValues[i] > 100"
          :cx="p.x"
          :cy="p.y"
          r="6"
          class="radar-vertex-halo"
          :style="{ fill: palette.halo }"
        />
        <circle :cx="p.x" :cy="p.y" r="3.5" :style="{ fill: vertexColor(displayValues[i]) }" />
        <!-- 加宽 hover 热区 -->
        <circle
          :cx="p.x"
          :cy="p.y"
          r="12"
          class="cursor-pointer fill-transparent"
          @mouseenter="hovered = i"
          @mouseleave="hovered = null"
        />
      </g>

      <!-- 轴端标注：维度名 + 真实数值（可溢出/为负） -->
      <text
        v-for="(label, i) in axisLabels"
        :key="`label-${i}`"
        :x="label.x"
        :y="label.y"
        :text-anchor="label.anchor"
        class="cursor-default fill-white/60 text-[11px]"
        @mouseenter="hovered = i"
        @mouseleave="hovered = null"
      >
        {{ labels[i] }}
        <tspan
          :x="label.x"
          dy="14"
          class="text-[12px] font-bold"
          :style="valueStyle(displayValues[i])"
        >
          {{ Math.round(displayValues[i]) }}
        </tspan>
      </text>
    </svg>

    <!-- hover 顶点/标注时显示维度说明 -->
    <div class="h-4 text-center text-xs text-white/50">
      <template v-if="hovered !== null">{{ labels[hovered] }}：{{ descs[hovered] }}</template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, useId, watch } from "vue";

/** 雷达图配色方案（好感粉色系 / 负面暗红暗紫系各传一份） */
export interface RadarPalette {
  gradientFrom: string;
  gradientTo: string;
  stroke: string;
  /** 正常顶点/数值色 */
  vertex: string;
  /** >100 满溢顶点/数值色 */
  vertexOverflow: string;
  /** 满溢顶点的脉冲光晕（rgba） */
  halo: string;
  /** 满溢数值的 drop-shadow 光晕（rgba） */
  glow: string;
  /** <0 负值冷色 */
  cold: string;
}

const props = defineProps<{
  /** 雷达标题（好感维度 / 负面情绪） */
  title: string;
  /** 六维当前值（可溢出/为负；数组变化时触发补间动画） */
  values: number[];
  /** 六维显示名（已 i18n） */
  labels: string[];
  /** 六维 hover 说明（已 i18n） */
  descs: string[];
  palette: RadarPalette;
}>();

// 同页两张雷达共存，渐变 id 必须各自唯一
const gradientId = `radar-gradient-${useId()}`;

// ── 几何（100 满刻度；溢出/负值仅钳渲染半径，数值照实显示） ──
const CX = 200;
const CY = 180;
const R = 115;
const LABEL_R = 143;

const angleFor = (i: number) => -Math.PI / 2 + (i * Math.PI * 2) / 6;
const pointAt = (i: number, radius: number) => ({
  x: CX + radius * Math.cos(angleFor(i)),
  y: CY + radius * Math.sin(angleFor(i)),
});
const ringPoints = (fraction: number) =>
  Array.from({ length: 6 }, (_, i) => {
    const p = pointAt(i, R * fraction);
    return `${p.x.toFixed(1)},${p.y.toFixed(1)}`;
  }).join(" ");

const clampRadius = (v: number) => Math.min(100, Math.max(0, v));

/** 展示用数值（tween 动画的当前帧）；角色切换由外层 keyed Transition 重挂载，无需吸附逻辑 */
const displayValues = ref<number[]>([...props.values]);
const dataPoints = computed(() =>
  displayValues.value.map((v, i) => pointAt(i, (clampRadius(v) / 100) * R)),
);
const dataPolygonPoints = computed(() =>
  dataPoints.value.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" "),
);

const axisLabels = computed(() =>
  Array.from({ length: 6 }, (_, i) => {
    const p = pointAt(i, LABEL_R);
    const cos = Math.cos(angleFor(i));
    return {
      x: Number(p.x.toFixed(1)),
      y: Number(p.y.toFixed(1)),
      anchor:
        Math.abs(cos) < 0.3 ? ("middle" as const) : cos > 0 ? ("start" as const) : ("end" as const),
    };
  }),
);

const vertexColor = (v: number) =>
  v > 100 ? props.palette.vertexOverflow : v < 0 ? props.palette.cold : props.palette.vertex;
const valueStyle = (v: number) => ({
  fill: vertexColor(v),
  ...(v > 100 ? { filter: `drop-shadow(0 0 4px ${props.palette.glow})` } : {}),
});

// ── 数值 tween：props.values 变化时 600ms 缓动过渡 ──
let rafId: number | null = null;

function cancelTween() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

function tweenTo(target: number[]) {
  cancelTween();
  const from = [...displayValues.value];
  const t0 = performance.now();
  const duration = 600;
  const step = (t: number) => {
    const p = Math.min(1, (t - t0) / duration);
    const eased = 1 - Math.pow(1 - p, 3);
    displayValues.value = from.map((f, i) => f + (target[i] - f) * eased);
    rafId = p < 1 ? requestAnimationFrame(step) : null;
  };
  rafId = requestAnimationFrame(step);
}

watch(
  () => props.values,
  (target) => tweenTo(target),
);

onUnmounted(cancelTween);

// ── 顶点/标注 hover ──
const hovered = ref<number | null>(null);
</script>

<style scoped>
/* 数据多边形：渐变填充（fill 由模板内联），描边色由 palette 内联 */
.radar-data-polygon {
  fill-opacity: 0.35;
  stroke-width: 2;
  stroke-linejoin: round;
}

/* 满溢顶点（>100，钳在满刻度边缘）的发光脉冲光晕；颜色由 palette.halo 内联 */
.radar-vertex-halo {
  transform-box: fill-box;
  transform-origin: center;
  animation: radar-vertex-pulse 1.6s ease-in-out infinite;
}
@keyframes radar-vertex-pulse {
  0%,
  100% {
    opacity: 0.4;
    transform: scale(1);
  }
  50% {
    opacity: 1;
    transform: scale(1.7);
  }
}
</style>
