<template>
  <div class="slider-wrapper">
    <span
      ><slot name="left">{{ leftLabel }}</slot></span
    >
    <input
      type="range"
      :min="min"
      :max="max"
      :step="step"
      :value="modelValue ? modelValue : max / 2"
      @input="onInput"
      @change="onChange"
    />
    <span
      ><slot name="right">{{ rightLabel }}</slot></span
    >
  </div>
</template>

<script setup lang="ts">
import { useSlots, computed } from "vue";
import type { VNode, Slots } from "vue";

// 1. 添加 modelValue 属性来接收 v-model 的值
const props = defineProps({
  modelValue: { type: Number, required: false }, // 接收当前值
  min: { type: Number, default: 0 },
  max: { type: Number, default: 100 },
  step: { type: Number, default: 1 },
});

// 2. 修改 emit，添加 update:modelValue
const emit = defineEmits(["update:modelValue", "change"]);

const onInput = (event: Event) => {
  const target = event.target as HTMLInputElement;
  // 触发 update:modelValue 事件以更新 v-model
  emit("update:modelValue", Number(target.value));
};

const onChange = (event: Event) => {
  const target = event.target as HTMLInputElement;
  emit("change", Number(target.value));
};

// --- Slot 内容处理的改进建议 (更稳健) ---
// 您原来的方法在某些情况下可能会出错，例如当插槽内容复杂时。
// 下面的方法是更安全的选择。
const slots: Slots = useSlots();
const leftLabel = computed(() => {
  // 检查默认插槽是否存在
  if (slots.default) {
    const defaultSlot = slots.default();
    // 确保内容是纯文本
    if (
      defaultSlot &&
      defaultSlot[0] &&
      typeof defaultSlot[0].children === "string"
    ) {
      return defaultSlot[0].children.split("/")[0] || "";
    }
  }
  return ""; // 如果没有提供，则返回空字符串
});

const rightLabel = computed(() => {
  if (slots.default) {
    const defaultSlot = slots.default();
    if (
      defaultSlot &&
      defaultSlot[0] &&
      typeof defaultSlot[0].children === "string"
    ) {
      const parts = defaultSlot[0].children.split("/");
      return parts[1] || "";
    }
  }
  return "";
});
</script>

<style scoped>
div {
  display: flex;
  align-items: center;
  gap: 15px;
  color: rgba(255, 255, 255, 0.9); /* 白色文字 */
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

input[type="range"] {
  -webkit-appearance: none;
  appearance: none;
  flex-grow: 1;
  margin: 10px 0;
  background-color: transparent;
  position: relative;
  outline: none;
}

input[type="range"]::-webkit-slider-runnable-track {
  width: 100%;
  height: 8px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.2); /* 透明白色轨道 */
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.2);
  transition: all 0.3s ease;
}

input[type="range"]:hover::-webkit-slider-runnable-track {
  height: 10px;
  background: rgba(255, 255, 255, 0.25);
}

input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 22px;
  height: 22px;
  background: linear-gradient(
    135deg,
    var(--accent-color),
    #64b5f6
  ); /* 渐变背景，不再是白色 */
  border-radius: 50%;
  cursor: grab;
  border: 2px solid rgba(255, 255, 255, 0.8); /* 半透明白色边框 */
  margin-top: -7px;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  transform-origin: center;
  box-shadow: 0 4px 12px rgba(121, 217, 255, 0.4), 0 2px 4px rgba(0, 0, 0, 0.2); /* 增强阴影效果 */
  position: relative;
  z-index: 2;
}

input[type="range"]::-webkit-slider-thumb:hover,
input[type="range"]::-webkit-slider-thumb:active {
  transform: scale(1.15);
  box-shadow: 0 6px 20px rgba(121, 217, 255, 0.6), 0 4px 8px rgba(0, 0, 0, 0.3);
  background: linear-gradient(
    135deg,
    #64b5f6,
    var(--accent-color)
  ); /* 悬停时反转渐变 */
}

input[type="range"]:active::-webkit-slider-thumb {
  cursor: grabbing;
}
</style>
