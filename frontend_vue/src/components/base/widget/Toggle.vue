<template>
  <div class="toggle-container">
    <input type="checkbox"
      :id="id"
      class="toggle-input"
      @change="change"
    >
    <label :for="id" class="toggle-label">
      <slot></slot>
    </label>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

const props = defineProps({
  text: {
    type: String,
    default: "默认文本"
  }

})
const id = ref('')
onMounted(() => {
  id.value = `toggle-${Math.random().toString(36).substring(2, 9)}`
})

// 处理组件事件
const emit = defineEmits(['change'])
const change = (event: Event) => {
  const target = event.target as HTMLInputElement
  emit('change', target.checked)
}


</script>

<style scoped>
.toggle-container {
    display: flex;
    align-items: center;
}

.toggle-input {
    display: none;
}

.toggle-label {
    cursor: pointer;
    position: relative;
    padding-left: 60px;
    font-size: 14px;
    color: white; /* 修改文字颜色为白色 */
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

.toggle-label::before {
    content: '';
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 50px;
    height: 26px;
    background-color: rgba(255, 255, 255, 0.2); /* 透明背景 */
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 13px;
    transition: all 0.3s ease;
    backdrop-filter: blur(5px);
}

.toggle-label::after {
    content: '';
    position: absolute;
    left: 4px;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 20px;
    background: linear-gradient(135deg, #ffffff, #f0f0f0); /* 轻微渐变的白色 */
    border-radius: 50%;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3), 0 1px 2px rgba(0, 0, 0, 0.1);
    transition: all 0.3s ease;
}

.toggle-input:checked + .toggle-label::before {
    background-color: rgba(121, 217, 255, 0.3); /* 半透明主题色 */
    border-color: var(--accent-color);
    box-shadow: 0 0 10px rgba(121, 217, 255, 0.3);
}

.toggle-input:checked + .toggle-label::after {
    left: 26px;
    background: linear-gradient(135deg, var(--accent-color), #64b5f6); /* 激活时使用主题色渐变 */
    box-shadow: 0 3px 8px rgba(121, 217, 255, 0.4), 0 1px 3px rgba(0, 0, 0, 0.2);
}

/* 文本样本显示区域 */
.text-sample-display {
    background: rgba(0, 0, 0, 0.3); /* 半透明黑色背景 */
    color: #ffffff;
    padding: 15px 20px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    font-family: 'Courier New', Courier, monospace;
    min-height: 2.5em; /* 确保有足够高度容纳光标 */
    backdrop-filter: blur(10px);
    box-shadow: inset 0 2px 8px rgba(0, 0, 0, 0.2);
}
</style>