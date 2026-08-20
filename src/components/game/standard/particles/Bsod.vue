<template>
  <!-- 假死机画面：模拟系统崩溃蓝屏（内容为原创恶搞文本，非真实系统信息） -->
  <div
    v-if="enabled"
    class="bsod-layer"
  >
    <div class="bsod-face">:(</div>
    <div class="bsod-text">
      LingChat OS 遇到无法恢复的错误，正在尝试挽留即将消失的数据。
    </div>
    <div class="bsod-progress">{{ progress }}% 完成</div>
    <div class="bsod-stop">终止代码：SCRIPT_7_NOT_FOUND</div>
    <div class="bsod-hint">如果你看到这张脸，说明有人不想让你继续。</div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
})

// 百分比故意卡在奇怪的区间跳动，最后停住不动
const progress = ref(0)
let timer = 0

onMounted(() => {
  const tick = () => {
    if (progress.value < 99) {
      progress.value = Math.min(99, progress.value + Math.floor(Math.random() * 23))
    }
    timer = window.setTimeout(tick, 700 + Math.random() * 1600)
  }
  tick()
})

onBeforeUnmount(() => clearTimeout(timer))
</script>

<style scoped>
.bsod-layer {
  position: absolute;
  inset: 0;
  background: #0a3d91;
  color: #fff;
  font-family: 'Consolas', 'Courier New', monospace;
  padding: 12vh 10vw;
  pointer-events: none;
  display: flex;
  flex-direction: column;
  gap: 3vh;
}

.bsod-face {
  font-size: 14vh;
  line-height: 1;
}

.bsod-text {
  font-size: 3vh;
  max-width: 60%;
}

.bsod-progress {
  font-size: 2.4vh;
}

.bsod-stop {
  margin-top: 4vh;
  font-size: 1.8vh;
  opacity: 0.8;
}

.bsod-hint {
  font-size: 1.6vh;
  opacity: 0.55;
}
</style>
