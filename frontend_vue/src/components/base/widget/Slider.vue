<template>
  <div>
    <span>{{ leftLabel }}</span>
    <input
      type="range"
      :min="min"
      :max="max"
      :step="step"
      @input="input"
      @change="change"
    >
    <span>{{ rightLabel }}</span>
  </div>
</template>

<script setup lang="ts">
import { useSlots, computed } from 'vue'
import type { VNode, Slots } from 'vue'
// 处理组件属性
const props = defineProps({
  min: { type: Number, default: 0 },
  max: { type: Number, default: 100 },
  step: { type: Number, default: 1 },
})

// 处理组件事件
const emit = defineEmits(['input', 'change'])
const input = (event: Event) => {
  const target = event.target as HTMLInputElement
  emit('input', Number(target.value))

}
const change = (event: Event) => {
  const target = event.target as HTMLInputElement
  emit('change', Number(target.value))
}

// 处理组件内容
const slots: Slots = useSlots()
const defaultSlot: any = slots.default?.()
const slotContent = defaultSlot[0].children
const slotLabels = slotContent.split('/');
const leftLabel = computed(() => {return slotLabels[0]})
const rightLabel = computed(() => {return slotLabels[1]})

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
    background: linear-gradient(135deg, var(--accent-color), #64b5f6); /* 渐变背景，不再是白色 */
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
    background: linear-gradient(135deg, #64b5f6, var(--accent-color)); /* 悬停时反转渐变 */
}

input[type="range"]:active::-webkit-slider-thumb {
    cursor: grabbing;
}
</style>