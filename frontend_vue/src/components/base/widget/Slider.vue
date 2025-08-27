<template>
  <div>
    <span><slot name="left">{{ leftLabel }}</slot></span>
    <input
      type="range"
      :min="min"
      :max="max"
      :step="step"
      :value="value"
      @input="$emit('input', Number(value))"
      @change="$emit('change', Number(value))"
      v-model="value"
    />
    <span><slot name="right">{{ rightLabel }}</slot></span>
  </div>
</template>

<script setup lang="ts">

// 导入外部模块
import { useSlots, computed, ref, Slots } from "vue";

// 定义组件属性
const props = defineProps({
  value: {
    type: Number,
    require: false,
  },
  min: {
    type: Number,
    default: 0,
  },
  max: {
    type: Number,
    default: 100,
  },
  step: {
    type: Number,
    default: 1,
  },
});


// 定义组件事件
const emit = defineEmits([
  "change",
  "input",
]);

// 定义动态变量
const value = ref()

// 获取插槽内容
const slots: Slots = useSlots();

// 处理组件行为

// 设置滑块默认值为中值
value.value = (props.min + props.max) /2

// 分别设置滑块两端内容
const leftLabel = computed(() => {
  if (slots.default) {
    const defaultSlot = slots.default();
    if (
      defaultSlot &&
      defaultSlot[0] &&
      typeof defaultSlot[0].children === "string"
    ) {
      const parts = defaultSlot[0].children.split("/");
      return parts[0] || "";
    }
  }
  return "";
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
  gap: 15px;
  display: flex;
  align-items: center;
  color: rgba(255, 255, 255, 0.9); /* 白色文字 */
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

input[type="range"] {
  flex-grow: 1;
  outline: none;
  margin: 10px 0;
  appearance: none;
  position: relative;
  -webkit-appearance: none;
  background-color: transparent;
}

input[type="range"]::-webkit-slider-runnable-track {
  width: 100%;
  height: 8px;
  border-radius: 4px;
  transition: all 0.3s ease;
  background: rgba(255, 255, 255, 0.2); /* 透明白色轨道 */
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.2);

}

input[type="range"]:hover::-webkit-slider-runnable-track {
  height: 10px;
  background: rgba(255, 255, 255, 0.25);
}

input[type="range"]::-webkit-slider-thumb {
  z-index: 2;
  width: 22px;
  height: 22px;
  cursor: grab;
  appearance: none;
  margin-top: -7px;
  position: relative;
  border-radius: 50%;
  -webkit-appearance: none;
  transform-origin: center;
  border: 2px solid rgba(255, 255, 255, 0.8); /* 半透明白色边框 */
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  background: linear-gradient( 135deg, var(--accent-color), #64b5f6 ); /* 渐变背景 */
  box-shadow: 0 4px 12px rgba(121, 217, 255, 0.4), 0 2px 4px rgba(0, 0, 0, 0.2);
}

input[type="range"]::-webkit-slider-thumb:hover,
input[type="range"]::-webkit-slider-thumb:active {
  transform: scale(1.15);
  background: linear-gradient( 135deg, #64b5f6, var(--accent-color));
  box-shadow: 0 6px 20px rgba(121, 217, 255, 0.6), 0 4px 8px rgba(0, 0, 0, 0.3);
}

input[type="range"]:active::-webkit-slider-thumb {
  cursor: grabbing;
}

</style>
