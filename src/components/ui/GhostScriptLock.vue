<template>
  <!-- 删角色文件彩蛋（DDLC ghost menu 对应物）：.chr 被全删的剧本进入时锁成
       纯黑底 + 黑白幽灵立绘，盖住一切 UI，不给任何文字和出口按钮；玩家自己
       把任一 .chr 放回标记目录后，轮询发现已解锁会自动撤掉。点窗口 X 走
       ghostQuitZoom 放大脸演出。 -->
  <div v-if="lock" class="ghost-lock-layer">
    <img
      v-if="bgOk"
      class="ghost-lock-bg"
      :src="bgSrc"
      alt=""
      draggable="false"
      @error="bgOk = false"
    />
    <div class="ghost-lock-scanlines"></div>
    <img
      v-if="imgOk"
      class="ghost-lock-sprite"
      :src="imgSrc"
      alt=""
      draggable="false"
      @error="imgOk = false"
    />
  </div>

  <!-- 锁定中点窗口 X：白底 + 立绘突然放大贴脸（DDLC quit: menu_art_m_ghost zoom 3.5），
       演出期间窗口保持打开，随后由 App.vue 的退出流程真正关闭 -->
  <div v-if="quitZoom" class="ghost-quit-layer">
    <img
      v-if="imgOk"
      class="ghost-quit-face"
      :src="imgSrc"
      alt=""
      draggable="false"
      @error="imgOk = false"
    />
  </div>

  <audio ref="musicRef" loop></audio>
  <audio ref="zoomAudioRef"></audio>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUIStore } from '../../stores/modules/ui/ui'
import { checkScriptGhostLock } from '../../api/services/script-info'

const uiStore = useUIStore()

const lock = computed(() => uiStore.ghostLock)
const quitZoom = computed(() => uiStore.ghostQuitZoom)

const imgOk = ref(true)
const bgOk = ref(true)
const musicRef = ref<HTMLAudioElement | null>(null)
const zoomAudioRef = ref<HTMLAudioElement | null>(null)

const imgSrc = computed(() => {
  const dir = lock.value?.assetDir
  if (!dir) return ''
  return convertFileSrc(`${dir}/Pics/ghost-ql-bw.webp`)
})

// 黑白崩坏教室背景（垫在立绘后面，压暗只当氛围底）
const bgSrc = computed(() => {
  const dir = lock.value?.assetDir
  if (!dir) return ''
  return convertFileSrc(`${dir}/Pics/ghost-bg-bw.webp`)
})

const assetPath = (rel: string) => {
  const dir = lock.value?.assetDir
  return dir ? convertFileSrc(`${dir}/${rel}`) : ''
}

// 玩家把 .chr 放回标记目录后自动解锁（无需重启/重进菜单）
let pollTimer = 0
async function pollUnlocked() {
  const current = lock.value
  if (!current || uiStore.ghostQuitZoom) return
  const state = await checkScriptGhostLock(current.scriptName)
  if (!state.locked && lock.value?.scriptName === current.scriptName) {
    uiStore.closeGhostLock()
  }
}

watch(
  lock,
  (value) => {
    clearInterval(pollTimer)
    if (value) {
      imgOk.value = true
      bgOk.value = true
      // DDLC ghostmenu.ogg：幽灵菜单循环 BGM
      if (musicRef.value) {
        musicRef.value.src = assetPath('Musics/ghostmenu.ogg')
        musicRef.value.volume = 0.85
        musicRef.value.play().catch(() => {})
      }
      pollTimer = window.setInterval(pollUnlocked, 2000)
    } else if (musicRef.value) {
      musicRef.value.pause()
      musicRef.value.currentTime = 0
    }
  },
  { immediate: true },
)

// 放大脸：白底 + 立绘冲向屏幕，配 s_kill_glitch1.ogg（夏树崩坏同款短刺音）
watch(quitZoom, (value) => {
  if (value && zoomAudioRef.value) {
    zoomAudioRef.value.src = assetPath('Sounds/s_kill_glitch1.ogg')
    zoomAudioRef.value.volume = 1
    zoomAudioRef.value.play().catch(() => {})
  }
})

onBeforeUnmount(() => {
  clearInterval(pollTimer)
})
</script>

<style scoped>
.ghost-lock-layer {
  position: fixed;
  inset: 0;
  z-index: 999990;
  background: #050607;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  cursor: default;
  user-select: none;
}

.ghost-lock-scanlines {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: repeating-linear-gradient(
    0deg,
    rgba(255, 255, 255, 0.05) 0 1px,
    transparent 1px 3px
  );
  mix-blend-mode: overlay;
  animation: ghost-scan-drift 7s linear infinite;
}

/* 黑白崩坏教室背景：铺满全屏但压暗压灰，只当氛围底，不抢立绘 */
.ghost-lock-bg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  filter: grayscale(1) brightness(0.6) contrast(1.1);
  opacity: 0.55;
}

/* 立绘比例与自由对话一致：height 102%、贴底水平居中（GameRoleAvatar 的
   h-[102%] + object-position center bottom 同款视觉效果） */
.ghost-lock-sprite {
  position: absolute;
  left: 50%;
  bottom: -1%;
  height: 102%;
  max-width: 90vw;
  object-fit: contain;
  object-position: center bottom;
  transform: translateX(-50%);
  filter: grayscale(1) contrast(1.15);
  animation:
    ghost-sprite-jitter 4.8s steps(1, end) infinite,
    ghost-sprite-flicker 9.3s steps(1, end) infinite;
}

.ghost-quit-layer {
  position: fixed;
  inset: 0;
  z-index: 1000002;
  background: #fff;
  overflow: hidden;
  /* 放大脸演出期间挡住一切点击，直到进程退出 */
  pointer-events: auto;
  cursor: wait;
}

/* DDLC quit 标签：menu_art_m_ghost 以 zoom 3.5 怼到 (-100,-100)——脸部瞬间占满屏幕。
   transform-origin 定在立绘脸部（约 21% 高度处），放大时脸钉在原地冲向玩家 */
.ghost-quit-face {
  position: absolute;
  left: 50%;
  top: 42%;
  height: min(82vh, 940px);
  object-fit: contain;
  transform: translate(-50%, -50%) scale(0.9);
  transform-origin: 50% 21%;
  filter: grayscale(1) contrast(1.2);
  animation: ghost-zoom-in 0.42s cubic-bezier(0.55, 0, 0.9, 0.4) forwards;
}

@keyframes ghost-zoom-in {
  from {
    transform: translate(-50%, -50%) scale(0.9);
  }
  to {
    transform: translate(-50%, -50%) scale(4.2);
  }
}

@keyframes ghost-scan-drift {
  from {
    background-position-y: 0;
  }
  to {
    background-position-y: 120px;
  }
}

/* 抖动基于 translateX(-50%) 居中立绘：x 分量必须保留 -50% 再做偏移 */
@keyframes ghost-sprite-jitter {
  0%,
  88%,
  100% {
    transform: translate(-50%, 0);
  }
  89% {
    transform: translate(calc(-50% - 5px), 1px);
  }
  91% {
    transform: translate(calc(-50% + 4px), -1px);
  }
  93% {
    transform: translate(-50%, 0);
  }
}

@keyframes ghost-sprite-flicker {
  0%,
  93%,
  100% {
    opacity: 1;
  }
  94% {
    opacity: 0.55;
  }
  95% {
    opacity: 1;
  }
  97% {
    opacity: 0.7;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ghost-lock-sprite,
  .ghost-lock-scanlines {
    animation: none !important;
  }
}
</style>
