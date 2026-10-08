<template>
  <!-- 恐怖剧本入口过渡：确认警告后"卡死 → 花屏"，然后才真正进入游戏 -->
  <Teleport to="body">
    <div
      v-if="uiStore.horrorEntryPhase !== ''"
      class="horror-entry-overlay"
      :class="`phase-${uiStore.horrorEntryPhase}`"
    >
      <!-- 花屏阶段：满屏噪点 + 反色 + RGB 错位 -->
      <canvas
        v-if="uiStore.horrorEntryPhase === 'static'"
        ref="canvasRef"
        class="static-canvas"
      ></canvas>
      <div
        v-if="uiStore.horrorEntryPhase === 'static'"
        class="invert-flash"
      ></div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useUIStore } from '../../stores/modules/ui/ui'

const uiStore = useUIStore()
const canvasRef = ref<HTMLCanvasElement | null>(null)
let rafId = 0
let audioCtx: AudioContext | null = null

function drawStatic() {
  const c = canvasRef.value
  if (!c) return
  c.width = window.innerWidth
  c.height = window.innerHeight
  const ctx = c.getContext('2d')
  if (!ctx) return

  const image = ctx.createImageData(c.width, c.height)
  const buf = new Uint32Array(image.data.buffer)
  let frame = 0

  const paint = () => {
    // 噪点：黑/白/暗红/青随机像素，每帧全量重填
    for (let i = 0; i < buf.length; i++) {
      const r = Math.random()
      buf[i] =
        r < 0.72
          ? 0xff000000 // 黑
          : r < 0.88
            ? 0xffffffff // 白
            : r < 0.96
              ? 0xff1010b8 // 暗红（BGR 序）
              : 0xffb8b000 // 青
    }
    ctx.putImageData(image, 0, 0)
    frame++
    rafId = requestAnimationFrame(paint)
  }
  paint()
}

/** WebAudio 合成一记短促的故障爆音（无需音频素材） */
function playGlitchBurst() {
  try {
    audioCtx = audioCtx || new AudioContext()
    const dur = 0.35
    const buf = audioCtx.createBuffer(1, audioCtx.sampleRate * dur, audioCtx.sampleRate)
    const data = buf.getChannelData(0)
    for (let i = 0; i < data.length; i++) {
      const t = i / data.length
      data[i] = (Math.random() * 2 - 1) * (1 - t) * 0.5
    }
    const src = audioCtx.createBufferSource()
    src.buffer = buf
    const gain = audioCtx.createGain()
    gain.gain.value = 0.7
    src.connect(gain).connect(audioCtx.destination)
    src.start()
  } catch {
    // 浏览器自动播放策略拦截时静默跳过
  }
}

watch(
  () => uiStore.horrorEntryPhase,
  async (phase) => {
    cancelAnimationFrame(rafId)
    if (phase === 'static') {
      // RGB 错位施加到整个 UI
      document.body.classList.add('ling-ui-corrupt')
      playGlitchBurst()
      await nextTick()
      drawStatic()
    } else {
      document.body.classList.remove('ling-ui-corrupt')
    }
  },
)

onBeforeUnmount(() => {
  cancelAnimationFrame(rafId)
  document.body.classList.remove('ling-ui-corrupt')
  audioCtx?.close().catch(() => {})
})
</script>

<style scoped>
.horror-entry-overlay {
  position: fixed;
  inset: 0;
  z-index: 2000000;
}

/* 卡死阶段：透明但吞掉所有输入，光标变成等待——画面看起来完全没响应 */
.phase-freeze {
  cursor: wait;
  background: transparent;
}

.phase-static {
  background: #000;
}

.static-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

/* 反色一闪 */
.invert-flash {
  position: absolute;
  inset: 0;
  background: #fff;
  mix-blend-mode: difference;
  animation: invert-blink 0.55s steps(2) both;
}

@keyframes invert-blink {
  0%,
  40% {
    opacity: 1;
  }
  20%,
  60%,
  100% {
    opacity: 0;
  }
}
</style>
