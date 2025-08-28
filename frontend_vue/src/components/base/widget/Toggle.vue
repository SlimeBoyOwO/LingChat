<template>
  <div>
    <input 
      type="checkbox"
      :id="id"
      :checked="checked"
      @change="$emit('change', value)"
      v-model="value"
    >
    <label :for="id">
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
const emit = defineEmits([
  'change'
])

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

div {
  display: flex;
  align-items: center;
}

input {
  display: none;
}

label {
  color: white;
  font-size: 14px;
  cursor: pointer;
  position: relative;
  padding-left: 60px;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

label::before {
  left: 0;
  top: 50%;
  content: '';
  width: 50px;
  height: 26px;
  position: absolute;
  border-radius: 13px;
  transition: all 0.3s ease;
  backdrop-filter: blur(5px);
  transform: translateY(-50%);
  border: 1px solid rgba(255, 255, 255, 0.3);
  background-color: rgba(255, 255, 255, 0.2);
}

label::after {
  top: 50%;
  left: 4px;
  content: '';
  width: 20px;
  height: 20px;
  position: absolute;
  border-radius: 50%;
  transition: all 0.3s ease;
  transform: translateY(-50%);
  background: linear-gradient(135deg, #ffffff, #f0f0f0);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3), 0 1px 2px rgba(0, 0, 0, 0.1);
}

input:checked + label::before {
  border-color: var(--accent-color);
  background-color: rgba(121, 217, 255, 0.3);
  box-shadow: 0 0 10px rgba(121, 217, 255, 0.3);
}

input:checked + label::after {
  left: 26px;
  background: linear-gradient(135deg, var(--accent-color), #64b5f6);
  box-shadow: 0 3px 8px rgba(121, 217, 255, 0.4), 0 1px 3px rgba(0, 0, 0, 0.2);
}

</style>