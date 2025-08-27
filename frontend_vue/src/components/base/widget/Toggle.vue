<template>
  <div>
    <input 
      type="checkbox"
      :id="id"
      :checked="default"
      @change="change"
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
  default: {
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

onMounted(() => {
  id.value = Math.random().toString(36).substring(2, 9)
})

function change() {
  emit('change', value.value)
}

</script>

<style scoped>

div {

  /* 布局位置 */
  display: flex;
  align-items: center;

}

input {

  /* 位置布局 */
  display: none;

}

label {

  /* 文字效果 */
  color: white;
  font-size: 14px;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);

  /* 位置布局 */
  position: relative;
  padding-left: 60px;

  /* 用户提示 */
  cursor: pointer;

}

label::before {

  /* 位置布局 */
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);

  /* 盒子模型 */
  width: 50px;
  height: 26px;
  border: 1px solid rgba(255, 255, 255, 0.3);
  
  /* 修饰元素 */
  content: '';
  border-radius: 13px;
  background-color: rgba(255, 255, 255, 0.2);
  backdrop-filter: blur(5px);

  /* 过渡动画 */
  transition: all 0.3s ease;

}

label::after {

  /* 位置布局 */
  position: absolute;
  left: 4px;
  top: 50%;
  transform: translateY(-50%);

  /* 盒子模型 */
  width: 20px;
  height: 20px;
  
  /* 修饰元素 */
  content: '';
  border-radius: 50%;
  background: linear-gradient(135deg, #ffffff, #f0f0f0);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3), 0 1px 2px rgba(0, 0, 0, 0.1);
  
  /* 过度动画 */
  transition: all 0.3s ease;
}

input:checked + label::before {

  /* 修饰元素 */
  border-color: var(--accent-color);
  background-color: rgba(121, 217, 255, 0.3);
  box-shadow: 0 0 10px rgba(121, 217, 255, 0.3);

}

input:checked + label::after {

  /* 盒子模型 */
  left: 26px;
  
  /* 修饰元素 */
  background: linear-gradient(135deg, var(--accent-color), #64b5f6);
  box-shadow: 0 3px 8px rgba(121, 217, 255, 0.4), 0 1px 3px rgba(0, 0, 0, 0.2);
  
}

</style>