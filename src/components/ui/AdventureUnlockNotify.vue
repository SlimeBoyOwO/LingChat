<template>
  <Transition name="slide-up">
    <div v-if="visible && currentAdventure" class="adventure-notify">
      <div class="glow-effect"></div>

      <div class="adventure-icon">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path d="M12 2L2 7l10 5 10-5-10-5z" />
          <path d="M2 17l10 5 10-5" />
          <path d="M2 12l10 5 10-5" />
        </svg>
      </div>

      <div class="adventure-content">
        <div class="adventure-header">
          <span class="adventure-label">羁绊冒险解锁</span>
        </div>
        <div class="adventure-title">{{ currentAdventure.name }}</div>
        <div class="adventure-description">{{ currentAdventure.description }}</div>
      </div>

      <div class="progress-bar-container">
        <div class="progress-bar" :style="{ animationDuration: '3000ms' }"></div>
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
@reference "tailwindcss";

.adventure-notify {
  @apply fixed bottom-8 right-8 z-[9999];
  @apply flex items-center gap-4;
  @apply p-4 min-w-[320px] max-w-[400px];
  @apply overflow-hidden rounded-xl;
  background: rgba(15, 15, 15, 0.5);
  backdrop-filter: blur(20px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(147, 51, 234, 0.3);
}

.glow-effect {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 150%;
  height: 150%;
  background: radial-gradient(circle, rgba(147, 51, 234, 0.15) 0%, transparent 60%);
  z-index: -1;
  filter: blur(20px);
}

.adventure-icon {
  @apply shrink-0 w-12 h-12 rounded-lg flex items-center justify-center;
  @apply text-purple-400;
  background: rgba(147, 51, 234, 0.1);
  border: 1px solid rgba(147, 51, 234, 0.2);
}

.adventure-icon svg {
  @apply w-8 h-8;
}

.adventure-content {
  @apply flex flex-col justify-center gap-0.5 flex-1;
}

.adventure-label {
  @apply text-purple-400 text-xs font-bold tracking-wider;
}

.adventure-title {
  @apply text-white font-bold text-sm leading-tight;
}

.adventure-description {
  @apply text-gray-300 text-xs leading-tight;
}

.progress-bar-container {
  @apply absolute bottom-0 left-0 w-full h-[2px] bg-gray-800/50;
}

.progress-bar {
  @apply h-full w-full origin-left;
  background: linear-gradient(90deg, #9333ea, #a855f7, #9333ea);
  animation: progress linear forwards;
}

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
