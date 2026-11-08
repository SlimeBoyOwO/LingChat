<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from "vue";
import { useI18n } from "vue-i18n";
import type {
  LightSource,
  LightingCameraParams,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
  LightingShot,
} from "@/api/services/scene";
import { SceneLightingRenderer } from "@/render/lighting";
import { SHOT_CAMERA_MAP } from "./types";

const props = defineProps<{
  backgroundSrc?: string | null;
  characterSrc?: string | null;
  currentPresetName?: string;
  lights: LightSource[];
  globalParams: LightingGlobalParams;
  compositingParams: LightingCompositingParams;
  materialParams: LightingMaterialParams;
  cameraParams: LightingCameraParams;
  selectedLightIndex: number;
  compareOriginal: boolean;
}>();

const emit = defineEmits<{
  (e: "update:selectedLightIndex", index: number): void;
  (e: "update:backgroundSrc", src: string): void;
  (e: "update:lightPos", payload: { index: number; x: number; y: number }): void;
  (
    e: "update:lightDirection",
    payload: { index: number; azimuth: number; elevation: number },
  ): void;
  (e: "setShot", shot: LightingShot): void;
}>();

const { t } = useI18n();
const canvasRef = ref<HTMLCanvasElement | null>(null);
const stageContainerRef = ref<HTMLDivElement | null>(null);
const radarCanvasRef = ref<HTMLCanvasElement | null>(null);

let renderer: SceneLightingRenderer | null = null;
let bgImg: HTMLImageElement | null = null;
let charImg: HTMLImageElement | null = null;

const STAGE_W = 960;
const STAGE_H = 540;

// Track dragging gizmo
const isDraggingGizmo = ref(false);
let activeDragIndex = -1;

// Hold original comparison
const isComparing = ref(false);

// Fullscreen mode state
const isFullscreen = ref(false);
const showFullscreenBanner = ref(true);
let bannerTimer: ReturnType<typeof setTimeout> | null = null;

const currentShot = computed(() => props.cameraParams.shot);

// Local custom background file upload
const bgFileInputRef = ref<HTMLInputElement | null>(null);

function triggerBgUpload() {
  bgFileInputRef.value?.click();
}

function onLocalBgFileSelected(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = (evt) => {
    const dataUrl = String(evt.target?.result || "");
    emit("update:backgroundSrc", dataUrl);
  };
  reader.readAsDataURL(file);
}

// Collapsible bottom telemetry
const isMetricsCollapsed = ref(false);

const selectedLight = computed(() => props.lights[props.selectedLightIndex]);

const selectedLightStatus = computed(() => {
  const l = selectedLight.value;
  if (!l) return t("settings.sceneEdit.lightingWorkbench.stage.unselected");
  const name =
    l.name ||
    (l.type === "ambient"
      ? t("settings.sceneEdit.lightingWorkbench.lights.addAmbient").replace(/^\+\s*/, "")
      : l.type === "directional"
        ? t("settings.sceneEdit.lightingWorkbench.lights.addDirectional").replace(/^\+\s*/, "")
        : t("settings.sceneEdit.lightingWorkbench.lights.addPoint").replace(/^\+\s*/, ""));
  if (l.type === "directional") {
    return `${name} · 方位 ${l.azimuth ?? 115}° / 仰角 ${l.elevation ?? 30}°`;
  }
  return `${name} @ (${l.x.toFixed(2)}, ${l.y.toFixed(2)})`;
});

const telemetry = computed(() => {
  const m = props.globalParams.master ?? 1;
  const amb = props.lights.find((l) => l.type === "ambient");
  const ambInt = amb?.intensity ?? 0.35;
  const key = props.lights.find((l) => l.is_key) || props.lights[0];
  const keyInt = key?.intensity ?? 0.8;

  const face = Math.min(0.99, +(0.25 + keyInt * 0.35 * m + ambInt * 0.15).toFixed(3));
  const char = Math.min(0.99, +(0.28 + keyInt * 0.38 * m).toFixed(3));
  const bg = Math.min(0.99, +(0.15 + ambInt * 0.25).toFixed(3));
  const ratio = Math.round((ambInt / (ambInt + keyInt + 0.001)) * 100);
  const white = Math.min(1.0, +(0.7 + keyInt * 0.28).toFixed(2));
  const shadow = Math.min(0.8, +(0.1 + ambInt * 0.22).toFixed(3));

  return { face, char, bg, ratio, white, shadow };
});

function resetBannerTimer() {
  showFullscreenBanner.value = true;
  if (bannerTimer) clearTimeout(bannerTimer);
  bannerTimer = setTimeout(() => {
    showFullscreenBanner.value = false;
  }, 3200);
}

function enterFullscreen() {
  isFullscreen.value = true;
  resetBannerTimer();
  window.addEventListener("keydown", handleKeydown);
  window.addEventListener("mousemove", onMouseMoveFullscreen);
  renderFrame();
}

function exitFullscreen() {
  isFullscreen.value = false;
  if (bannerTimer) clearTimeout(bannerTimer);
  window.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("mousemove", onMouseMoveFullscreen);
  renderFrame();
}

function onMouseMoveFullscreen() {
  if (isFullscreen.value) {
    resetBannerTimer();
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" || e.key === "Esc") {
    exitFullscreen();
  }
}

// Initialize images and WebGL renderer
async function initRenderer() {
  if (!canvasRef.value) return;

  try {
    renderer = new SceneLightingRenderer(canvasRef.value);
    renderer.resize(STAGE_W, STAGE_H);

    // Load background
    bgImg = new Image();
    bgImg.crossOrigin = "anonymous";
    bgImg.src =
      props.backgroundSrc ||
      "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='960' height='540'><rect width='960' height='540' fill='%231a1d26'/><text x='480' y='270' fill='%23475569' font-size='24' text-anchor='middle'>Scene Background</text></svg>";

    await new Promise((resolve) => {
      if (bgImg!.complete) return resolve(true);
      bgImg!.onload = () => resolve(true);
      bgImg!.onerror = () => resolve(false);
    });
    renderer.setBackground(bgImg);

    // Load character if present
    if (props.characterSrc) {
      charImg = new Image();
      charImg.crossOrigin = "anonymous";
      charImg.src = props.characterSrc;
      await new Promise((resolve) => {
        if (charImg!.complete) return resolve(true);
        charImg!.onload = () => resolve(true);
        charImg!.onerror = () => resolve(false);
      });
      renderer.setCharacter(charImg, { depth: "auto" });
    }

    renderFrame();
  } catch (err) {
    console.warn("Failed to initialize SceneLightingRenderer:", err);
  }
}

// Render single frame
function renderFrame() {
  if (!renderer) return;

  const bgRect: [number, number, number, number] = [0, 0, STAGE_W, STAGE_H];

  // Character framing box based on camera zoom & focus_y
  const zoom = props.cameraParams.zoom || 1.25;
  const focusY = props.cameraParams.focus_y || 0.16;
  const charAspect =
    charImg && charImg.naturalWidth && charImg.naturalHeight
      ? charImg.naturalWidth / charImg.naturalHeight
      : 0.75;

  const baseH = STAGE_H * 0.92 * zoom;
  const baseW = baseH * charAspect;
  const charX = (STAGE_W - baseW) * 0.5;
  const charY = (STAGE_H - baseH) * 0.5 + focusY * STAGE_H;

  const charRect: [number, number, number, number] = [charX, charY, baseW, baseH];

  if (props.compareOriginal || isComparing.value) {
    // Unlit mode
    renderer.render({
      stage: { width: STAGE_W, height: STAGE_H },
      backgroundRect: bgRect,
      characterRect: charRect,
      lights: [],
      global: {
        master: 1,
        ambient_blend: 0,
        albedo_protect: 0,
        soft_knee: 0.7,
        lift: 0,
        mono: 0,
      },
      compositing: {
        light_wrap: 0,
        rim_width: 0,
        bloom_int: 0,
        bloom_thresh: 1,
      },
      material: { ...props.materialParams, use_normal: false, use_depth: false },
      camera: props.cameraParams,
    });
  } else {
    renderer.render({
      stage: { width: STAGE_W, height: STAGE_H },
      backgroundRect: bgRect,
      characterRect: charRect,
      lights: props.lights,
      global: props.globalParams,
      compositing: props.compositingParams,
      material: props.materialParams,
      camera: props.cameraParams,
    });
  }

  drawRadar();
}

// Watch props to trigger repaint
watch(
  () => [
    props.lights,
    props.globalParams,
    props.compositingParams,
    props.materialParams,
    props.cameraParams,
    props.compareOriginal,
    props.selectedLightIndex,
  ],
  () => {
    renderFrame();
  },
  { deep: true },
);

watch(
  () => props.backgroundSrc,
  (newSrc) => {
    if (!renderer || !newSrc) return;
    bgImg = new Image();
    bgImg.crossOrigin = "anonymous";
    bgImg.onload = () => {
      renderer?.setBackground(bgImg!);
      renderFrame();
    };
    bgImg.src = newSrc;
  },
);

// Gizmo pin dragging on canvas
function startGizmoDrag(index: number, e: MouseEvent) {
  e.stopPropagation();
  emit("update:selectedLightIndex", index);
  isDraggingGizmo.value = true;
  activeDragIndex = index;
  window.addEventListener("mousemove", onGizmoDrag);
  window.addEventListener("mouseup", stopGizmoDrag);
}

function onGizmoDrag(e: MouseEvent) {
  if (!isDraggingGizmo.value || !stageContainerRef.value) return;
  const rect = stageContainerRef.value.getBoundingClientRect();
  const rawX = (e.clientX - rect.left) / rect.width;
  const rawY = (e.clientY - rect.top) / rect.height;
  const clampedX = Math.max(0.02, Math.min(0.98, rawX));
  const clampedY = Math.max(0.02, Math.min(0.98, rawY));

  emit("update:lightPos", {
    index: activeDragIndex,
    x: +clampedX.toFixed(3),
    y: +clampedY.toFixed(3),
  });
}

function stopGizmoDrag() {
  isDraggingGizmo.value = false;
  activeDragIndex = -1;
  window.removeEventListener("mousemove", onGizmoDrag);
  window.removeEventListener("mouseup", stopGizmoDrag);
}

// Polar Radar Direction Pad
function drawRadar() {
  if (!radarCanvasRef.value || isFullscreen.value) return;
  const ctx = radarCanvasRef.value.getContext("2d");
  if (!ctx) return;

  const w = radarCanvasRef.value.width;
  const h = radarCanvasRef.value.height;
  const cx = w / 2;
  const cy = h / 2;
  const r = Math.min(cx, cy) - 6;

  ctx.clearRect(0, 0, w, h);

  // Background circle
  ctx.beginPath();
  ctx.arc(cx, cy, r, 0, Math.PI * 2);
  ctx.fillStyle = "#0c0e14";
  ctx.fill();
  ctx.strokeStyle = "#242938";
  ctx.lineWidth = 1;
  ctx.stroke();

  // Concentric elevation rings
  [0.33, 0.66, 1.0].forEach((ratio) => {
    ctx.beginPath();
    ctx.arc(cx, cy, r * ratio, 0, Math.PI * 2);
    ctx.strokeStyle = "#1a202c";
    ctx.stroke();
  });

  // Cross axes
  ctx.beginPath();
  ctx.moveTo(cx, cy - r);
  ctx.lineTo(cx, cy + r);
  ctx.moveTo(cx - r, cy);
  ctx.lineTo(cx + r, cy);
  ctx.strokeStyle = "#1a202c";
  ctx.stroke();

  // Active light vector
  const sel = props.lights[props.selectedLightIndex];
  if (sel && (sel.type === "directional" || sel.type === "spot")) {
    const rad = ((sel.azimuth - 90) * Math.PI) / 180;
    const elevRatio = Math.cos((sel.elevation * Math.PI) / 180);
    const px = cx + Math.cos(rad) * r * elevRatio;
    const py = cy + Math.sin(rad) * r * elevRatio;

    // Line from center
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    ctx.lineTo(px, py);
    ctx.strokeStyle = sel.color || "#38bdf8";
    ctx.lineWidth = 2;
    ctx.stroke();

    // End point
    ctx.beginPath();
    ctx.arc(px, py, 4, 0, Math.PI * 2);
    ctx.fillStyle = sel.color || "#38bdf8";
    ctx.fill();
  }
}

onMounted(() => {
  initRenderer();
});

onUnmounted(() => {
  exitFullscreen();
  stopGizmoDrag();
});
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden bg-[#090b10]">
    <!-- Top toolbar: Camera framing & Compare (hidden in fullscreen) -->
    <div
      v-if="!isFullscreen"
      class="flex h-10 shrink-0 items-center justify-between border-b border-[#283042] bg-[#11141c] px-3 text-xs"
    >
      <div class="flex items-center gap-2.5">
        <div class="flex items-center gap-1.5">
          <span class="mr-0.5 font-bold text-slate-300">{{
            $t("settings.sceneEdit.lightingWorkbench.stage.framing")
          }}</span>
          <button
            v-for="(val, key) in SHOT_CAMERA_MAP"
            :key="key"
            type="button"
            @click="emit('setShot', key)"
            :class="[
              'rounded px-2 py-0.5 text-xs font-medium transition-colors',
              currentShot === key
                ? 'bg-purple-600 text-white shadow-sm'
                : 'bg-[#181d28] text-slate-400 hover:bg-[#222938] hover:text-slate-200',
            ]"
          >
            {{ $t("settings.sceneEdit.lightingWorkbench.stage.shots." + key) }}
          </button>
        </div>

        <button
          type="button"
          @click="triggerBgUpload"
          class="rounded border border-[#2b3347] bg-[#171c26] px-2 py-0.5 text-xs font-medium text-slate-300 transition-colors hover:bg-[#202736] hover:text-white"
          :title="$t('settings.sceneEdit.lightingWorkbench.stage.customBgTitle')"
        >
          + 本地场景
        </button>
        <input
          ref="bgFileInputRef"
          type="file"
          accept="image/*"
          class="hidden"
          @change="onLocalBgFileSelected"
        />
      </div>

      <button
        type="button"
        @mousedown="isComparing = true"
        @mouseup="isComparing = false"
        @mouseleave="isComparing = false"
        :class="[
          'rounded border border-[#2b3245] px-2.5 py-0.5 text-xs font-medium transition-all select-none',
          isComparing
            ? 'border-amber-500/60 bg-amber-500/20 text-amber-300'
            : 'bg-[#161a24] text-slate-300 hover:bg-[#1e2433] hover:text-white',
        ]"
      >
        按住对比原图
      </button>
    </div>

    <!-- Fullscreen Top Exit Banner (Bilibili/Chrome style) -->
    <div
      v-if="isFullscreen"
      class="fixed top-5 left-1/2 z-[10002] flex -translate-x-1/2 flex-col items-center gap-1.5 transition-opacity duration-300"
      :class="showFullscreenBanner ? 'opacity-100' : 'pointer-events-none opacity-0'"
    >
      <button
        type="button"
        @click="exitFullscreen"
        class="flex h-7 w-7 items-center justify-center rounded-full border border-white/25 bg-slate-900/90 text-xs font-bold text-slate-200 shadow-xl transition-all hover:scale-110 hover:bg-rose-600 hover:text-white"
        title="退出全屏 (ESC)"
      >
        ✕
      </button>
      <div
        class="flex items-center gap-2 rounded-md border border-white/20 bg-slate-900/95 px-3.5 py-1.5 text-xs text-slate-200 shadow-2xl backdrop-blur-md"
      >
        <span>{{ $t("settings.sceneEdit.lightingWorkbench.stage.fullscreenToast") }}</span>
        <kbd
          class="rounded border border-slate-700 bg-slate-950 px-1.5 py-0.5 font-mono text-[11px] text-sky-400"
          >Esc</kbd
        >
      </div>
    </div>

    <!-- Canvas Stage (Letterbox centered) -->
    <div
      class="relative flex flex-1 items-center justify-center"
      :class="isFullscreen ? 'p-0' : 'p-4'"
    >
      <div
        ref="stageContainerRef"
        :class="[
          'group/stage transition-all',
          isFullscreen
            ? 'fixed inset-0 z-[10000] h-screen w-screen max-w-none rounded-none border-none bg-black'
            : 'relative aspect-video w-full max-w-[960px] overflow-hidden rounded-lg border border-[#283042] bg-black shadow-2xl',
        ]"
      >
        <canvas
          ref="canvasRef"
          width="960"
          height="540"
          class="h-full w-full object-contain"
        ></canvas>

        <!-- Hover Fullscreen Trigger in top-right (blue-boxed area) -->
        <button
          v-if="!isFullscreen"
          type="button"
          @click="enterFullscreen"
          class="absolute top-3 right-3 z-30 flex items-center gap-1 rounded border border-white/20 bg-slate-950/75 px-2.5 py-1 text-xs font-semibold text-white/90 opacity-0 shadow-lg backdrop-blur-md transition-all group-hover/stage:opacity-100 hover:scale-105 hover:bg-slate-900 hover:text-white"
          :title="$t('settings.sceneEdit.lightingWorkbench.stage.fullscreenTitle')"
        >
          {{ $t("settings.sceneEdit.lightingWorkbench.stage.fullscreen") }}
        </button>

        <!-- Interactive Light Gizmos (hidden in fullscreen) -->
        <template v-if="!isFullscreen">
          <div
            v-for="(l, idx) in lights"
            :key="l.id"
            v-show="l.type === 'point' || l.type === 'spot'"
            class="absolute -translate-x-1/2 -translate-y-1/2 cursor-grab active:cursor-grabbing"
            :style="{
              left: `${l.x * 100}%`,
              top: `${l.y * 100}%`,
            }"
            @mousedown="startGizmoDrag(idx, $event)"
          >
            <div
              :class="[
                'flex h-6 w-6 items-center justify-center rounded-full border text-[10px] font-bold shadow-lg transition-transform',
                selectedLightIndex === idx
                  ? 'scale-125 border-white bg-purple-600 text-white shadow-purple-500/50'
                  : 'border-slate-500/60 bg-slate-900/80 text-slate-300 hover:scale-110',
              ]"
              :style="{
                boxShadow:
                  selectedLightIndex === idx ? `0 0 12px ${l.color}` : `0 0 6px ${l.color}88`,
              }"
            >
              #{{ idx + 1 }}
            </div>
          </div>
        </template>

        <!-- Direction Radar Compass (bottom right, hidden in fullscreen) -->
        <div
          v-if="!isFullscreen"
          class="absolute right-3 bottom-3 flex flex-col items-center rounded-lg border border-[#242b3d] bg-[#0c0e14]/90 p-1.5 shadow-xl backdrop-blur-sm"
        >
          <canvas ref="radarCanvasRef" width="72" height="72" class="block"></canvas>
          <span class="mt-1 text-[9px] text-slate-400">{{
            $t("settings.sceneEdit.lightingWorkbench.stage.radarTitle")
          }}</span>
        </div>
      </div>
    </div>

    <!-- Stage Bottom Status & Collapsible Telemetry -->
    <footer
      v-if="!isFullscreen"
      class="shrink-0 border-t border-[#283042] bg-[#0d1017] px-3.5 py-2 text-xs transition-all"
    >
      <div class="flex items-center justify-between text-slate-400">
        <div class="flex items-center gap-2">
          <span
            >{{ $t("settings.sceneEdit.lightingWorkbench.stage.currentPreset") }}
            <strong class="font-semibold text-indigo-400">{{
              currentPresetName || $t("settings.sceneEdit.lightingWorkbench.stage.customPreset")
            }}</strong></span
          >
          <span class="text-slate-600">|</span>
          <span
            >{{ $t("settings.sceneEdit.lightingWorkbench.stage.selectedLight") }}
            <span class="font-medium text-slate-300">{{ selectedLightStatus }}</span></span
          >
        </div>
        <div class="flex items-center gap-3">
          <span
            >{{ $t("settings.sceneEdit.lightingWorkbench.stage.gpuRenderTime") }}
            <strong class="font-mono text-emerald-400">0.1 ms</strong></span
          >
          <button
            type="button"
            @click="isMetricsCollapsed = !isMetricsCollapsed"
            class="rounded border border-[#2b3347] bg-[#161b26] px-2 py-0.5 text-[11px] text-slate-300 transition-colors hover:bg-[#202738] hover:text-white"
            title="折叠/展开指标面板"
          >
            {{
              isMetricsCollapsed
                ? $t("settings.sceneEdit.lightingWorkbench.stage.expandMetrics")
                : $t("settings.sceneEdit.lightingWorkbench.stage.collapseMetrics")
            }}
          </button>
        </div>
      </div>

      <!-- 6 Metrics Cards Grid (Collapsible) -->
      <div v-show="!isMetricsCollapsed" class="mt-2 grid grid-cols-6 gap-2 pt-1 transition-all">
        <div class="rounded border border-[#1e2535] bg-[#131722]/80 px-2 py-1 text-center">
          <div class="text-[10px] text-slate-400">
            {{ $t("settings.sceneEdit.lightingWorkbench.stage.metrics.face") }}
          </div>
          <div class="font-mono text-xs font-bold text-amber-300">
            {{ telemetry.face.toFixed(3) }}
          </div>
        </div>
        <div class="rounded border border-[#1e2535] bg-[#131722]/80 px-2 py-1 text-center">
          <div class="text-[10px] text-slate-400">
            {{ $t("settings.sceneEdit.lightingWorkbench.stage.metrics.char") }}
          </div>
          <div class="font-mono text-xs font-bold text-purple-300">
            {{ telemetry.char.toFixed(3) }}
          </div>
        </div>
        <div class="rounded border border-[#1e2535] bg-[#131722]/80 px-2 py-1 text-center">
          <div class="text-[10px] text-slate-400">
            {{ $t("settings.sceneEdit.lightingWorkbench.stage.metrics.bg") }}
          </div>
          <div class="font-mono text-xs font-bold text-sky-300">{{ telemetry.bg.toFixed(3) }}</div>
        </div>
        <div class="rounded border border-[#1e2535] bg-[#131722]/80 px-2 py-1 text-center">
          <div class="text-[10px] text-slate-400">
            {{ $t("settings.sceneEdit.lightingWorkbench.stage.metrics.ratio") }}
          </div>
          <div class="font-mono text-xs font-bold text-emerald-300">{{ telemetry.ratio }}%</div>
        </div>
        <div class="rounded border border-[#1e2535] bg-[#131722]/80 px-2 py-1 text-center">
          <div class="text-[10px] text-slate-400">
            {{ $t("settings.sceneEdit.lightingWorkbench.stage.metrics.white") }}
          </div>
          <div class="font-mono text-xs font-bold text-rose-300">
            {{ telemetry.white.toFixed(2) }}
          </div>
        </div>
        <div class="rounded border border-[#1e2535] bg-[#131722]/80 px-2 py-1 text-center">
          <div class="text-[10px] text-slate-400">
            {{ $t("settings.sceneEdit.lightingWorkbench.stage.metrics.shadow") }}
          </div>
          <div class="font-mono text-xs font-bold text-indigo-300">
            {{ telemetry.shadow.toFixed(3) }}
          </div>
        </div>
      </div>
    </footer>
  </div>
</template>
