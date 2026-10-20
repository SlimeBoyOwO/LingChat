<template>
  <textarea
    ref="inputRef"
    :value="modelValue"
    :rows="expanded ? rows : 1"
    class="block resize-none transition-colors duration-200"
    :class="expanded ? 'overflow-y-auto' : 'overflow-hidden'"
    @input="onInput"
    @focus="expanded = true"
    @blur="onBlur"
  ></textarea>
</template>

<script setup lang="ts">
import { ref } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    rows?: number;
  }>(),
  { rows: 4 },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const inputRef = ref<HTMLTextAreaElement>();
const expanded = ref(false);

// 用 canvas 按当前字号量文本宽度，判断失焦后能否收回单行
let measureCtx: CanvasRenderingContext2D | null = null;
function fitsOneLine(el: HTMLTextAreaElement, text: string): boolean {
  if (!text || text.includes("\n")) return false;
  const style = getComputedStyle(el);
  if (!measureCtx) measureCtx = document.createElement("canvas").getContext("2d");
  if (!measureCtx) return text.length <= 16;
  measureCtx.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
  const available =
    el.clientWidth - parseFloat(style.paddingLeft || "0") - parseFloat(style.paddingRight || "0");
  return measureCtx.measureText(text).width <= available;
}

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLTextAreaElement).value);
}

function onBlur() {
  const el = inputRef.value;
  if (el && fitsOneLine(el, el.value)) expanded.value = false;
}
</script>
