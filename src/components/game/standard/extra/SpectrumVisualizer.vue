<template>
  <!-- 迷你频谱（与左下角"声效"按钮对称的右下角），展开后是同源的详情面板 -->
  <div
    v-if="visible"
    ref="rootRef"
    class="fixed right-6 bottom-[calc(24px+var(--safe-area-inset-bottom))] z-1000 flex flex-col items-end gap-2"
  >
    <Transition
      enter-active-class="transition-all duration-300 ease-out"
      leave-active-class="transition-all duration-200 ease-in"
      enter-from-class="opacity-0 translate-y-3 scale-95"
      leave-to-class="opacity-0 translate-y-3 scale-95"
    >
      <div
        v-if="expanded"
        class="w-59 rounded-2xl border border-white/10 bg-[#12121c]/45 p-3 text-white shadow-[0_10px_30px_rgba(0,0,0,0.35)] backdrop-blur-[18px]"
      >
        <!-- 当前音源 + 采集状态点（未接入音频时为暗灰） -->
        <div class="mb-2 flex items-center gap-2">
          <AudioLines :size="12" class="shrink-0 text-(--accent-color)" />
          <span class="min-w-0 flex-1 truncate text-[11px] font-semibold text-gray-200">
            {{ nowPlayingText }}
          </span>
          <span
            class="h-1.5 w-1.5 shrink-0 rounded-full transition-colors duration-300"
            :style="{ background: spectrumActive ? colors.from : 'rgba(255, 255, 255, 0.22)' }"
          ></span>
        </div>

        <canvas ref="fullCanvasRef" class="mx-auto block"></canvas>

        <!-- 配色快捷切换（完整配置在声效面板） -->
        <div class="mt-2.5" @click.stop>
          <SpectrumPaletteChips
            :model-value="activePalette"
            :custom-from="settingsStore.audio.spectrumColor1"
            :custom-to="settingsStore.audio.spectrumColor2"
            @update:model-value="selectPalette"
          />
        </div>
      </div>
    </Transition>

    <!-- 迷你频谱：没有外壳，只有浮在画面上的柱子 -->
    <button
      class="cursor-pointer border-none bg-transparent p-1 opacity-90 transition-all duration-200 hover:scale-105 hover:opacity-100 active:scale-95"
      :title="expanded ? $t('game.spectrum.collapse') : $t('game.spectrum.expand')"
      @click.stop="expanded = !expanded"
    >
      <canvas ref="miniCanvasRef" class="block"></canvas>
    </button>
  </div>
</template>

<script setup lang="ts">
import {
  DEFAULT_SPECTRUM_PALETTE,
  resolveSpectrumColors,
  resolveSpectrumStyle,
  type SpectrumStyle,
} from "@/constants/spectrum";
import SpectrumPaletteChips from "./SpectrumPaletteChips.vue";
import { useSettingsStore } from "@/stores/modules/settings";
import { useUIStore } from "@/stores/modules/ui/ui";
import {
  getSpectrumAnalyser,
  getSpectrumSampleRate,
  isSpectrumSupported,
  setSpectrumEnabled,
  spectrumActive,
} from "@/utils/audioSpectrum";
import { AudioLines } from "lucide-vue-next";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const uiStore = useUIStore();
const settingsStore = useSettingsStore();
const { t } = useI18n();

/** 频带总数：取三种形态里用到的最大柱数（圆环最大 48），其余按比例合并 */
const MAX_BARS = 48;

interface Spec {
  kind: SpectrumStyle;
  w: number;
  h: number;
  bars: number;
  gap: number;
  glow: number;
  /** 圆环：基准半径 / 单根柱最大长度 */
  r0: number;
  len: number;
}

/**
 * 每个形态的尺寸。
 * 画布要留出辉光的余量（glowPadding），否则柱子外发光会被画布边缘裁成硬边
 * ——没有外壳遮挡时这道硬边很显眼。
 * - 镜像/柱状是宽扁的，迷你态 88×32（加按钮 4px 内边距 = 40，与左侧"声效"按钮等高）
 * - 圆环要方正，迷你态 48×48，展开态 132×132（居中）
 */
function specFor(style: SpectrumStyle, big: boolean): Spec {
  if (style === "ring") {
    return big
      ? { kind: style, w: 132, h: 132, bars: 48, gap: 0, glow: 6, r0: 32, len: 26 }
      : { kind: style, w: 48, h: 48, bars: 26, gap: 0, glow: 4, r0: 11, len: 9 };
  }
  return big
    ? { kind: style, w: 212, h: 60, bars: 40, gap: 2, glow: 6, r0: 0, len: 0 }
    : { kind: style, w: 88, h: 32, bars: 20, gap: 1.6, glow: 4, r0: 0, len: 0 };
}

/** 帧率上限：40 帧肉眼已足够顺滑，明显更省 WebView 开销 */
const FRAME_MS = 1000 / 40;

const rootRef = ref<HTMLElement | null>(null);
const miniCanvasRef = ref<HTMLCanvasElement | null>(null);
const fullCanvasRef = ref<HTMLCanvasElement | null>(null);
const expanded = ref(false);

// ===== 设置 =====
const spectrumOn = computed(() => !!settingsStore.audio.spectrumEnabled);
const spectrumStyle = computed(() => resolveSpectrumStyle(settingsStore.audio.spectrumStyle));
const activePalette = computed(
  () => settingsStore.audio.spectrumPalette || DEFAULT_SPECTRUM_PALETTE,
);
const colors = computed(() =>
  resolveSpectrumColors(
    activePalette.value,
    settingsStore.audio.spectrumColor1,
    settingsStore.audio.spectrumColor2,
  ),
);

const visible = computed(() => spectrumOn.value && !uiStore.showSettings && isSpectrumSupported());

const selectPalette = (id: string) => {
  settingsStore.update("audio.spectrumPalette", id);
};

// ===== 当前音源文案 =====
const fileNameOf = (url: string): string => {
  const name = decodeURIComponent(url.replace(/\\/g, "/").split("/").pop() || "");
  return name.replace(/\.[^/.]+$/, "") || name;
};

const nowPlayingText = computed(() => {
  const bgm = uiStore.currentBackgroundMusic;
  if (bgm && bgm !== "None" && !uiStore.bgMusicStoped) {
    return uiStore.bgMusicPaused
      ? t("game.spectrum.paused")
      : fileNameOf(bgm) || t("game.spectrum.title");
  }
  const ambient = uiStore.ambientTracks.filter((tr) => !tr.paused).length;
  if (ambient > 0) return t("game.spectrum.ambientCount", { n: ambient });
  return t("game.spectrum.idle");
});

// ===== 绘制 =====
const miniSpec = computed(() => specFor(spectrumStyle.value, false));
const fullSpec = computed(() => specFor(spectrumStyle.value, true));

/** 每个频带的平滑能量（0~1），快起慢落 */
const levels = new Float32Array(MAX_BARS);
let freqData = new Uint8Array(0);
let bandEdges = new Int32Array(0);
let bandSampleRate = 0;
/** 0 = 实时数据，1 = 静息呼吸动画（两者之间平滑过渡） */
let idleMix = 0;
let idleClock = 0;
let rafId = 0;
let lastFrame = 0;

/** 按对数分频铺频带：低频窄、高频宽，听感上更均匀 */
function ensureBands(binCount: number) {
  const sr = getSpectrumSampleRate();
  if (bandEdges.length === MAX_BARS + 1 && bandSampleRate === sr) return;
  const nyquist = sr / 2;
  const lo = 45;
  const hi = Math.min(13000, nyquist * 0.9);
  const edges = new Int32Array(MAX_BARS + 1);
  for (let i = 0; i <= MAX_BARS; i++) {
    const f = lo * Math.pow(hi / lo, i / MAX_BARS);
    edges[i] = Math.min(binCount - 1, Math.max(0, Math.round((f / nyquist) * binCount)));
  }
  // 保证每根柱子至少占一个频点
  for (let i = 1; i <= MAX_BARS; i++) {
    if (edges[i]! <= edges[i - 1]!) edges[i] = Math.min(binCount - 1, edges[i - 1]! + 1);
  }
  bandEdges = edges;
  bandSampleRate = sr;
}

function updateLevels(now: number) {
  const a = getSpectrumAnalyser();
  let peak = 0;
  if (a) {
    if (freqData.length !== a.frequencyBinCount) freqData = new Uint8Array(a.frequencyBinCount);
    a.getByteFrequencyData(freqData);
    ensureBands(a.frequencyBinCount);
    for (let i = 0; i < MAX_BARS; i++) {
      const start = bandEdges[i]!;
      const end = Math.max(start + 1, bandEdges[i + 1]!);
      let max = 0;
      for (let k = start; k < end && k < freqData.length; k++) {
        if (freqData[k]! > max) max = freqData[k]!;
      }
      // 高频能量天然偏弱，做一点倾斜补偿，否则只有左边几根跳
      const v = Math.min(1, (max / 255) * (1 + 0.6 * (i / (MAX_BARS - 1))));
      levels[i] = v > levels[i]! ? v : levels[i]! + (v - levels[i]!) * 0.18;
      if (levels[i]! > peak) peak = levels[i]!;
    }
  } else {
    for (let i = 0; i < MAX_BARS; i++) levels[i] = levels[i]! + (0 - levels[i]!) * 0.12;
  }
  idleClock = now;
  // 长时间没有能量 → 切到呼吸动画，避免看起来像卡死
  idleMix += ((peak < 0.035 ? 1 : 0) - idleMix) * 0.06;
}

/** 取第 i 根柱子的显示值（实时数据与静息动画按 idleMix 混合） */
function barValue(i: number, bars: number, wrap: boolean): number {
  const start = Math.round((i * MAX_BARS) / bars);
  const end = Math.max(start + 1, Math.round(((i + 1) * MAX_BARS) / bars));
  let v = 0;
  let n = 0;
  for (let k = start; k < end && k < MAX_BARS; k++) {
    v += levels[k]!;
    n++;
  }
  v /= Math.max(1, n);

  if (idleMix <= 0.001) return v;
  // 圆环首尾要接得上：空间相位取整数圈，接缝处才不会跳
  const p = i / bars;
  const t = idleClock * 0.0016;
  const spatial = wrap ? p * Math.PI * 4 : p * 6.2;
  const idle = Math.max(
    0.03,
    0.062 + 0.045 * Math.sin(spatial - t * 1.6) + 0.02 * Math.sin(p * 2.3 + t),
  );
  return v * (1 - idleMix) + idle * idleMix;
}

function roundRect(
  c: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const rr = Math.max(0, Math.min(r, w / 2, h / 2));
  c.beginPath();
  // 原生 roundRect 更省事，缺了就走 arcTo 兜底（旧 WebKitGTK）
  const native = c as unknown as {
    roundRect?: (x: number, y: number, w: number, h: number, r: number) => void;
  };
  if (typeof native.roundRect === "function") {
    native.roundRect(x, y, w, h, rr);
    return;
  }
  c.moveTo(x + rr, y);
  c.arcTo(x + w, y, x + w, y + h, rr);
  c.arcTo(x + w, y + h, x, y + h, rr);
  c.arcTo(x, y + h, x, y, rr);
  c.arcTo(x, y, x + w, y, rr);
  c.closePath();
}

function hexToRgb(hex: string): [number, number, number] {
  const m = /^#?([\da-f]{3}|[\da-f]{6})$/i.exec((hex || "").trim());
  if (!m) return [121, 217, 255];
  let h = m[1]!;
  if (h.length === 3)
    h = h
      .split("")
      .map((ch) => ch + ch)
      .join("");
  const n = parseInt(h, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** 两色插值（圆环沿圆周做色相过渡用） */
function mixColor(from: string, to: string, t: number): string {
  const a = hexToRgb(from);
  const b = hexToRgb(to);
  const mix = (x: number, y: number) => Math.round(x + (y - x) * t);
  return `rgb(${mix(a[0], b[0])}, ${mix(a[1], b[1])}, ${mix(a[2], b[2])})`;
}

/** 画布按 DPR 铺底，小尺寸下也不糊 */
function prepare(canvas: HTMLCanvasElement, spec: Spec): CanvasRenderingContext2D | null {
  const c = canvas.getContext("2d");
  if (!c) return null;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const bw = Math.round(spec.w * dpr);
  const bh = Math.round(spec.h * dpr);
  if (canvas.width !== bw || canvas.height !== bh) {
    canvas.width = bw;
    canvas.height = bh;
    canvas.style.width = `${spec.w}px`;
    canvas.style.height = `${spec.h}px`;
  }
  c.setTransform(dpr, 0, 0, dpr, 0, 0);
  c.clearRect(0, 0, spec.w, spec.h);
  return c;
}

/** 镜像 / 柱状：横向排布的胶囊柱 */
function drawLinear(c: CanvasRenderingContext2D, spec: Spec) {
  const { from, to } = colors.value;
  const grad = c.createLinearGradient(0, 0, spec.w, 0);
  grad.addColorStop(0, from);
  grad.addColorStop(1, to);

  const bars = spec.bars;
  const barW = (spec.w - spec.gap * (bars - 1)) / bars;
  // 四周留出辉光余量，别让外发光被画布裁掉
  const pad = spec.glow * 0.9;
  const midY = spec.h / 2;
  const halfMax = midY - pad;
  const fullMax = spec.h - pad * 2;
  const mirror = spec.kind === "mirror";

  // 镜像形态给一条极淡的中线，上下对称关系才看得清
  if (mirror) {
    c.globalAlpha = 0.12;
    c.fillStyle = grad;
    c.fillRect(0, midY - 0.5, spec.w, 1);
  }

  c.fillStyle = grad;
  c.shadowColor = from;
  c.shadowBlur = spec.glow;
  for (let i = 0; i < bars; i++) {
    const v = barValue(i, bars, false);
    const minLen = barW * 0.62;
    const x = i * (barW + spec.gap);
    c.globalAlpha = 0.34 + 0.66 * Math.min(1, v * 1.5);
    if (mirror) {
      const half = Math.max(minLen, Math.min(halfMax, v * halfMax));
      roundRect(c, x, midY - half, barW, half * 2, barW / 2);
    } else {
      const h = Math.max(minLen, Math.min(fullMax, v * fullMax));
      roundRect(c, x, spec.h - pad - h, barW, h, barW / 2);
    }
    c.fill();
  }
  c.globalAlpha = 1;
  c.shadowBlur = 0;
}

/** 圆环：绕圆周向外辐射的柱子，颜色沿圆周 from→to 过渡 */
function drawRing(c: CanvasRenderingContext2D, spec: Spec) {
  const { from, to } = colors.value;
  const cx = spec.w / 2;
  const cy = spec.h / 2;
  const bars = spec.bars;
  const r0 = spec.r0;
  const barW = Math.max(1, ((2 * Math.PI * r0) / bars) * 0.55);
  const minLen = barW * 0.7;

  // 基准圆环
  c.beginPath();
  c.arc(cx, cy, r0, 0, Math.PI * 2);
  c.strokeStyle = from;
  c.lineWidth = Math.max(0.6, barW * 0.35);
  c.globalAlpha = 0.16;
  c.stroke();

  c.shadowBlur = spec.glow;
  for (let i = 0; i < bars; i++) {
    const v = barValue(i, bars, true);
    const len = Math.max(minLen, Math.min(spec.len, v * spec.len));
    const color = mixColor(from, to, i / bars);
    const angle = -Math.PI / 2 + (i / bars) * Math.PI * 2;
    c.save();
    c.translate(cx, cy);
    c.rotate(angle);
    c.globalAlpha = 0.34 + 0.66 * Math.min(1, v * 1.5);
    c.fillStyle = color;
    c.shadowColor = color;
    roundRect(c, -barW / 2, -(r0 + len), barW, len, barW / 2);
    c.fill();
    c.restore();
  }
  c.globalAlpha = 1;
  c.shadowBlur = 0;
}

function draw(canvas: HTMLCanvasElement | null, spec: Spec) {
  if (!canvas) return;
  const c = prepare(canvas, spec);
  if (!c) return;
  if (spec.kind === "ring") drawRing(c, spec);
  else drawLinear(c, spec);
}

function frame(now: number) {
  rafId = requestAnimationFrame(frame);
  if (!visible.value || document.hidden || now - lastFrame < FRAME_MS) return;
  lastFrame = now;
  updateLevels(now);
  if (expanded.value) draw(fullCanvasRef.value, fullSpec.value);
  draw(miniCanvasRef.value, miniSpec.value);
}

// ===== 开关联动 =====
watch(spectrumOn, (on) => setSpectrumEnabled(on), { immediate: true });
// 设置面板打开时隐藏，与左侧声效按钮一致；顺手收起展开面板
watch(visible, (v) => {
  if (!v) expanded.value = false;
});

// 点击外部收起
const onDocMouseDown = (e: MouseEvent) => {
  if (!expanded.value) return;
  if (rootRef.value?.contains(e.target as Node)) return;
  expanded.value = false;
};

onMounted(() => {
  document.addEventListener("mousedown", onDocMouseDown);
  rafId = requestAnimationFrame(frame);
});

onUnmounted(() => {
  document.removeEventListener("mousedown", onDocMouseDown);
  cancelAnimationFrame(rafId);
});
</script>
