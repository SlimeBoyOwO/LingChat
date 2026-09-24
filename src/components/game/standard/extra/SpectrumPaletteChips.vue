<template>
  <!-- 频谱配色色块：迷你胶囊的展开面板与声效面板共用 -->
  <div class="flex flex-wrap items-center gap-1.5">
    <button
      v-for="p in SPECTRUM_PALETTES"
      :key="p.id"
      type="button"
      class="h-4 w-4 shrink-0 cursor-pointer rounded-full border transition-all duration-200 hover:scale-110"
      :class="
        modelValue === p.id ? 'scale-110 border-white/90' : 'border-white/20 hover:border-white/50'
      "
      :style="{ background: `linear-gradient(135deg, ${p.from}, ${p.to})` }"
      :title="p.label"
      @click="$emit('update:modelValue', p.id)"
    ></button>

    <!-- 自定义：色块直接用当前的自定义颜色 -->
    <button
      type="button"
      class="h-4 w-4 shrink-0 cursor-pointer rounded-full border transition-all duration-200 hover:scale-110"
      :class="
        modelValue === CUSTOM_SPECTRUM_PALETTE
          ? 'scale-110 border-white/90'
          : 'border-white/20 hover:border-white/50'
      "
      :style="{
        background: `conic-gradient(from 210deg, ${resolvedFrom}, ${resolvedTo}, ${resolvedFrom})`,
      }"
      :title="$t('game.soundPanel.spectrum.custom')"
      @click="$emit('update:modelValue', CUSTOM_SPECTRUM_PALETTE)"
    ></button>
  </div>
</template>

<script setup lang="ts">
import {
  CUSTOM_SPECTRUM_PALETTE,
  DEFAULT_SPECTRUM_COLOR_FROM,
  DEFAULT_SPECTRUM_COLOR_TO,
  SPECTRUM_PALETTES,
} from "@/constants/spectrum";
import { computed } from "vue";

const props = defineProps<{
  /** 当前配色 id */
  modelValue: string;
  /** 自定义配色的两个颜色（用于预览色块） */
  customFrom?: string;
  customTo?: string;
}>();

defineEmits<{ (e: "update:modelValue", id: string): void }>();

const resolvedFrom = computed(() => props.customFrom || DEFAULT_SPECTRUM_COLOR_FROM);
const resolvedTo = computed(() => props.customTo || DEFAULT_SPECTRUM_COLOR_TO);
</script>
