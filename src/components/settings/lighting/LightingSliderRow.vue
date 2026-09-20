<template>
  <div class="flex items-center gap-2">
    <span class="w-[5.5rem] shrink-0 text-[11px] text-white/50">{{ label }}</span>
    <input
      type="range"
      class="lighting-range min-w-0 flex-1"
      :class="{ 'opacity-40': disabled }"
      :style="{ '--accent-color': color }"
      :min="min"
      :max="max"
      :step="step"
      :value="modelValue"
      :disabled="disabled"
      @input="onInput"
    />
    <!-- 数值必须显式回显：调的是亮度还是强度，光看滑块位置猜不出来 -->
    <span class="w-12 shrink-0 text-right text-[11px] text-white/60 tabular-nums">
      {{ display }}
    </span>
  </div>
</template>

<script setup lang="ts">
  import { computed } from "vue";

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

  function onInput(e: Event) {
    emit("update:modelValue", Number((e.target as HTMLInputElement).value));
  }

  const display = computed(() => `${props.modelValue.toFixed(props.decimals)}${props.unit}`);
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
