<template>
  <!-- 突脸惊吓：全屏黑底 + 图片急速放大冲入 + 高频抖动 + 自带音效 -->
  <div
    v-if="visible"
    class="jumpscare-layer"
  >
    <div class="jumpscare-shake">
      <img
        class="jumpscare-img"
        :src="imgSrc"
        alt=""
        draggable="false"
      />
    </div>
    <audio ref="audioRef"></audio>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUIStore } from '../../../../stores/modules/ui/ui'

const uiStore = useUIStore()

const visible = ref(false)
const audioRef = ref<HTMLAudioElement | null>(null)
let hideTimer = 0

const imgSrc = computed(() => {
  const p = uiStore.jumpscareImage
  if (!p) return ''
  if (p.startsWith('http') || p.startsWith('data:') || p.startsWith('blob:')) return p
  return convertFileSrc(p)
})

watch(
  () => uiStore.jumpscareUntil,
  (until) => {
    clearTimeout(hideTimer)
    if (!until || !uiStore.jumpscareImage) {
      visible.value = false
      if (audioRef.value) {
        audioRef.value.pause()
        audioRef.value.currentTime = 0
      }
      return
    }

    visible.value = true

    // 音效：组件自管，避免与全局短效音效的"同路径不重播"冲突
    if (uiStore.jumpscareSound && audioRef.value) {
      audioRef.value.src = convertFileSrc(uiStore.jumpscareSound)
      audioRef.value.volume = 1
      audioRef.value.play().catch(() => {})
    }

    const remain = until - Date.now()
    hideTimer = window.setTimeout(() => {
      visible.value = false
      if (audioRef.value) {
        audioRef.value.pause()
        audioRef.value.currentTime = 0
      }
      uiStore.clearJumpscare()
    }, Math.max(150, remain))
  },
)

onBeforeUnmount(() => {
  clearTimeout(hideTimer)
})
</script>

<style scoped>
/* 脱离 GameBackground 的层叠上下文，压过对话框等所有 UI */
.jumpscare-layer {
  position: fixed;
  inset: 0;
  z-index: 1000000;
  background: #000;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.jumpscare-shake {
  animation: jumpscare-shake 0.07s linear infinite;
}

.jumpscare-img {
  max-width: 100vw;
  max-height: 100vh;
  object-fit: contain;
  animation: jumpscare-zoom 0.5s cubic-bezier(0.1, 1.4, 0.3, 1) both;
  filter: contrast(1.15) saturate(1.2);
}

/* 急速放大冲入视野 */
@keyframes jumpscare-zoom {
  from {
    transform: scale(0.55);
    opacity: 0.4;
  }
  to {
    transform: scale(1.12);
    opacity: 1;
  }
}

@keyframes jumpscare-shake {
  0% {
    transform: translate(0, 0);
  }
  25% {
    transform: translate(-9px, 5px);
  }
  50% {
    transform: translate(8px, -6px);
  }
  75% {
    transform: translate(-5px, -8px);
  }
  100% {
    transform: translate(6px, 7px);
  }
}
</style>
