<template>
  <div>
    <p>{{ text }}</p>
    <span></span>
  </div>
</template>

<script setup>

// 导入外部模块
import { useSlots, ref, watch } from 'vue'

// 定义组件属性
const props = defineProps({
  speed:{
    type: Number,
    default: 0,
  },
})

// 定义动态变量
const text = ref()

// 获取插槽内容
const sampleText = useSlots().default()[0].children

// 处理组件行为

// 侦测speed的变化重置打字机
watch(
  () => props.speed,
  () => typewriter(props.speed)
)

let typingInterval
const typewriter = (speed) => {
  clearInterval(typingInterval);
  text.value = '';
  let i = 0;
  const maxDelay = 200;
  const minDelay = 10;
  const delay = maxDelay - ((speed - 1) / 99) * (maxDelay - minDelay);
  typingInterval = setInterval(() => {
    if (i < sampleText.length) {
      text.value += sampleText.charAt(i)
      i++;
    } else {
      clearInterval(typingInterval);
    }
  }, delay);
}

</script>

<style scoped>

div {
  color: #ffffff;
  min-height: 2.5em;
  padding: 15px 20px;
  border-radius: 12px;
  backdrop-filter: blur(10px);
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.1); 
  box-shadow: inset 0 2px 8px rgba(0, 0, 0, 0.2);
  font-family: 'Courier New', Courier, monospace;
}

p {
  display: inline;
}

span { 
  width: 3px;
  height: 1.2em;
  margin-left: 4px;
  display: inline-block;
  vertical-align: text-bottom;
  background-color: var(--accent-color);
  animation: cursor-blink 0.8s infinite;
}

</style>
