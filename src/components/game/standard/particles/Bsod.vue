<template>
  <!-- DDLC ch5 fake_exception 同款假异常窗口：浅灰底 + 等宽报错文本。
       trace/独白文本由剧本经 background_effect 的 text/echo 字段自带；
       缺省时 trace 用通用占位，无独白——引擎不硬编码任何剧本的彩蛋文本。 -->
  <div
    v-if="enabled"
    class="crash-layer"
  >
    <div class="crash-title">An exception has occurred.</div>
    <div class="crash-trace">
      {{ traceLine }}<br>
      See traceback.txt for details.
    </div>
    <div v-if="echoHtml" class="crash-echo" v-html="echoHtml"></div>
    <div class="crash-flicker"></div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'

defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
})

const uiStore = useUIStore()

const traceLine = computed(
  () => uiStore.bsodText || 'File "game/script.rpy", line 88',
)
// 独白允许剧本用 \n 分行；转义后换行转 <br>，防 HTML 注入
const echoHtml = computed(() => {
  const raw = uiStore.bsodEcho
  if (!raw) return ''
  const escaped = raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return escaped.replace(/\n/g, '<br>')
})
</script>

<style scoped>
.crash-layer {
  position: absolute;
  inset: 0;
  background: #dadada;
  color: #111;
  font-family: 'Consolas', 'Courier New', monospace;
  padding: 6vh 8vw;
  pointer-events: none;
  display: flex;
  flex-direction: column;
  gap: 4vh;
  /* 整体极轻微的不规律抽动，像信号不良的显示器 */
  animation: crash-jitter 2.7s steps(1) infinite;
}

.crash-title {
  font-size: 4.2vh;
  font-weight: 700;
}

.crash-trace {
  font-size: 2vh;
  line-height: 1.7;
  opacity: 0.85;
}

.crash-echo {
  margin-top: 8vh;
  font-size: 1.7vh;
  line-height: 1.9;
  opacity: 0;
  /* 在常用 1.6–2.4s 演出窗口内完成淡入，避免刚出现就被下一拍收掉。 */
  animation: crash-echo-in 0.7s ease-out 0.55s forwards;
}

/* 偶发的水平细亮纹扫过 */
.crash-flicker {
  position: absolute;
  inset: 0;
  background: repeating-linear-gradient(
    0deg,
    transparent 0 97px,
    rgba(255, 255, 255, 0.35) 97px 98px
  );
  opacity: 0;
  animation: crash-scan 3.4s steps(1) infinite;
}

@keyframes crash-jitter {
  0%, 88% { transform: translate(0, 0); }
  89% { transform: translate(-1px, 1px); }
  92% { transform: translate(1px, 0); }
  95% { transform: translate(0, -1px); }
  96%, 100% { transform: translate(0, 0); }
}

@keyframes crash-echo-in {
  from { opacity: 0; }
  to { opacity: 0.55; }
}

@keyframes crash-scan {
  0%, 78% { opacity: 0; }
  79% { opacity: 0.5; }
  80%, 100% { opacity: 0; }
}
</style>
