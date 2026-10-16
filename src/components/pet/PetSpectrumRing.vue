<template>
  <!-- 头像圆框外圈的音频频谱。形态固定为圆环（配置项里的 spectrumStyle 在这里不生效），
       配色/自定义颜色与主界面共用同一套设置。 -->
  <canvas
    v-if="visible"
    ref="canvasRef"
    class="pointer-events-none absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2"
  ></canvas>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { DEFAULT_SPECTRUM_PALETTE, resolveSpectrumColors } from "@/constants/spectrum";
import { useSettingsStore } from "@/stores/modules/settings";
import { isSpectrumSupported, setSpectrumEnabled } from "@/utils/audioSpectrum";
import { SpectrumRenderer, type SpectrumSpec } from "@/utils/spectrumRender";
import {
  PET_SPECTRUM_BARS,
  PET_SPECTRUM_BAR_RATIO,
  PET_SPECTRUM_CANVAS_BASE,
  PET_SPECTRUM_GLOW_BASE,
  PET_SPECTRUM_LEN_BASE,
  PET_SPECTRUM_R0_BASE,
  PET_SPECTRUM_SMOOTHING,
} from "./constants";

const settingsStore = useSettingsStore();
const canvasRef = ref<HTMLCanvasElement | null>(null);
// 频带数跟着柱数走，一根柱对一条频带（否则相邻柱会取到同一条频带，看起来是变宽不是变密）
const renderer = new SpectrumRenderer(PET_SPECTRUM_BARS, PET_SPECTRUM_SMOOTHING);

const spectrumOn = computed(() => !!settingsStore.audio.spectrumEnabled);
const colors = computed(() =>
  resolveSpectrumColors(
    settingsStore.audio.spectrumPalette || DEFAULT_SPECTRUM_PALETTE,
    settingsStore.audio.spectrumColor1,
    settingsStore.audio.spectrumColor2,
  ),
);
const visible = computed(() => spectrumOn.value && isSpectrumSupported());

const spec = computed<SpectrumSpec>(() => {
  const s = settingsStore.pet?.scale || 1;
  return {
    kind: "ring",
    w: PET_SPECTRUM_CANVAS_BASE * s,
    h: PET_SPECTRUM_CANVAS_BASE * s,
    bars: PET_SPECTRUM_BARS,
    barWidthRatio: PET_SPECTRUM_BAR_RATIO,
    // 折半镜像：频带铺满一圈时低频占半圈、高频占半圈，而音乐能量集中在低频，
    // 不折的话下半圈是空的
    foldSpectrum: true,
    gap: 0,
    glow: PET_SPECTRUM_GLOW_BASE * s,
    r0: PET_SPECTRUM_R0_BASE * s,
    len: PET_SPECTRUM_LEN_BASE * s,
    // 没有能量就什么都不画（不留柱底、不走静息呼吸、不画基准圆）
    bare: true,
  };
});

// 30 帧：桌宠窗常驻置顶，与默认的 Live2D 帧率同档就够了，不必跟主界面一样 40 帧
const FRAME_MS = 1000 / 30;
let rafId = 0;
let lastFrame = 0;

function frame(now: number) {
  rafId = requestAnimationFrame(frame);
  if (!visible.value || document.hidden || now - lastFrame < FRAME_MS) return;
  lastFrame = now;
  renderer.update(now);
  renderer.draw(canvasRef.value, spec.value, colors.value);
}

// 桌宠下没有 SpectrumVisualizer，由本组件负责打开采集（幂等，见 utils/audioSpectrum.ts）
watch(spectrumOn, (on) => setSpectrumEnabled(on), { immediate: true });

onMounted(() => {
  rafId = requestAnimationFrame(frame);
});

onUnmounted(() => cancelAnimationFrame(rafId));
</script>
