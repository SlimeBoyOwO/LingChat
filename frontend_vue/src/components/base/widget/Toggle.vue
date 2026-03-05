<template>
  <div class="flex items-center">
    <input
      type="checkbox"
      :id="id"
      :checked="checked"
      @change="$emit('change', value)"
      v-model="value"
      class="hidden peer"
    />
    <label
      :for="id"
      class="relative pl-[60px] cursor-pointer text-white text-[14px] select-none"
      style="text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3)"
    >
      <span
        class="absolute left-0 top-1/2 -translate-y-1/2 w-[50px] h-[26px] rounded-[13px] transition-all duration-300 ease-in-out backdrop-blur-[5px] border border-white/30 bg-white/20 before:content-[''] after:content-[''] peer-checked:border-[var(--accent-color)] peer-checked:bg-[rgba(121,217,255,0.3)] peer-checked:shadow-[0_0_10px_rgba(121,217,255,0.3)]"
      ></span>
      <span
        class="absolute top-1/2 left-[4px] -translate-y-1/2 w-[20px] h-[20px] rounded-full transition-all duration-300 ease-in-out bg-gradient-to-br from-white to-[#f0f0f0] shadow-[0_2px_6px_rgba(0,0,0,0.3),0_1px_2px_rgba(0,0,0,0.1)] peer-checked:left-[26px] peer-checked:bg-gradient-to-br peer-checked:from-[var(--accent-color)] peer-checked:to-[#64b5f6] peer-checked:shadow-[0_3px_8px_rgba(121,217,255,0.4),0_1px_3px_rgba(0,0,0,0.2)]"
      ></span>
      <slot></slot>
    </label>
  </div>
</template>

<script setup lang="ts">
// 导入外部模块
import { ref, onMounted } from 'vue'

// 定义组件属性
const props = defineProps({
  checked: {
    type: Boolean,
    default: false,
  },
})

// 定义组件事件
const emit = defineEmits(['change'])

// 定义动态变量
const id = ref()
const value = ref()

// 处理组件行为

// 挂载组件生成唯一标识
onMounted(() => {
  id.value = Math.random().toString(36).substring(2, 9)
})
</script>

<style scoped>
/* 保留CSS变量引用 */
:deep(*) {
  --accent-color: var(--accent-color);
}
</style>
