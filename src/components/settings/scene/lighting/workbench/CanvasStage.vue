<script setup lang="ts">
/**
 * 画布舞台（外壳）。
 *
 * 只做三件：把渲染接线交给 useStageRenderer、处理灯点拖拽与平移缩放、
 * 画四角标注。场景条与底部控制条是独立组件，模板不再和 WebGL 挤在一起。
 */
import { computed, onUnmounted, ref } from "vue";
import type {
  LightSource,
  LightingCameraParams,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
  LightingShot,
} from "@/api/services/scene";
import CanvasSceneBar from "./CanvasSceneBar.vue";
import CanvasFootBar from "./CanvasFootBar.vue";
import { STAGE_H, STAGE_W, useStageRenderer } from "./useStageRenderer";
import type { CanvasTool } from "./useWorkbenchUi";

const props = withDefaults(
  defineProps<{
    backgroundSrc?: string | null;
    characterSrc?: string | null;
    lights: LightSource[];
    globalParams: LightingGlobalParams;
    compositingParams: LightingCompositingParams;
    materialParams: LightingMaterialParams;
    cameraParams: LightingCameraParams;
    selectedLightIndex: number;
    compareOriginal: boolean;
    sceneName?: string;
    presetName?: string;
    zoom?: number;
    showGizmo?: boolean;
    showGrid?: boolean;
    tool?: CanvasTool;
    /** 用户上传的法线/深度图（空则自动生成） */
    normalSrc?: string | null;
    depthSrc?: string | null;
    /** 框选命中的灯 id，用来高亮 */
    highlightIds?: string[];
  }>(),
  { zoom: 1, showGizmo: true, showGrid: false, tool: "select" },
);

const emit = defineEmits<{
  (e: "update:selectedLightIndex", index: number): void;
  (e: "update:zoom", zoom: number): void;
  (e: "update:tool", tool: CanvasTool): void;
  (e: "update:showGizmo", v: boolean): void;
  (e: "update:showGrid", v: boolean): void;
  (e: "update:lightPos", payload: { id: string; x: number; y: number }): void;
  (e: "setShot", shot: LightingShot): void;
  (e: "place-light", payload: { x: number; y: number }): void;
  (e: "marquee", payload: { x0: number; y0: number; x1: number; y1: number }): void;
}>();

const { canvasRef, isComparing, bgSize, lastError } = useStageRenderer({
  backgroundSrc: () => props.backgroundSrc,
  characterSrc: () => props.characterSrc,
  lights: () => props.lights,
  globalParams: () => props.globalParams,
  compositingParams: () => props.compositingParams,
  materialParams: () => props.materialParams,
  cameraParams: () => props.cameraParams,
  compareOriginal: () => props.compareOriginal,
  normalSrc: () => props.normalSrc,
  depthSrc: () => props.depthSrc,
});

const frameRef = ref<HTMLDivElement | null>(null);

const panX = ref(0);
const panY = ref(0);

const stageTransform = computed(() => ({
  transform: `translate(${panX.value}px, ${panY.value}px) scale(${props.zoom})`,
}));

// —— 拖拽：灯点 or 手形平移 ——
// 灯点拖拽持 id 而不是下标：拖拽途中撤销/删灯会把整表替换，
// 持下标就会把坐标写给另一盏灯

let dragging: { kind: "light"; id: string } | { kind: "pan"; x: number; y: number } | null = null;

function onPinDown(id: string, e: MouseEvent) {
  e.stopPropagation();
  emit(
    "update:selectedLightIndex",
    props.lights.findIndex((l) => l.id === id),
  );
  dragging = { kind: "light", id };
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragEnd);
  window.addEventListener("blur", onDragEnd);
}

function onViewportDown(e: MouseEvent) {
  if (props.tool !== "hand") return;
  dragging = { kind: "pan", x: e.clientX - panX.value, y: e.clientY - panY.value };
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragEnd);
  window.addEventListener("blur", onDragEnd);
}

function onDragMove(e: MouseEvent) {
  if (!dragging) return;
  if (dragging.kind === "pan") {
    panX.value = e.clientX - dragging.x;
    panY.value = e.clientY - dragging.y;
    return;
  }
  if (!frameRef.value) return;
  const rect = frameRef.value.getBoundingClientRect();
  if (!rect.width || !rect.height) return;
  const nx = (e.clientX - rect.left) / rect.width;
  const ny = (e.clientY - rect.top) / rect.height;
  emit("update:lightPos", {
    id: dragging.id,
    x: +Math.max(0.02, Math.min(0.98, nx)).toFixed(3),
    y: +Math.max(0.02, Math.min(0.98, ny)).toFixed(3),
  });
}

function onDragEnd() {
  dragging = null;
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragEnd);
  window.removeEventListener("blur", onDragEnd);
}

// —— 视口操作 ——

function onStageDblClick() {
  // 稿上的「双击角色快速聚焦」：拉到近景
  emit("setShot", "cu");
}

function onWheel(e: WheelEvent) {
  if (!e.ctrlKey && !e.metaKey) return;
  e.preventDefault();
  const next = Math.min(2, Math.max(0.5, +(props.zoom + (e.deltaY > 0 ? -0.05 : 0.05)).toFixed(2)));
  emit("update:zoom", next);
}

function fitCanvas() {
  emit("update:zoom", 1);
  panX.value = 0;
  panY.value = 0;
}

// —— 全屏预览 ——

const isFullscreen = ref(false);
const hintVisible = ref(false);
let hintTimer: ReturnType<typeof setTimeout> | null = null;

function toggleFullscreen() {
  isFullscreen.value = !isFullscreen.value;
  if (isFullscreen.value) {
    // 进入后 3 秒提示按 Esc 退出
    hintVisible.value = true;
    if (hintTimer) clearTimeout(hintTimer);
    hintTimer = setTimeout(() => {
      hintVisible.value = false;
    }, 3000);
    window.addEventListener("keydown", onFullscreenKey);
  } else {
    hintVisible.value = false;
    if (hintTimer) clearTimeout(hintTimer);
    window.removeEventListener("keydown", onFullscreenKey);
  }
}

function onFullscreenKey(e: KeyboardEvent) {
  if (e.key !== "Escape") return;
  e.preventDefault();
  toggleFullscreen();
}

onUnmounted(() => {
  if (hintTimer) clearTimeout(hintTimer);
  window.removeEventListener("keydown", onFullscreenKey);
  window.removeEventListener("mousemove", onMarqueeMove);
  window.removeEventListener("mouseup", onMarqueeUp);
  // 拖拽中卸载会漏掉这两个监听，之后每次鼠标移动都会继续 emit 坐标
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragEnd);
  window.removeEventListener("blur", onDragEnd);
});

// —— 放置光源 / 框选 ——

const marquee = ref<{ x0: number; y0: number; x1: number; y1: number } | null>(null);

function normPoint(e: MouseEvent): { x: number; y: number } | null {
  const frame = frameRef.value;
  if (!frame) return null;
  const r = frame.getBoundingClientRect();
  if (!r.width || !r.height) return null;
  return {
    x: Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)),
    y: Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)),
  };
}

function onFrameDown(e: MouseEvent) {
  if (props.tool === "marquee") {
    e.preventDefault();
    const p = normPoint(e);
    if (!p) return;
    marquee.value = { x0: p.x, y0: p.y, x1: p.x, y1: p.y };
    window.addEventListener("mousemove", onMarqueeMove);
    window.addEventListener("mouseup", onMarqueeUp);
    return;
  }
  if (props.tool === "light") {
    const p = normPoint(e);
    if (!p) return;
    emit("place-light", { x: +p.x.toFixed(3), y: +p.y.toFixed(3) });
  }
}

function onMarqueeMove(e: MouseEvent) {
  if (!marquee.value) return;
  const p = normPoint(e);
  if (!p) return;
  marquee.value = { ...marquee.value, x1: p.x, y1: p.y };
}

function onMarqueeUp() {
  const m = marquee.value;
  window.removeEventListener("mousemove", onMarqueeMove);
  window.removeEventListener("mouseup", onMarqueeUp);
  marquee.value = null;
  if (!m) return;
  const x0 = Math.min(m.x0, m.x1);
  const y0 = Math.min(m.y0, m.y1);
  const x1 = Math.max(m.x0, m.x1);
  const y1 = Math.max(m.y0, m.y1);
  // 小于 1% 当成误触
  if (x1 - x0 < 0.01 || y1 - y0 < 0.01) return;
  emit("marquee", { x0, y0, x1, y1 });
}

const marqueeStyle = computed(() => {
  const m = marquee.value;
  if (!m) return {};
  const x = Math.min(m.x0, m.x1);
  const y = Math.min(m.y0, m.y1);
  return {
    left: `${x * 100}%`,
    top: `${y * 100}%`,
    width: `${Math.abs(m.x1 - m.x0) * 100}%`,
    height: `${Math.abs(m.y1 - m.y0) * 100}%`,
  };
});

const highlighted = computed(() => new Set(props.highlightIds ?? []));

// —— 派生展示值 ——

const selectedLight = computed(() => props.lights[props.selectedLightIndex] ?? null);

const lightPinLabel = computed(() => {
  const l = selectedLight.value;
  if (!l) return "";
  const typeName =
    l.type === "point"
      ? "点光"
      : l.type === "spot"
        ? "锥光"
        : l.type === "ambient"
          ? "环境光"
          : "平行光";
  return `${typeName} ${l.intensity.toFixed(2)}`;
});

/** 环境占比：环境光强度 / 全部灯强度之和（参数推导，不做像素采样） */
const ambientShare = computed(() => {
  const total = props.lights.reduce((s, l) => s + (l.enabled === false ? 0 : l.intensity), 0);
  const amb = props.lights
    .filter((l) => l.type === "ambient" && l.enabled !== false)
    .reduce((s, l) => s + l.intensity, 0);
  if (total <= 0) return 0;
  return Math.round((amb / total) * 100);
});

const pins = computed(() =>
  props.showGizmo
    ? props.lights
        .map((l, i) => ({ l, i }))
        .filter(({ l }) => (l.type === "point" || l.type === "spot") && l.enabled !== false)
    : [],
);
</script>

<template>
  <div class="stage-wrap" :class="{ 'is-fullscreen': isFullscreen }">
    <CanvasSceneBar
      v-if="!isFullscreen"
      :scene-name="sceneName"
      :shot="cameraParams.shot"
      @set-shot="emit('setShot', $event)"
      @compare-hold="isComparing = $event"
      @fit="fitCanvas"
    />

    <div v-if="lastError && !isFullscreen" class="stage-error" :title="lastError">
      <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor">
        <circle cx="8" cy="8" r="5.6" stroke-width="1.3" />
        <path d="M8 4.9v3.6M8 10.9v.2" stroke-width="1.5" stroke-linecap="round" />
      </svg>
      <span class="wb-ellipsis">{{ lastError }}</span>
    </div>

    <div
      class="stage-viewport"
      :class="{ 'is-fullscreen': isFullscreen }"
      @mousedown="onViewportDown"
      @wheel="onWheel"
    >
      <div
        ref="frameRef"
        class="stage-frame"
        :class="{ 'is-hand': tool === 'hand' }"
        :style="stageTransform"
        @dblclick="onStageDblClick"
        @mousedown="onFrameDown"
      >
        <canvas ref="canvasRef" :width="STAGE_W" :height="STAGE_H" class="stage-canvas" />

        <div v-if="showGrid" class="stage-grid" />

        <!-- 光源点 -->
        <div
          v-for="pin in isFullscreen ? [] : pins"
          :key="pin.l.id"
          class="stage-pin"
          :style="{ left: `${pin.l.x * 100}%`, top: `${pin.l.y * 100}%` }"
          @mousedown="onPinDown(pin.l.id, $event)"
        >
          <span
            class="stage-pin-dot"
            :class="{
              'is-active': pin.i === selectedLightIndex,
              'is-hl': highlighted.has(pin.l.id),
            }"
            :style="{ boxShadow: `0 0 0 3px ${pin.l.color}55` }"
          />
          <span v-if="pin.i === selectedLightIndex" class="stage-pin-tag">{{ lightPinLabel }}</span>
        </div>

        <!-- 框选矩形 -->
        <div v-if="marquee" class="stage-marquee" :style="marqueeStyle" />

        <!-- 四角标注（全屏预览时隐藏，只留画面） -->
        <template v-if="!isFullscreen">
          <div class="stage-hud hud-tl">场景 · {{ sceneName || "未命名" }}</div>
          <div class="stage-hud hud-tr">{{ bgSize.width }} × {{ bgSize.height }}</div>
          <div class="stage-hud hud-bl">
            <svg viewBox="0 0 16 16" width="9" height="9" fill="none" stroke="currentColor">
              <rect x="3.4" y="7" width="9.2" height="6" rx="1.4" stroke-width="1.3" />
              <path d="M5.6 7V5.2a2.4 2.4 0 0 1 4.8 0V7" stroke-width="1.3" />
            </svg>
            {{ presetName || "未选预设" }} · 已锁定
          </div>
          <div class="stage-hud hud-br">
            镜头 {{ cameraParams.zoom.toFixed(2) }}× · 环境 {{ ambientShare }}%
          </div>
        </template>
      </div>
    </div>

    <CanvasFootBar
      v-if="!isFullscreen"
      :show-gizmo="showGizmo"
      :show-grid="showGrid"
      :zoom="zoom"
      :tool="tool"
      @update:show-gizmo="emit('update:showGizmo', $event)"
      @update:show-grid="emit('update:showGrid', $event)"
      @update:zoom="emit('update:zoom', $event)"
      @update:tool="emit('update:tool', $event)"
      :fullscreen="isFullscreen"
      @toggle-fullscreen="toggleFullscreen"
    />

    <!-- 全屏提示与出口 -->
    <transition name="hint-fade">
      <div v-if="isFullscreen && hintVisible" class="fullscreen-hint">
        按 <kbd>Esc</kbd> 退出全屏
      </div>
    </transition>
    <button
      v-if="isFullscreen"
      type="button"
      class="fullscreen-exit"
      title="退出全屏 (Esc)"
      @click="toggleFullscreen"
    >
      ✕
    </button>
  </div>
</template>

<style scoped>
.stage-wrap {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: var(--wb-canvas);
}

.stage-viewport {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 14px;
}

.stage-frame {
  position: relative;
  aspect-ratio: 16 / 9;
  max-width: 100%;
  max-height: 100%;
  width: 100%;
  border-radius: var(--wb-r-lg);
  overflow: hidden;
  background: #dfe0e6;
  box-shadow: 0 10px 30px rgb(23 23 28 / 10%);
  transition: transform 0.12s ease-out;
}

.stage-frame.is-hand {
  cursor: grab;
}

.stage-frame.is-hand:active {
  cursor: grabbing;
}

.stage-canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.stage-grid {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background-image:
    linear-gradient(to right, rgb(255 255 255 / 30%) 1px, transparent 1px),
    linear-gradient(to bottom, rgb(255 255 255 / 30%) 1px, transparent 1px);
  background-size: 8.333% 8.333%;
}

.stage-pin {
  position: absolute;
  transform: translate(-50%, -50%);
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: grab;
  z-index: 6;
}

.stage-pin:active {
  cursor: grabbing;
}

.stage-pin-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #fff;
  border: 1.5px solid #fff;
  box-shadow: 0 1px 4px rgb(0 0 0 / 35%);
}

.stage-pin-dot.is-active {
  background: var(--wb-accent);
  border-color: #fff;
  width: 9px;
  height: 9px;
}

.stage-pin-tag {
  padding: 1.5px 6px;
  border-radius: 999px;
  background: rgb(255 255 255 / 94%);
  color: var(--wb-text);
  font-size: 10px;
  font-weight: 600;
  white-space: nowrap;
  box-shadow: 0 2px 8px rgb(23 23 28 / 16%);
}

.stage-hud {
  position: absolute;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border-radius: 5px;
  background: rgb(255 255 255 / 72%);
  backdrop-filter: blur(4px);
  color: #4a4a55;
  font-size: 10px;
  pointer-events: none;
  user-select: none;
}

.hud-tl {
  left: 8px;
  top: 8px;
}

.hud-tr {
  right: 8px;
  top: 8px;
  font-variant-numeric: tabular-nums;
}

.hud-bl {
  left: 8px;
  bottom: 8px;
}

.hud-br {
  right: 8px;
  bottom: 8px;
  font-variant-numeric: tabular-nums;
}
/* 渲染异常条：画布空白时至少能看见原因 */
.stage-error {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 6px 10px 0;
  padding: 4px 8px;
  border-radius: var(--wb-r-sm);
  background: #fdecec;
  border: 1px solid #f6cfcf;
  color: #b23b3b;
  font-size: 10.5px;
  flex-shrink: 0;
}

.stage-error .wb-ellipsis {
  flex: 1;
  min-width: 0;
}

/* 框选矩形 */
.stage-marquee {
  position: absolute;
  border: 1px dashed var(--wb-accent);
  background: rgb(110 86 248 / 12%);
  pointer-events: none;
  z-index: 8;
}

.stage-pin-dot.is-hl {
  background: var(--wb-accent);
  box-shadow: 0 0 0 4px rgb(110 86 248 / 32%);
}

/* 全屏：只留画布，其余 chrome 由 v-if 摘掉 */
.stage-wrap.is-fullscreen {
  position: fixed;
  inset: 0;
  z-index: 10001;
  background: #000;
}

.stage-viewport.is-fullscreen {
  padding: 0;
  background: #000;
}

.stage-wrap.is-fullscreen .stage-frame {
  border-radius: 0;
  box-shadow: none;
  background: #000;
}

.fullscreen-hint {
  position: fixed;
  top: 18px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 10002;
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 6px 14px;
  border-radius: 999px;
  background: rgb(23 23 28 / 86%);
  color: #fff;
  font-size: 12px;
  pointer-events: none;
}

.fullscreen-hint kbd {
  padding: 1px 5px;
  border-radius: 4px;
  background: rgb(255 255 255 / 16%);
  font-family: inherit;
  font-size: 11px;
}

.fullscreen-exit {
  position: fixed;
  top: 14px;
  right: 14px;
  z-index: 10002;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: rgb(23 23 28 / 72%);
  color: #fff;
  font-size: 12px;
  transition:
    background 0.12s ease,
    transform 0.12s ease;
}

.fullscreen-exit:hover {
  background: rgb(224 85 85 / 92%);
  transform: scale(1.06);
}

.hint-fade-leave-active {
  transition: opacity 0.3s ease;
}

.hint-fade-leave-to {
  opacity: 0;
}
</style>
