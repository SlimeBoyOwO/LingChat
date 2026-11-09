<script setup lang="ts">
/**
 * 滑块 + 数字框：稿上的标准参数行。
 *
 * 两条硬规则（来自沙箱）：数字框吃非法输入要回滚而不是写 NaN；
 * 双击标签恢复「当前预设」的默认值，不是硬编码默认值。
 */
import { computed, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: number;
    label: string;
    min: number;
    max: number;
    step?: number;
    /** 显示精度（小数位） */
    precision?: number;
    /** 双击标签时恢复到的值；不传则标签不可重置 */
    defaultValue?: number;
    hint?: string;
  }>(),
  { step: 0.01, precision: 2 },
);

const emit = defineEmits<{ (e: "update:modelValue", v: number): void }>();

function fmt(v: number): string {
  return Number(v).toFixed(props.precision);
}

const text = ref(fmt(props.modelValue));

watch(
  () => props.modelValue,
  (v) => {
    // 只同步「数值真的变了」的情况，否则会把用户正在输入的小数位吃掉
    if (Number(text.value) !== Number(fmt(v))) text.value = fmt(v);
  },
);

const fill = computed(() => {
  const span = props.max - props.min;
  const pct = span <= 0 ? 0 : ((props.modelValue - props.min) / span) * 100;
  return `${Math.max(0, Math.min(100, pct))}%`;
});

function commit(raw: number) {
  if (!Number.isFinite(raw)) return;
  const clamped = Math.min(props.max, Math.max(props.min, raw));
  const next = Number(clamped.toFixed(4));
  if (next === props.modelValue) return;
  emit("update:modelValue", next);
}

function onRange(e: Event) {
  commit(Number((e.target as HTMLInputElement).value));
}

function onTextInput() {
  const raw = text.value.trim();
  if (raw === "" || raw === "-" || raw === ".") return;
  const n = Number(raw);
  if (!Number.isFinite(n)) return;
  commit(n);
}

function onTextBlur() {
  text.value = fmt(props.modelValue);
}

function reset() {
  if (props.defaultValue === undefined) return;
  emit("update:modelValue", props.defaultValue);
}
</script>

<template>
  <div class="wb-param">
    <div class="wb-param-top">
      <span
        class="wb-param-label wb-ellipsis"
        :class="{ 'is-resettable': defaultValue !== undefined }"
        :title="hint || label"
        @dblclick="reset"
        >{{ label }}</span
      >
      <span class="wb-param-value">
        <input
          v-model="text"
          class="wb-num"
          inputmode="decimal"
          @input="onTextInput"
          @blur="onTextBlur"
          @keydown.enter="onTextBlur"
        />
      </span>
    </div>
    <input
      class="wb-slider"
      type="range"
      :min="min"
      :max="max"
      :step="step"
      :value="modelValue"
      :style="{ '--wb-fill': fill }"
      @input="onRange"
    />
  </div>
</template>
