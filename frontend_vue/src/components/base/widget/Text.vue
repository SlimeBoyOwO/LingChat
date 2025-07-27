<template>
  <div>
      <span id="typed-text-sample">{{ text }}</span>
      <span class="typing-cursor">
      </span>
  </div>
</template>

<script setup>
import { useSlots, computed, ref, watch } from 'vue'
const props = defineProps({
  speed: { type: Number, default: 0 },
})
const sampleText = useSlots().default()[0].children
let typingInterval
const text = ref('')

const typewriter = (speed) => {
    clearInterval(typingInterval);

    let textSampleDisplay = '';
    let i = 0;
    const maxDelay = 200;
    const minDelay = 10;
    const delay = maxDelay - ((speed - 1) / 99) * (maxDelay - minDelay);

    typingInterval = setInterval(() => {
        if (i < sampleText.length) {
          textSampleDisplay += sampleText.charAt(i)
          text.value = textSampleDisplay
            i++;
        } else {
            clearInterval(typingInterval);
        }
    }, delay);
}
watch(() => props.speed, () => {
  typewriter(props.speed)
})
</script>

<style scoped>
/* 打字光标动画 */
.typing-cursor {
    display: inline-block;
    height: 1.2em;
    width: 3px;
    vertical-align: text-bottom;
    margin-left: 4px;
    background-color: var(--accent-color);
    animation: cursor-blink 0.8s infinite;
}
div {
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
