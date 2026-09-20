<template>
  <div class="flex items-center gap-2">
    <span class="w-[5.5rem] shrink-0 text-[11px] text-white/50">{{ label }}</span>
    <input
      ref="sliderRef"
      type="range"
      class="lighting-range min-w-0 flex-1"
      :class="{ 'opacity-40': disabled }"
      :style="{ '--accent-color': color }"
      :min="min"
      :max="max"
      :step="step"
      :value="modelValue"
      :disabled="disabled"
      @input="onSlide"
      @pointerup="releaseSlider"
    />
    <!-- 数值既能看又能敲：显示位做成输入框，精确值直接键入，↑↓ 微调。
         刻意不用 type="number"，它自带滚轮步进，一滚列表就把值改了。 -->
    <input
      ref="textRef"
      v-model="text"
      type="text"
      inputmode="decimal"
      class="w-14 shrink-0 rounded border border-transparent bg-transparent px-1 py-0.5 text-right
        text-[11px] text-white/60 tabular-nums transition-colors hover:border-white/15
        hover:text-white/80 focus:border-amber-400/60 focus:bg-black/30 focus:text-white
        focus:outline-none disabled:opacity-40"
      :disabled="disabled"
      @focus="onFocus"
      @blur="commit"
      @keydown.enter.prevent="commit"
      @keydown.esc.prevent="cancel"
      @keydown.up.prevent="nudge(1, $event)"
      @keydown.down.prevent="nudge(-1, $event)"
    />
  </div>
</template>

<script setup lang="ts">
  import { nextTick, ref, watch } from "vue";

  const props = withDefaults(
    defineProps<{
      modelValue: number;
      label: string;
      min: number;
      max: number;
      step?: number;
      unit?: string;
      decimals?: number;
      color?: string;
      disabled?: boolean;
    }>(),
    { step: 1, unit: "", decimals: 0, color: "#f59e0b", disabled: false }
  );

  const emit = defineEmits<{ "update:modelValue": [value: number] }>();

  const sliderRef = ref<HTMLInputElement | null>(null);
  const textRef = ref<HTMLInputElement | null>(null);
  const editing = ref(false);
  const text = ref(format(props.modelValue));

  watch(
    () => props.modelValue,
    (v) => {
      if (!editing.value) text.value = format(v);
    }
  );

  function format(v: number): string {
    return `${v.toFixed(props.decimals)}${props.unit}`;
  }

  function parse(raw: string): number | null {
    const cleaned = raw.replace(/[^0-9.+\-]/g, "");
    const n = Number.parseFloat(cleaned);
    if (!Number.isFinite(n)) return null;
    const clamped = Math.min(props.max, Math.max(props.min, n));
    const factor = 10 ** props.decimals;
    return Math.round(clamped * factor) / factor;
  }

  function apply(v: number | null): void {
    if (v === null) {
      text.value = format(props.modelValue);
      return;
    }
    emit("update:modelValue", v);
    text.value = format(v);
  }

  // 拖完就散焦：Chromium 里被点过的 range 会吃滚轮，用户在列表里往下滚就会
  // 顺手把这格数值改掉。滚轮只该负责滚动，要微调请用输入框或方向键。
  function releaseSlider(): void {
    sliderRef.value?.blur();
  }

  function onSlide(e: Event): void {
    emit("update:modelValue", Number((e.target as HTMLInputElement).value));
  }

  function onFocus(): void {
    editing.value = true;
    text.value = String(props.modelValue);
    void nextTick(() => textRef.value?.select());
  }

  function commit(): void {
    editing.value = false;
    apply(parse(text.value));
  }

  // 这里不抢焦点：Esc 只是把内容弹回当前值，紧接着的 blur 会走 commit，
  // 而回弹后的文本本来就等于当前值，所以不会有副作用。
  function cancel(): void {
    editing.value = false;
    text.value = format(props.modelValue);
  }

  function nudge(dir: number, e: KeyboardEvent): void {
    const scale = 10 ** props.decimals;
    const base = parse(text.value) ?? props.modelValue;
    const delta = props.step * (e.shiftKey ? 10 : 1) * dir;
    // 先放大再取整，避开 0.1+0.2 这类浮点尾巴
    apply(Math.round((base + delta) * scale) / scale);
  }
</script>

<style scoped>
  /* 与场景编辑器同一套 range 外观 */
  .lighting-range {
    -webkit-appearance: none;
    appearance: none;
    height: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.15);
    outline: none;
    cursor: pointer;
  }

  .lighting-range::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--accent-color, #f59e0b);
    border: 2px solid rgba(255, 255, 255, 0.8);
    cursor: grab;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .lighting-range::-webkit-slider-thumb:active {
    cursor: grabbing;
    transform: scale(1.15);
  }
</style>
