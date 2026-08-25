<template>
  <!-- 删角色文件彩蛋（DDLC ghost menu 对应物）：.chr 被全删的剧本进入时锁成
       黑白幽灵立绘，盖住一切 UI，只放出「重置记忆」按钮；玩家放回任一 .chr
       后轮询发现已解锁会自动撤掉。点窗口 X 走 ghostQuitZoom 放大脸演出。 -->
  <div v-if="lock" class="ghost-lock-layer">
    <div class="ghost-lock-scanlines"></div>
    <img
      v-if="imgOk"
      class="ghost-lock-sprite"
      :src="imgSrc"
      alt=""
      draggable="false"
      @error="imgOk = false"
    />
    <div class="ghost-lock-hint">{{ hintText }}</div>
    <button
      class="ghost-lock-reset"
      type="button"
      :disabled="resetting"
      @click="onReset"
    >
      {{ resetting ? '· · ·' : $t('views.menu.resetMemory') }}
    </button>
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

  <!-- 重置成功的白闪（盖住一切，一闪而过） -->
  <div v-if="resetFlash" class="ghost-reset-flash"></div>

  <audio ref="musicRef" loop></audio>
  <audio ref="zoomAudioRef"></audio>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUIStore } from '../../stores/modules/ui/ui'
import { checkScriptGhostLock, resetScriptState } from '../../api/services/script-info'

const uiStore = useUIStore()

const lock = computed(() => uiStore.ghostLock)
const quitZoom = computed(() => uiStore.ghostQuitZoom)

const imgOk = ref(true)
const resetting = ref(false)
const resetFlash = ref(false)
const musicRef = ref<HTMLAudioElement | null>(null)
const zoomAudioRef = ref<HTMLAudioElement | null>(null)

// 乱码提示：正常短句与叠加符版本快速交替，模拟文本腐坏
const HINT_NORMAL = '她不在这里'
const HINT_CORRUPTED = '她̷不̸在̶这̵里̷'
const hintText = ref(HINT_CORRUPTED)
let hintTimer = 0

const imgSrc = computed(() => {
  const dir = lock.value?.assetDir
  if (!dir) return ''
  return convertFileSrc(`${dir}/Pics/ghost-ql-bw.webp`)
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
    clearInterval(hintTimer)
    if (value) {
      imgOk.value = true
      // DDLC ghostmenu.ogg：幽灵菜单循环 BGM
      if (musicRef.value) {
        musicRef.value.src = assetPath('Musics/ghostmenu.ogg')
        musicRef.value.volume = 0.85
        musicRef.value.play().catch(() => {})
      }
      hintTimer = window.setInterval(() => {
        hintText.value = Math.random() < 0.72 ? HINT_CORRUPTED : HINT_NORMAL
      }, 480)
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

async function onReset() {
  const current = lock.value
  if (!current || resetting.value) return
  resetting.value = true
  try {
    await resetScriptState(current.scriptName)
    // 白闪一下再解锁：像画面被冲掉，回到正常的菜单
    resetFlash.value = true
    window.setTimeout(() => {
      resetFlash.value = false
      resetting.value = false
      uiStore.closeGhostLock()
    }, 320)
  } catch {
    resetting.value = false
  }
}

onBeforeUnmount(() => {
  clearInterval(pollTimer)
  clearInterval(hintTimer)
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

.ghost-lock-sprite {
  height: min(78vh, 900px);
  max-width: 90vw;
  object-fit: contain;
  filter: grayscale(1) contrast(1.15);
  animation:
    ghost-sprite-jitter 4.8s steps(1, end) infinite,
    ghost-sprite-flicker 9.3s steps(1, end) infinite;
}

.ghost-lock-hint {
  margin-top: 18px;
  color: rgba(228, 234, 238, 0.82);
  font-size: clamp(18px, 2vw, 30px);
  letter-spacing: 0.35em;
  text-shadow:
    -2px 0 rgba(255, 255, 255, 0.35),
    2px 0 rgba(10, 10, 10, 0.9);
  animation: ghost-hint-flicker 3.1s steps(1, end) infinite;
}

/* 唯一可操作的按钮：沿用主菜单白字阴影风格，但去掉一切彩色 */
.ghost-lock-reset {
  margin-top: 34px;
  padding: 10px 34px;
  background: transparent;
  border: 1px solid rgba(228, 234, 238, 0.55);
  color: rgba(228, 234, 238, 0.88);
  font-family: 'Maoken_Assorted_Sans', -apple-system, BlinkMacSystemFont, 'Segoe_UI', Roboto,
    'Helvetica_Neue', Arial, sans-serif;
  font-size: clamp(18px, 1.8vw, 28px);
  letter-spacing: 0.2em;
  cursor: pointer;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.6);
  transition: background 0.25s ease, color 0.25s ease, transform 0.25s ease;
}

.ghost-lock-reset:hover:not(:disabled) {
  background: rgba(228, 234, 238, 0.92);
  color: #0a0b0c;
  transform: translateY(-2px);
}

.ghost-lock-reset:disabled {
  opacity: 0.5;
  cursor: wait;
}

.ghost-quit-layer {
  position: fixed;
  inset: 0;
  z-index: 1000002;
  background: #fff;
  overflow: hidden;
  /* 放大脸演出期间挡住一切点击（含下面的重置按钮），直到进程退出 */
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

.ghost-reset-flash {
  position: fixed;
  inset: 0;
  z-index: 1000001;
  background: #fff;
  pointer-events: none;
  animation: ghost-flash-out 0.32s ease-out forwards;
}

@keyframes ghost-zoom-in {
  from {
    transform: translate(-50%, -50%) scale(0.9);
  }
  to {
    transform: translate(-50%, -50%) scale(4.2);
  }
}

@keyframes ghost-flash-out {
  from {
    opacity: 1;
  }
  to {
    opacity: 0;
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

@keyframes ghost-sprite-jitter {
  0%,
  88%,
  100% {
    transform: translate(0, 0);
  }
  89% {
    transform: translate(-5px, 1px);
  }
  91% {
    transform: translate(4px, -1px);
  }
  93% {
    transform: translate(0, 0);
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

@keyframes ghost-hint-flicker {
  0%,
  90%,
  100% {
    opacity: 0.82;
  }
  91% {
    opacity: 0.2;
  }
  93% {
    opacity: 0.82;
  }
  96% {
    opacity: 0.45;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ghost-lock-sprite,
  .ghost-lock-hint,
  .ghost-lock-scanlines {
    animation: none !important;
  }
}
</style>
