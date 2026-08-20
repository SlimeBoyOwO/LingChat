<template>
  <!-- DDLC ch5 fake_exception 同款假异常窗口：浅灰底 + 等宽报错文本（内容为原创恶搞文本，非真实错误） -->
  <div
    v-if="enabled"
    class="crash-layer"
  >
    <div class="crash-title">An exception has occurred.</div>
    <div class="crash-trace">
      File "game/data/game_data/scripts/standalone/第七个测试剧本/Chapters/end_cold.yaml", line 88<br>
      See traceback.txt for details.
    </div>
    <div class="crash-echo">
      唔……人家好像把什么弄坏了？<br>
      等人家一下，应该还能修好……<br>
      其实吧，把"她"直接删掉会快一点。啊哈哈。
    </div>
    <div class="crash-flicker"></div>
  </div>
</template>

<script setup lang="ts">
defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
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
  /* 彩蛋独白延迟淡入：像有人在报错页背后小声说话 */
  animation: crash-echo-in 1.2s ease-out 1.6s forwards;
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
