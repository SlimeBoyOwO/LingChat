<template>
  <Transition name="slide-up">
    <div
      v-if="visible && currentAdventure"
      class="fixed bottom-8 right-8 z-9999 flex items-center gap-4 p-4 min-w-[320px] max-w-100 overflow-hidden rounded-xl"
      style="
        background: rgba(15, 15, 15, 0.5);
        backdrop-filter: blur(20px);
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
        border: 1px solid rgba(147, 51, 234, 0.3);
      "
    >
      <!-- 光晕效果 -->
      <div
        class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[150%] h-[150%] -z-10"
        style="
          background: radial-gradient(circle, rgba(147, 51, 234, 0.15) 0%, transparent 60%);
          filter: blur(20px);
        "
      ></div>

      <!-- 图标 -->
      <div
        class="shrink-0 w-12 h-12 rounded-lg flex items-center justify-center text-purple-400"
        style="background: rgba(147, 51, 234, 0.1); border: 1px solid rgba(147, 51, 234, 0.2)"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          class="w-8 h-8"
        >
          <path d="M12 2L2 7l10 5 10-5-10-5z" />
          <path d="M2 17l10 5 10-5" />
          <path d="M2 12l10 5 10-5" />
        </svg>
      </div>

      <!-- 内容 -->
      <div class="flex flex-col justify-center gap-0.5 flex-1">
        <div class="text-purple-400 text-xs font-bold tracking-wider">羁绊冒险解锁</div>
        <div class="text-white font-bold text-sm leading-tight">
          {{ currentAdventure.name }}
        </div>
        <div class="text-gray-300 text-xs leading-tight">
          {{ currentAdventure.description }}
        </div>
      </div>

      <!-- 进度条容器 -->
      <div class="absolute bottom-0 left-0 w-full h-0.5 bg-gray-800/50">
        <div
          class="h-full w-full origin-left"
          style="
            background: linear-gradient(90deg, #9333ea, #a855f7, #9333ea);
            animation: progress linear forwards;
            animation-duration: 3000ms;
          "
        ></div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useAdventureStore } from '@/stores/modules/adventure'
import type { UnlockedAdventure } from '@/api/services/adventure'

const adventureStore = useAdventureStore()
const visible = ref(false)
const currentAdventure = ref<UnlockedAdventure | null>(null)

let timer: number | null = null

const showNotification = (adventure: UnlockedAdventure) => {
  currentAdventure.value = adventure
  visible.value = true

  if (timer) clearTimeout(timer)
  timer = window.setTimeout(() => {
    visible.value = false
    currentAdventure.value = null
  }, 3000)
}

watch(
  () => adventureStore.unlockNotifications.length,
  (count) => {
    if (count > 0 && !visible.value) {
      const adventure = adventureStore.popUnlockNotification()
      if (adventure) {
        showNotification(adventure)
      }
    }
  },
  { immediate: true },
)
</script>

<style scoped>
/* 保留必要的动画和过渡效果 */
@keyframes progress {
  0% {
    transform: scaleX(1);
  }
  100% {
    transform: scaleX(0);
  }
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100px) scale(0.9);
  opacity: 0;
}
</style>
