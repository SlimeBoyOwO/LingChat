<template>
  <div>
    <span
      ><slot name="left">{{ leftLabel }}</slot></span
    >
    <input
      type="range"
      :min="min"
      :max="max"
      :step="step"
      v-model="value"
      @change="$emit('change', Number(value))"
    />
    <span
      ><slot name="right">{{ rightLabel }}</slot></span
    >
  </div>
</template>

<script setup lang="ts">
// 导入外部模块
import { useSlots, computed, ref, watch } from 'vue'
import type { Slots } from 'vue'

// 定义组件属性
const props = defineProps({
  // 双向绑定值
  modelValue: {
    type: [Number, String],
    default: 0,
  },
  // 最小值
  min: {
    type: [Number, String],
    default: 0,
  },
  // 最大值
  max: {
    type: [Number, String],
    default: 100,
  },
  // 步长
  step: {
    type: [Number, String],
    default: 1,
  },
  // 是否禁用
  disabled: {
    type: Boolean,
    default: false,
  },
  // 自定义CSS变量（用于滑块颜色）
  accentColor: {
    type: String,
    default: '#4fc3f7',
  },
})

// 定义组件事件
const emit = defineEmits(['change', 'input'])

// 获取插槽内容
const slots: Slots = useSlots()

// 使用计算属性处理v-model绑定
const value = ref(Number(props.modelValue))

// 分别设置滑块两端内容
const leftLabel = computed(() => {
  if (slots.default) {
    const defaultSlot = slots.default()
    if (defaultSlot && defaultSlot[0] && typeof defaultSlot[0].children === 'string') {
      const parts = defaultSlot[0].children.split('/')
      return parts[0] || ''
    }
  }
  return ''
})
const rightLabel = computed(() => {
  if (slots.default) {
    const defaultSlot = slots.default()
    if (defaultSlot && defaultSlot[0] && typeof defaultSlot[0].children === 'string') {
      const parts = defaultSlot[0].children.split('/')
      return parts[1] || ''
    }
  }
  return ''
})
</script>

<style scoped>
div {
  gap: 15px;
  display: flex;
  align-items: center;
  color: rgba(255, 255, 255, 0.9); /* 白色文字 */
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

input[type='range'] {
  flex-grow: 1;
  outline: none;
  margin: 10px 0;
  appearance: none;
  position: relative;
  -webkit-appearance: none;
  background-color: transparent;
}

input[type='range']::-webkit-slider-runnable-track {
  width: 100%;
  height: 8px;
  border-radius: 4px;
  transition: all 0.3s ease;
  background: rgba(255, 255, 255, 0.2); /* 透明白色轨道 */
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.2);
}

input[type='range']:hover::-webkit-slider-runnable-track {
  height: 10px;
  background: rgba(255, 255, 255, 0.25);
}

input[type='range']::-webkit-slider-thumb {
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
  background: linear-gradient(135deg, var(--accent-color), #64b5f6); /* 渐变背景 */
  box-shadow:
    0 4px 12px rgba(121, 217, 255, 0.4),
    0 2px 4px rgba(0, 0, 0, 0.2);
}

input[type='range']::-webkit-slider-thumb:hover,
input[type='range']::-webkit-slider-thumb:active {
  transform: scale(1.15);
  background: linear-gradient(135deg, #64b5f6, var(--accent-color));
  box-shadow:
    0 6px 20px rgba(121, 217, 255, 0.6),
    0 4px 8px rgba(0, 0, 0, 0.3);
}

input[type='range']:active::-webkit-slider-thumb {
  cursor: grabbing;
}
</style>
