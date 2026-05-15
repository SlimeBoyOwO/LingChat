<template>
  <!-- 1. 上方提示动画 (带有两边渐变透明的背景，且位置上移至 top-20vh) -->
  <transition appear :css="false" @before-enter="beforeEnter" @enter="enter" @leave="leave">
    <div
      v-if="showAnimation"
      class="fixed left-0 right-0 top-[20vh] flex justify-center pointer-events-none z-1000"
    >
      <!-- 中间实体、两端渐变淡出透明的遮罩背景层 -->
      <div
        class="relative flex items-center px-20 py-4 bg-slate-900/60 backdrop-blur-md mask-fade-edges"
      >
        <div class="flex items-center gap-4 opacity-90">
          <!-- 左侧渐隐线 -->
          <div class="h-px w-10 bg-linear-to-r from-transparent to-brand/80"></div>

          <!-- 左侧旋转星光 -->
          <svg
            class="w-4 h-4 text-brand animate-star-spin drop-shadow-[0_0_8px_rgba(var(--color-brand),0.8)]"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path d="M12 1L14.5 8.5L22 11L14.5 13.5L12 21L9.5 13.5L2 11L9.5 8.5L12 1Z" />
          </svg>

          <!-- 核心文字 -->
          <span
            class="text-xl md:text-2xl font-bold text-white tracking-[0.2em] drop-shadow-[0_4px_12px_rgba(0,0,0,0.8)] text-shadow-glow"
          >
            进入自由对话
          </span>

          <!-- 右侧反向旋转星光 -->
          <svg
            class="w-4 h-4 text-brand animate-star-spin-reverse drop-shadow-[0_0_8px_rgba(var(--color-brand),0.8)]"
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path d="M12 1L14.5 8.5L22 11L14.5 13.5L12 21L9.5 13.5L2 11L9.5 8.5L12 1Z" />
          </svg>

          <!-- 右侧渐隐线 -->
          <div class="h-px w-10 bg-linear-to-l from-transparent to-brand/80"></div>
        </div>
      </div>
    </div>
  </transition>

  <!-- 2. 左侧提示 (完全保持原样) -->
  <transition name="slide-tag">
    <div v-if="shouldShowInfo" class="fixed top-28 left-0 pointer-events-none z-40">
      <!-- 带有右侧箭头切角的标签 -->
      <div
        class="relative flex items-center pr-10 pl-5 py-2 bg-slate-900/60 backdrop-blur-md shadow-md transition-all duration-300"
        style="
          clip-path: polygon(0 0, calc(100% - 16px) 0, 100% 50%, calc(100% - 16px) 100%, 0 100%);
        "
      >
        <!-- 紧贴左侧边缘的高亮指示条 -->
        <div class="absolute left-0 top-0 bottom-0 w-1 bg-brand"></div>

        <div class="flex items-center gap-4">
          <!-- 轮次信息 -->
          <div v-if="freeDialogueInfo.maxRounds > 0" class="flex items-center gap-1.5">
            <span class="text-[13px] text-gray-400">回合</span>
            <div class="flex items-baseline gap-0.5">
              <span class="text-base font-bold text-white">{{
                freeDialogueInfo.currentRound
              }}</span>
              <span class="text-[13px] text-gray-500 font-medium"
                >/{{ freeDialogueInfo.maxRounds }}</span
              >
            </div>
          </div>

          <!-- 分隔线 -->
          <div
            v-if="freeDialogueInfo.maxRounds > 0 && freeDialogueInfo.endLine"
            class="w-px h-3.5 bg-gray-600"
          ></div>

          <!-- 结束词提示 -->
          <div v-if="freeDialogueInfo.endLine" class="flex items-center gap-1.5">
            <span class="text-[13px] text-gray-400">结束词</span>
            <span class="text-[13px] font-medium text-brand">
              "{{ freeDialogueInfo.endLine }}"
            </span>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'

const gameStore = useGameStore()
const uiStore = useUIStore()

// 进入动画控制
const showAnimation = ref(false)
let animationTimeout: number | null = null

// 信息显示控制
const showInfo = ref(false)

// 获取自由对话信息
const freeDialogueInfo = computed(() => {
  return (
    gameStore.runningScript?.freeDialogueInfo || {
      isFreeDialogue: false,
      maxRounds: 0,
      endLine: '',
      currentRound: 0,
    }
  )
})

// 是否应该显示信息
const shouldShowInfo = computed(() => {
  return uiStore.showSettings ? false : showInfo.value
})

// 监听自由对话状态变化
watch(
  () => freeDialogueInfo.value.isFreeDialogue,
  (newVal, oldVal) => {
    if (newVal === oldVal) return

    if (newVal && !oldVal) {
      showAnimation.value = true
      showInfo.value = true

      // 高频事件，动画显示时间缩短至 1.8 秒
      if (animationTimeout) clearTimeout(animationTimeout)
      animationTimeout = window.setTimeout(() => {
        showAnimation.value = false
      }, 1800)
    } else if (!newVal && oldVal) {
      showInfo.value = false
    }
  },
)

// === JS 生命周期钩子：实现丝滑的高斯模糊缩放淡入淡出 ===

function beforeEnter(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '0'
  element.style.transform = 'scale(1.05) translateY(10px)'
  element.style.filter = 'blur(8px)'
  element.style.transition = 'all 0.8s cubic-bezier(0.2, 0.8, 0.2, 1)'
}

function enter(el: Element, done: () => void) {
  const element = el as HTMLElement
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      element.style.opacity = '1'
      element.style.transform = 'scale(1) translateY(0)'
      element.style.filter = 'blur(0px)'
      setTimeout(done, 800)
    })
  })
}

function leave(el: Element, done: () => void) {
  const element = el as HTMLElement
  element.style.transition = 'all 0.6s cubic-bezier(0.4, 0, 0.2, 1)'
  element.style.opacity = '0'
  element.style.transform = 'scale(0.98) translateY(-10px)'
  element.style.filter = 'blur(4px)'
  setTimeout(done, 600)
}
</script>

<style scoped>
/* ========== 横幅两端渐隐遮罩 ========== */
.mask-fade-edges {
  /* 中间 60% 区域是不透明的，向两边最后 20% 渐变到完全透明，完美融合进背景 */
  -webkit-mask-image: linear-gradient(
    to right,
    transparent 0%,
    black 20%,
    black 80%,
    transparent 100%
  );
  mask-image: linear-gradient(to right, transparent 0%, black 20%, black 80%, transparent 100%);
}

/* ========== 星光与发光特效 ========== */
@keyframes star-spin {
  0% {
    transform: rotate(0deg) scale(0.9);
    opacity: 0.7;
  }
  50% {
    transform: rotate(180deg) scale(1.1);
    opacity: 1;
  }
  100% {
    transform: rotate(360deg) scale(0.9);
    opacity: 0.7;
  }
}

@keyframes star-spin-reverse {
  0% {
    transform: rotate(0deg) scale(0.9);
    opacity: 0.7;
  }
  50% {
    transform: rotate(-180deg) scale(1.1);
    opacity: 1;
  }
  100% {
    transform: rotate(-360deg) scale(0.9);
    opacity: 0.7;
  }
}

.animate-star-spin {
  animation: star-spin 4s linear infinite;
}

.animate-star-spin-reverse {
  animation: star-spin-reverse 4s linear infinite;
}

.text-shadow-glow {
  text-shadow: 0 0 12px rgba(var(--color-brand), 0.6);
}

/* ========== 左侧标签滑动动画 (保持不变) ========== */
.slide-tag-enter-active,
.slide-tag-leave-active {
  transition: all 0.5s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.slide-tag-enter-from {
  opacity: 0;
  transform: translateX(-100%);
}

.slide-tag-enter-to {
  opacity: 1;
  transform: translateX(0);
}

.slide-tag-leave-from {
  opacity: 1;
  transform: translateX(0);
}

.slide-tag-leave-to {
  opacity: 0;
  transform: translateX(-100%);
}
</style>
