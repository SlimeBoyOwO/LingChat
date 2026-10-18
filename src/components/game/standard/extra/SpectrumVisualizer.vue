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
import { isSpectrumSupported, setSpectrumEnabled, spectrumActive } from "@/utils/audioSpectrum";
import { SpectrumRenderer, type SpectrumSpec } from "@/utils/spectrumRender";
import { AudioLines } from "@lucide/vue";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const uiStore = useUIStore();
const settingsStore = useSettingsStore();
const { t } = useI18n();

/**
 * 每个形态的尺寸。
 * 画布要留出辉光的余量（glowPadding），否则柱子外发光会被画布边缘裁成硬边
 * ——没有外壳遮挡时这道硬边很显眼。
 * - 镜像/柱状是宽扁的，迷你态 88×32（加按钮 4px 内边距 = 40，与左侧"声效"按钮等高）
 * - 圆环要方正，迷你态 48×48，展开态 132×132（居中）
 */
function specFor(style: SpectrumStyle, big: boolean): SpectrumSpec {
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

// 采集与绘制都在 utils/spectrumRender.ts（桌宠头像外圈的圆环频谱共用同一份）
const renderer = new SpectrumRenderer();
let rafId = 0;
let lastFrame = 0;

function frame(now: number) {
  rafId = requestAnimationFrame(frame);
  if (!visible.value || document.hidden || now - lastFrame < FRAME_MS) return;
  lastFrame = now;
  renderer.update(now);
  if (expanded.value) renderer.draw(fullCanvasRef.value, fullSpec.value, colors.value);
  renderer.draw(miniCanvasRef.value, miniSpec.value, colors.value);
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
